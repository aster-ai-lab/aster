//! Build reproducible native AIX archives from validated package directories.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use aster_use_core::{UseError, UseResult};
use tempfile::NamedTempFile;
use tokio::fs;
use zip::write::SimpleFileOptions;

use crate::digest::collect_package_files;
use crate::package::io_error;
use crate::{inspect_aix_package, inspect_release_bundle, ReleaseBundlePackage};

/// Build and validate a deterministic `.aix` archive, then publish it atomically.
///
/// The destination must be outside the source and must not already exist; its
/// parent directory must exist. Native
/// package validation, file/expanded/archive limits and path rules are retained.
/// Unix file permissions are preserved; other targets use regular-file mode 0644.
/// No installation, execution, signing or upload occurs.
///
/// Validation errors use the existing extension codes; invalid output requests
/// use `use.extension.package_output_invalid`. I/O and join failures use
/// `use.extension.io`. Blocking archive work is bounded by the source limits but
/// cannot be aborted once started. Cancellation during final publication may
/// still publish the completed archive.
pub async fn build_aix_package(source: &Path, output: &Path) -> UseResult<ReleaseBundlePackage> {
    if output.extension() != Some(std::ffi::OsStr::new("aix")) {
        return Err(invalid_output(
            "The output requires the exact .aix extension.",
        ));
    }
    let expected = inspect_release_bundle(source).await?;
    let source = fs::canonicalize(source)
        .await
        .map_err(|error| io_error("resolve AIX source directory", source, error))?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)
        .await
        .map_err(|error| io_error("resolve AIX output directory", parent, error))?;
    let filename = output
        .file_name()
        .ok_or_else(|| invalid_output("The output requires a filename."))?;
    let output = parent.join(filename);
    if output.starts_with(&source) {
        return Err(invalid_output(
            "The AIX output must be outside its source directory.",
        ));
    }
    let temporary = tokio::task::spawn_blocking(move || write_archive(&source, &parent))
        .await
        .map_err(|error| worker_error("build archive", error))??;
    let package = inspect_aix_package(temporary.path()).await?;
    if package != expected {
        return Err(UseError::new(
            "use.extension.package_changed",
            "The AIX contents do not match the validated source package.",
        ));
    }
    tokio::task::spawn_blocking(move || {
        crate::atomic_file::persist_named_temporary_noclobber_blocking(temporary, &output)
            .map_err(|error| io_error("publish new AIX archive", &output, error))
    })
    .await
    .map_err(|error| worker_error("publish archive", error))??;
    Ok(package)
}

fn write_archive(source: &Path, parent: &Path) -> UseResult<NamedTempFile> {
    let files = collect_package_files(source)?;
    let mut temporary = tempfile::Builder::new()
        .suffix(".aix")
        .tempfile_in(parent)
        .map_err(|error| io_error("stage AIX archive", parent, error))?;
    let mut archive = zip::ZipWriter::new(temporary.as_file_mut());
    for file in files {
        let input = File::open(&file.path)
            .map_err(|error| io_error("open AIX source file", &file.path, error))?;
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(9))
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(
                file_mode(&input)
                    .map_err(|error| io_error("inspect AIX source mode", &file.path, error))?,
            );
        archive
            .start_file(file.normalized, options)
            .map_err(archive_error)?;
        let copied = io::copy(&mut input.take(file.size + 1), &mut archive)
            .map_err(|error| io_error("write AIX file", &file.path, error))?;
        if copied != file.size {
            return Err(UseError::new(
                "use.extension.package_changed",
                "An AIX source file changed while being archived.",
            ));
        }
    }
    archive.finish().map_err(archive_error)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io_error("sync staged AIX archive", temporary.path(), error))?;
    Ok(temporary)
}

#[cfg(unix)]
fn file_mode(file: &File) -> io::Result<u32> {
    use std::os::unix::fs::PermissionsExt;
    Ok(file.metadata()?.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn file_mode(_file: &File) -> io::Result<u32> {
    Ok(0o644)
}

fn archive_error(error: zip::result::ZipError) -> UseError {
    let code = match &error {
        zip::result::ZipError::Io(_) => "use.extension.io",
        _ => "use.extension.package_archive_invalid",
    };
    UseError::new(code, format!("Cannot write AIX archive: {error}"))
}

fn worker_error(action: &str, error: tokio::task::JoinError) -> UseError {
    UseError::new("use.extension.io", format!("Cannot {action}: {error}"))
}

fn invalid_output(message: &str) -> UseError {
    UseError::new("use.extension.package_output_invalid", message)
}

#[cfg(test)]
#[path = "aix_pack_tests.rs"]
mod tests;
