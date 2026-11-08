#![cfg(test)]

use super::*;

struct Identity;
impl RegistryTargetIdentity for Identity {
    fn package_id(&self) -> &str {
        "acme/research"
    }
    fn version(&self) -> &str {
        "2.0.0"
    }
    fn channel(&self) -> &str {
        "stable"
    }
    fn target(&self) -> &str {
        "linux-x86_64"
    }
}

#[test]
fn aix_registry_target_keeps_the_exact_package_version_channel_and_platform_binding() {
    let target =
        TargetName::new("extensions/acme/research/2.0.0/stable/linux-x86_64/research.aix").unwrap();
    validate_target_name(&target, &Identity).unwrap();
    for path in [
        "extensions/acme/other/2.0.0/stable/linux-x86_64/research.aix",
        "extensions/acme/research/3.0.0/stable/linux-x86_64/research.aix",
        "extensions/acme/research/2.0.0/nightly/linux-x86_64/research.aix",
        "extensions/acme/research/2.0.0/stable/windows-x86_64/research.aix",
        "extensions/acme/research/2.0.0/stable/linux-x86_64/research.vsix",
    ] {
        assert_eq!(
            validate_target_name(&TargetName::new(path).unwrap(), &Identity)
                .unwrap_err()
                .code,
            "use.extension.registry_target_invalid"
        );
    }
}
