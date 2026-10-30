//! Reconstructed observed workspace states; never a second journal or authority.

use super::*;
use a3s_effect::{ToolFileIdentityV1, ToolFileKind, ToolPathChangeV1};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct MutationProgress {
    baseline: BTreeMap<String, ToolFileIdentityV1>,
    current: BTreeMap<String, ToolFileIdentityV1>,
    seen: BTreeSet<String>,
    incoherent: bool,
}

impl MutationProgress {
    pub(crate) fn observe(
        &mut self,
        changes: Option<Vec<ToolPathChangeV1>>,
    ) -> Result<bool, GoalControlError> {
        let Some(changes) = changes else {
            self.incoherent = true;
            return Ok(false);
        };
        self.incoherent = false;
        self.seen.insert(self.identity()?);
        let mut endpoints: BTreeMap<String, (Option<ToolFileIdentityV1>, ToolFileIdentityV1)> =
            BTreeMap::new();
        let mut coherent = true;
        for change in changes {
            if let Some((_, previous)) = endpoints.get_mut(&change.path) {
                // Overlapping observers may repeat the same after-state.
                coherent &= change.before.as_ref() == Some(previous) || &change.after == previous;
                *previous = change.after;
            } else {
                endpoints.insert(change.path, (change.before, change.after));
            }
        }
        let mut net_changed = false;
        for (path, (before, after)) in endpoints {
            if let Some(previous) = self.current.get(&path) {
                self.incoherent |= before.as_ref() != Some(previous);
            }
            net_changed |= before
                .as_ref()
                .is_some_and(|before| changed(before, &after));
            self.baseline
                .entry(path.clone())
                .or_insert_with(|| before.unwrap_or_else(|| after.clone()));
            self.current.insert(path, after);
        }
        let novel = self.seen.insert(self.identity()?);
        self.incoherent |= !coherent;
        Ok(coherent && net_changed && novel)
    }

    pub(crate) fn last_observation_is_coherent(&self) -> bool {
        !self.incoherent
    }

    fn identity(&self) -> Result<String, GoalControlError> {
        // Sparse deviations let a newly observed path return to its original
        // state without changing identities of earlier, already-seen cuts.
        let deviations: BTreeMap<_, _> = self
            .current
            .iter()
            .filter_map(|(path, after)| {
                let before = self.baseline.get(path)?;
                changed(before, after).then(|| {
                    (
                        path,
                        (
                            after.kind,
                            matches!(after.kind, ToolFileKind::File | ToolFileKind::Symlink)
                                .then_some(after.content_digest.as_deref())
                                .flatten(),
                            if before.kind == ToolFileKind::Missing || before.mode.is_some() {
                                after.mode
                            } else {
                                None
                            },
                        ),
                    )
                })
            })
            .collect();
        Ok(crate::content_digest::digest_json(
            "a3s.code.goal-observed-workspace.v1",
            &deviations,
        )?)
    }
}

fn changed(before: &ToolFileIdentityV1, after: &ToolFileIdentityV1) -> bool {
    before.kind != after.kind
        || (matches!(before.kind, ToolFileKind::File | ToolFileKind::Symlink)
            && before.content_digest != after.content_digest)
        || (before.kind != ToolFileKind::Missing
            && before.mode.zip(after.mode).is_some_and(|(a, b)| a != b))
}

#[cfg(test)]
mod tests;
