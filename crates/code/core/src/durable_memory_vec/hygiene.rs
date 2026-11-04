//! Derived content-to-ID lookup, owned by the native store's mutation lock.

use super::{memory_id, CONTENT_FIELD};
use anyhow::Result;
use aster_durable_memory::memory_content_fingerprint;
use aster_vec::Collection;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};

#[derive(Eq, Hash, PartialEq)]
enum ContentKey {
    Exact([u8; 32]),
    Durable([u8; 32]),
}

pub(super) struct MemoryHygiene {
    entries: HashMap<ContentKey, BTreeSet<String>>,
    dirty: bool,
}

impl MemoryHygiene {
    pub(super) fn load(collection: &Collection) -> Result<Self> {
        let mut hygiene = Self {
            entries: HashMap::new(),
            dirty: false,
        };
        for document in collection.iter_with_options(Some(&[CONTENT_FIELD]), false)? {
            let document = document?;
            if let Some(content) = document.get_string(CONTENT_FIELD)? {
                hygiene.remember(&memory_id(&document)?, &content);
            }
        }
        Ok(hygiene)
    }

    pub(super) fn ensure_current(&mut self, collection: &Collection) -> Result<()> {
        if self.dirty {
            *self = Self::load(collection)?;
        }
        Ok(())
    }

    pub(super) fn exact_duplicate(&self, content: &str) -> Option<&str> {
        self.first_id(&exact_key(content))
    }

    pub(super) fn durable_duplicate(&self, content: &str) -> Option<&str> {
        durable_key(content).and_then(|key| self.first_id(&key))
    }

    fn first_id(&self, key: &ContentKey) -> Option<&str> {
        self.entries.get(key)?.first().map(String::as_str)
    }

    pub(super) fn remember(&mut self, id: &str, content: &str) {
        for key in keys(content) {
            self.entries.entry(key).or_default().insert(id.to_owned());
        }
    }

    pub(super) fn forget(&mut self, id: &str, content: &str) {
        for key in keys(content) {
            if let Some(ids) = self.entries.get_mut(&key) {
                ids.remove(id);
                if ids.is_empty() {
                    self.entries.remove(&key);
                }
            }
        }
    }

    pub(super) fn invalidate(&mut self) {
        // A checkpoint failure can follow primary publication.
        self.dirty = true;
    }

    pub(super) fn finish(&mut self) {
        self.dirty = false;
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.dirty = false;
    }
}

fn keys(content: &str) -> impl Iterator<Item = ContentKey> {
    std::iter::once(exact_key(content)).chain(durable_key(content))
}

fn exact_key(content: &str) -> ContentKey {
    ContentKey::Exact(Sha256::digest(content.trim().as_bytes()).into())
}

fn durable_key(content: &str) -> Option<ContentKey> {
    memory_content_fingerprint(content)
        .map(|fingerprint| ContentKey::Durable(Sha256::digest(fingerprint.as_bytes()).into()))
}
