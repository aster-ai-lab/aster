//! A foreground verifier temporarily owns the parent's claimed files.

use super::{canonical_root, dirty};
use std::path::{Path, PathBuf};

pub(crate) struct WriteClaimLoan {
    parent: String,
    child: String,
    paths: Vec<PathBuf>,
}

impl WriteClaimLoan {
    pub(crate) fn new(parent: &str, child: &str, workspace: &Path) -> Result<Self, String> {
        super::refuse_foreign_workspace_owner(Some(parent), workspace)?;
        let workspace = canonical_root(workspace);
        let mut claims = dirty()
            .lock()
            .map_err(|_| "write claims are unavailable".to_string())?;
        if let Some((path, owner)) = claims
            .iter()
            .find(|(path, owner)| owner.as_str() != parent && path.starts_with(&workspace))
        {
            return Err(format!(
                "concurrent write to {} is already owned by session {owner}",
                path.display()
            ));
        }
        let paths = claims
            .iter_mut()
            .filter_map(|(path, owner)| {
                if owner == parent && path.starts_with(&workspace) {
                    *owner = child.to_string();
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();
        Ok(Self {
            parent: parent.to_string(),
            child: child.to_string(),
            paths,
        })
    }
}

impl Drop for WriteClaimLoan {
    fn drop(&mut self) {
        // Claim transitions run no callbacks; poisoning breaks the ownership invariant.
        let mut claims = dirty().lock().expect("dirty paths");
        for path in &self.paths {
            if claims.get(path) == Some(&self.child) {
                claims.insert(path.clone(), self.parent.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests;
