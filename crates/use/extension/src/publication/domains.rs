use std::collections::BTreeSet;
use std::path::Path;

use aster_acl::{Block, Value};
use aster_use_core::UseResult;
use tokio::fs;
use tokio::io::AsyncReadExt;

use super::invalid;
use crate::package::io_error;

const MAX_POLICY_BYTES: u64 = 256 * 1024;

pub(super) async fn admit(root: &Path, selected: &str, publisher: &str) -> UseResult<Vec<String>> {
    let path = root.join("domains.acl");
    let metadata = fs::symlink_metadata(&path)
        .await
        .map_err(|error| io_error("inspect plugin domain policy", &path, error))?;
    if aster_use_core::metadata_is_link_or_reparse_point(&metadata) || !metadata.is_file() {
        return Err(invalid("Plugin domain policy must be a regular file."));
    }
    let mut input = Vec::new();
    fs::File::open(&path)
        .await
        .map_err(|error| io_error("open plugin domain policy", &path, error))?
        .take(MAX_POLICY_BYTES + 1)
        .read_to_end(&mut input)
        .await
        .map_err(|error| io_error("read plugin domain policy", &path, error))?;
    if input.is_empty() || input.len() as u64 > MAX_POLICY_BYTES {
        return Err(invalid(
            "Plugin domain policy must contain between 1 byte and 256 KiB.",
        ));
    }
    let text = std::str::from_utf8(&input)
        .map_err(|_| invalid("Plugin domain policy must be UTF-8 ACL."))?;
    parse(text, selected, publisher)
}

fn parse(text: &str, selected: &str, publisher: &str) -> UseResult<Vec<String>> {
    let document = aster_acl::parse_acl(text)
        .map_err(|error| invalid(format!("Invalid plugin domain ACL: {error}")))?;
    let [root] = document.blocks.as_slice() else {
        return Err(invalid("Domain policy requires one plugin_domains block."));
    };
    if root.name != "plugin_domains"
        || !root.labels.is_empty()
        || root.attributes.len() != 1
        || root.attributes.get("schema_version") != Some(&Value::Number(1.0))
        || root.blocks.is_empty()
        || root.blocks.len() > 128
    {
        return Err(invalid(
            "Domain policy requires schema_version 1 and between 1 and 128 domain blocks.",
        ));
    }
    let mut names = BTreeSet::new();
    let mut admitted = None;
    for block in &root.blocks {
        let [domain] = block.labels.as_slice() else {
            return Err(invalid(
                "Each domain requires exactly one explicit identifier.",
            ));
        };
        if block.name != "domain"
            || !crate::valid_segment(domain)
            || !names.insert(domain)
            || !block.blocks.is_empty()
            || block.attributes.len() != 2
        {
            return Err(invalid(
                "Domain entries require unique native identifiers, owners and publishers.",
            ));
        }
        let owners = strings(block, "owners")?;
        let publishers = strings(block, "publishers")?;
        if publishers.iter().any(|value| !crate::valid_segment(value)) {
            return Err(invalid(
                "Domain publishers must be native publisher identifiers.",
            ));
        }
        if domain == selected && publishers.iter().any(|value| value == publisher) {
            admitted = Some(owners);
        }
    }
    admitted.ok_or_else(|| {
        invalid("The requested domain and publisher are not admitted by domains.acl.")
    })
}

fn strings(block: &Block, name: &str) -> UseResult<Vec<String>> {
    let Some(Value::List(values)) = block.attributes.get(name) else {
        return Err(invalid(format!(
            "Domain {name} must be an explicit string list."
        )));
    };
    if values.is_empty() || values.len() > 64 {
        return Err(invalid(format!(
            "Domain {name} requires between 1 and 64 identifiers."
        )));
    }
    let mut unique = BTreeSet::new();
    values
        .iter()
        .map(|value| {
            let Value::String(value) = value else {
                return Err(invalid(format!(
                    "Domain {name} contains a non-string identifier."
                )));
            };
            if value.is_empty()
                || value.len() > 256
                || value.trim() != value
                || value.chars().any(char::is_control)
                || !unique.insert(value)
            {
                return Err(invalid(format!(
                    "Domain {name} identifiers must be nonempty, unique and at most 256 bytes."
                )));
            }
            Ok(value.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publication::tests::POLICY;

    #[test]
    fn policy_refuses_unknown_fields_duplicate_domains_and_invalid_ownership() {
        for changed in [
            POLICY.replace("schema_version = 1", "schema_version = 2"),
            POLICY.replace("schema_version = 1", "schema_version = 1\n unknown = true"),
            POLICY.replace("\"development\"", "\"knowledge\""),
            POLICY.replace("\"development\"", "\"../development\""),
            POLICY.replace("owners = [\"fixture-maintainer\"]", "owners = []"),
            POLICY.replace(
                "publishers = [\"acme\"]",
                "publishers = [\"acme\", \"acme\"]",
            ),
            POLICY.replace("publishers = [\"acme\"]", "publishers = [true]"),
            POLICY.replace("publishers = [\"acme\"]", "publishers = [\"Acme\"]"),
            POLICY.replace(
                "owners = [\"fixture-other\"]",
                "owners = [\" fixture-other\"]",
            ),
        ] {
            assert_eq!(
                parse(&changed, "knowledge", "acme").unwrap_err().code,
                super::super::ERROR
            );
        }
    }

    #[tokio::test]
    async fn domain_file_admission_preserves_encoding_and_size_limits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("domains.acl");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_POLICY_BYTES + 1).unwrap();
        assert_eq!(
            admit(root.path(), "knowledge", "acme")
                .await
                .unwrap_err()
                .code,
            super::super::ERROR
        );
        std::fs::write(&path, b"\xff").unwrap();
        assert_eq!(
            admit(root.path(), "knowledge", "acme")
                .await
                .unwrap_err()
                .code,
            super::super::ERROR
        );
    }
}
