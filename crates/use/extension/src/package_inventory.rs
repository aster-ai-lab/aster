//! Validate manifest references against regular files before a host publishes a package.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{validate_relative_path, ExtensionManifest};
use aster_use_core::{UseError, UseResult};

impl ExtensionManifest {
    /// File inventory comes from the host's bounded, link-free archive reader.
    /// This checks references, not content, executable permissions, or host grants.
    pub fn validate_file_inventory(&self, files: &BTreeSet<PathBuf>) -> UseResult<()> {
        for path in files {
            validate_relative_path(path)?;
        }
        self.validate_package_root(Path::new("."))?;
        let required = [Path::new("aster-use-extension.acl"), Path::new("README.md")];
        let mut references = required.to_vec();
        for tool in &self.tools {
            references.extend(tool.package_paths());
        }
        for mcp in &self.mcp_servers {
            references.extend(mcp.package_paths());
        }
        for flow in &self.flows {
            references.extend(flow.package_paths());
        }
        references.extend(self.skills.iter().map(|skill| skill.path.as_path()));
        for agent in &self.agents {
            references.extend(agent.package_paths());
        }
        for ui in &self.ui {
            references.extend(ui.package_paths());
        }
        for path in references {
            if !files.contains(path) {
                return Err(UseError::new(
                    "use.extension.package_file_missing",
                    format!("Package is missing declared file '{}'.", path.display()),
                ));
            }
        }
        for root in self.okf.iter().map(|okf| Path::new(&okf.bundle.root)) {
            if !files
                .iter()
                .any(|path| path != root && path.starts_with(root))
            {
                return Err(UseError::new(
                    "use.extension.package_file_missing",
                    format!(
                        "Package is missing OKF bundle directory '{}'.",
                        root.display()
                    ),
                ));
            }
        }
        // A regular file cannot also be an ancestor of another regular file.
        for path in files {
            if path
                .ancestors()
                .skip(1)
                .any(|parent| files.contains(parent))
            {
                return Err(UseError::new(
                    "use.extension.package_layout_invalid",
                    format!(
                        "Package file '{}' has a file as an ancestor.",
                        path.display()
                    ),
                ));
            }
        }
        Ok(())
    }
}
