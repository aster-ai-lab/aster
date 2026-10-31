//! Same-step validation of private immutable model tensors, never user tensors.
//!
//! Only the native complete-step owner enables this scope. Its tensor graph has
//! no in-place writes; clones retain identity, and new operations/views get a
//! fresh Candle TensorId. First checks execute synchronously at the original
//! boundary. A hit proves the identical immutable tensor was already checked
//! under an equal/stronger predicate. No values, precision or errors are deferred.
//! This must not be enabled around arbitrary caller-owned Candle tensors: Candle
//! also exposes mutating APIs, which would invalidate this ownership proof.
use aster_power::{
    error::{PowerError, Result},
    inference::InferenceLimits,
};
use candle_core::{Tensor, TensorId};
use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    marker::PhantomData,
    rc::Rc,
};

const SLOTS: usize = 8192;
type Entry = Option<(TensorId, bool)>;
thread_local! { static ACTIVE: RefCell<Option<Cache>> = const { RefCell::new(None) }; }
thread_local! { static LAST: RefCell<Option<(u64, u64)>> = const { RefCell::new(None) }; }

pub(crate) fn take_last_counts() -> Option<(u64, u64)> {
    LAST.with(|last| last.borrow_mut().take())
}

struct Cache {
    entries: Vec<Entry>,
    checks: u64,
    hits: u64,
    poisoned: bool,
}

/// Non-Send guard: a worker's private cache cannot migrate to another thread.
pub(crate) struct Scope(PhantomData<Rc<()>>);
impl Scope {
    pub(crate) fn begin(limits: &InferenceLimits, staged_bytes: u64) -> Result<Self> {
        let bytes = (SLOTS as u64)
            .checked_mul(std::mem::size_of::<Entry>() as u64)
            .and_then(|n| n.checked_add(256))
            .and_then(|n| n.checked_add(staged_bytes))
            .ok_or_else(|| invalid("validation cache metadata budget overflowed"))?;
        limits.checked_state_bytes(bytes, "step-local validation metadata plus staged state")?;
        ACTIVE.with(|active| {
            let mut active = active
                .try_borrow_mut()
                .map_err(|_| invalid("validation scope is already borrowed"))?;
            if active.is_some() {
                return Err(invalid("native validation scopes cannot nest"));
            }
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(SLOTS)
                .map_err(|_| invalid("cannot allocate bounded validation metadata"))?;
            if entries.capacity() != SLOTS {
                return Err(invalid(
                    "validation metadata allocation has an unexpected capacity",
                ));
            }
            entries.resize(SLOTS, None);
            *active = Some(Cache {
                entries,
                checks: 0,
                hits: 0,
                poisoned: false,
            });
            Ok(Self(PhantomData))
        })
    }

    pub(crate) fn validate_complete(&self) -> Result<()> {
        ACTIVE.with(|active| {
            let active = active
                .try_borrow()
                .map_err(|_| invalid("validation scope is still borrowed at commit"))?;
            match active.as_ref() {
                Some(cache) if !cache.poisoned => Ok(()),
                _ => Err(invalid(
                    "native validation scope is absent or poisoned; refusing commit",
                )),
            }
        })
    }

    #[cfg(test)]
    fn counts(&self) -> (u64, u64) {
        ACTIVE.with(|active| {
            let active = active.borrow();
            let cache = active.as_ref().unwrap();
            (cache.checks, cache.hits)
        })
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let counts = ACTIVE.with(|active| active.borrow_mut().take().map(|c| (c.checks, c.hits)));
        LAST.with(|last| *last.borrow_mut() = counts);
    }
}

pub(crate) fn validate(
    tensor: &Tensor,
    nonnegative: bool,
    check: impl FnOnce() -> Result<()>,
) -> Result<()> {
    ACTIVE.with(|active| {
        let mut active = active
            .try_borrow_mut()
            .map_err(|_| invalid("native finite validation cannot reenter its scope"))?;
        let Some(cache) = active.as_mut() else {
            return check();
        };
        if cache.poisoned {
            return Err(invalid("native validation step is poisoned"));
        }
        let mut hash = DefaultHasher::new();
        tensor.id().hash(&mut hash);
        let slot = hash.finish() as usize % SLOTS;
        if cache.entries[slot]
            .is_some_and(|(id, positive)| id == tensor.id() && (positive || !nonnegative))
        {
            cache.hits = cache.hits.checked_add(1).ok_or_else(|| {
                cache.poisoned = true;
                invalid("native validation hit counter overflowed")
            })?;
            return Ok(());
        }
        cache.checks = cache.checks.checked_add(1).ok_or_else(|| {
            cache.poisoned = true;
            invalid("native validation check counter overflowed")
        })?;
        match check() {
            Ok(()) => {
                // Direct-mapped eviction is safe: a collision requires another
                // real validation. No tensor/storage or GPU payload is retained.
                cache.entries[slot] = Some((tensor.id(), nonnegative));
                Ok(())
            }
            Err(error) => {
                cache.poisoned = true;
                Err(error)
            }
        }
    })
}

fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}

#[cfg(test)]
mod tests;
