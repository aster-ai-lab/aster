use super::{
    validate_snapshot, ScheduleError, ScheduleScope, ScheduleSnapshot, ScheduleStore,
    SCHEDULE_STORE_MAX_BYTES, SCHEDULE_STORE_SCHEMA_V1,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct MemoryScheduleStore {
    partitions: Mutex<BTreeMap<ScheduleScope, ScheduleSnapshot>>,
}

impl MemoryScheduleStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScheduleStore for MemoryScheduleStore {
    fn load(&self, scope: &ScheduleScope) -> Result<ScheduleSnapshot, ScheduleError> {
        scope.validate()?;
        let partitions = self
            .partitions
            .lock()
            .map_err(|_| ScheduleError::Store("memory schedule lock is poisoned".to_string()))?;
        let snapshot = partitions
            .get(scope)
            .cloned()
            .unwrap_or_else(|| ScheduleSnapshot::new(scope.clone()));
        validate_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn commit(
        &self,
        expected_generation: u64,
        next: ScheduleSnapshot,
    ) -> Result<(), ScheduleError> {
        validate_snapshot(&next)?;
        let mut partitions = self
            .partitions
            .lock()
            .map_err(|_| ScheduleError::Store("memory schedule lock is poisoned".to_string()))?;
        let current_generation = partitions
            .get(&next.scope)
            .map_or(0, |snapshot| snapshot.generation);
        if current_generation != expected_generation
            || expected_generation.checked_add(1) != Some(next.generation)
        {
            return Err(ScheduleError::Conflict);
        }
        partitions.insert(next.scope.clone(), next);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleStoreFile {
    schema: String,
    partitions: Vec<ScheduleSnapshot>,
}

impl ScheduleStoreFile {
    fn empty() -> Self {
        Self {
            schema: SCHEDULE_STORE_SCHEMA_V1.to_string(),
            partitions: Vec::new(),
        }
    }
}

/// Atomic JSON data adapter. The sidecar lock fences independent scheduler
/// processes; this is durable runtime data, not product configuration or ACL.
#[derive(Debug)]
pub struct FileScheduleStore {
    path: PathBuf,
    lock_path: PathBuf,
}

impl FileScheduleStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        Self {
            path,
            lock_path: PathBuf::from(lock_path),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, ScheduleError>,
    ) -> Result<T, ScheduleError> {
        let parent = self
            .path
            .parent()
            .ok_or(ScheduleError::InvalidField("path"))?;
        if !parent.is_dir() {
            return Err(ScheduleError::Store(
                "schedule file parent does not exist".to_string(),
            ));
        }
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(io_error)?;
        lock.lock_exclusive().map_err(io_error)?;
        let result = operation();
        let unlock = lock.unlock().map_err(io_error);
        match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn read_unlocked(&self) -> Result<ScheduleStoreFile, ScheduleError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ScheduleStoreFile::empty());
            }
            Err(error) => return Err(io_error(error)),
        };
        if file.metadata().map_err(io_error)?.len() > SCHEDULE_STORE_MAX_BYTES as u64 {
            return Err(ScheduleError::Store(
                "schedule file exceeds its size limit".to_string(),
            ));
        }
        let document: ScheduleStoreFile = serde_json::from_reader(file).map_err(json_error)?;
        if document.schema != SCHEDULE_STORE_SCHEMA_V1 {
            return Err(ScheduleError::UnsupportedSchema);
        }
        for partition in &document.partitions {
            validate_snapshot(partition)?;
        }
        let mut scopes = std::collections::BTreeSet::new();
        if document
            .partitions
            .iter()
            .any(|partition| !scopes.insert(partition.scope.clone()))
        {
            return Err(ScheduleError::Store(
                "schedule file contains duplicate scopes".to_string(),
            ));
        }
        Ok(document)
    }

    fn write_unlocked(&self, document: &ScheduleStoreFile) -> Result<(), ScheduleError> {
        let parent = self
            .path
            .parent()
            .ok_or(ScheduleError::InvalidField("path"))?;
        let bytes = serde_json::to_vec_pretty(document).map_err(json_error)?;
        if bytes.len() > SCHEDULE_STORE_MAX_BYTES {
            return Err(ScheduleError::Store(
                "schedule file exceeds its size limit".to_string(),
            ));
        }
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
        temporary.write_all(&bytes).map_err(io_error)?;
        temporary.as_file().sync_all().map_err(io_error)?;
        temporary
            .persist(&self.path)
            .map_err(|error| io_error(error.error))?;
        Ok(())
    }
}

impl ScheduleStore for FileScheduleStore {
    fn load(&self, scope: &ScheduleScope) -> Result<ScheduleSnapshot, ScheduleError> {
        scope.validate()?;
        self.with_lock(|| {
            let document = self.read_unlocked()?;
            let snapshot = document
                .partitions
                .into_iter()
                .find(|partition| &partition.scope == scope)
                .unwrap_or_else(|| ScheduleSnapshot::new(scope.clone()));
            Ok(snapshot)
        })
    }

    fn commit(
        &self,
        expected_generation: u64,
        next: ScheduleSnapshot,
    ) -> Result<(), ScheduleError> {
        validate_snapshot(&next)?;
        self.with_lock(|| {
            let mut document = self.read_unlocked()?;
            let existing = document
                .partitions
                .iter()
                .position(|partition| partition.scope == next.scope);
            let current_generation = existing
                .and_then(|index| document.partitions.get(index))
                .map_or(0, |snapshot| snapshot.generation);
            if current_generation != expected_generation
                || expected_generation.checked_add(1) != Some(next.generation)
            {
                return Err(ScheduleError::Conflict);
            }
            if let Some(index) = existing {
                document.partitions[index] = next;
            } else {
                document.partitions.push(next);
            }
            document
                .partitions
                .sort_by(|left, right| left.scope.cmp(&right.scope));
            self.write_unlocked(&document)
        })
    }
}

fn io_error(error: std::io::Error) -> ScheduleError {
    ScheduleError::Store(error.to_string())
}

fn json_error(error: serde_json::Error) -> ScheduleError {
    ScheduleError::Store(error.to_string())
}
