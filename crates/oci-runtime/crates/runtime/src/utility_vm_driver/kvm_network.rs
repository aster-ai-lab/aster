use std::ffi::CString;
use std::path::Path;

use aster_oci_agent_protocol::{
    AgentVmAttachmentManifest, AgentVmMacAddress, AgentVmNetworkAttachment,
    AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME,
};
use aster_oci_sdk::{Error, ErrorCode, NetworkAttachment, Result};
use serde_json::Value;

use super::UtilityVmLaunchRequest;

use super::vm_attachment_transport::{
    ensure_absent, ensure_dedicated_bundle_path, persist_manifest, PENDING_MANIFEST_FILE_NAME,
};

mod storage;

const TUN_FLAG: u32 = 0x0001;
const TAP_FLAG: u32 = 0x0002;

/// Prepare immutable raw-storage and TAP transport evidence before VM entry.
pub(crate) async fn prepare(request: &UtilityVmLaunchRequest<'_>) -> Result<Option<String>> {
    let final_path = request
        .runtime_share
        .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
    let pending_path = request.runtime_share.join(PENDING_MANIFEST_FILE_NAME);
    if request.attachment_contract.network_attachments().is_empty()
        && request.attachment_contract.storage().is_empty()
    {
        ensure_absent(&final_path, &pending_path).await?;
        return Ok(None);
    }
    if request.attachment_contract.guest_session().is_some() {
        return Err(attachment_error(
            ErrorCode::Unsupported,
            "KVM storage and network devices cannot be hot-plugged into a reusable Guest session",
        ));
    }
    request.attachment_contract.validate(request.bundle)?;

    let configuration: Value =
        serde_json::from_str(request.bundle.config_json()).map_err(|error| {
            attachment_error(
                ErrorCode::Internal,
                format!("validated OCI configuration could not be decoded: {error}"),
            )
        })?;
    let attachment_digest = request.attachment_contract.digest()?;
    let mut network = Vec::with_capacity(request.attachment_contract.network_attachments().len());
    for attachment in request.attachment_contract.network_attachments() {
        let tap_name = tap_name(&configuration, attachment)?;
        verify_host_tap(&tap_name).await?;
        let mac_address =
            AgentVmMacAddress::derive(&attachment_digest, attachment.identity(), &tap_name)?;
        network.push(AgentVmNetworkAttachment::new(
            attachment.identity().clone(),
            tap_name,
            attachment.namespace().clone(),
            attachment.interface().clone(),
            attachment.cleanup(),
            mac_address,
        )?);
    }
    network.sort();

    let mut storage = Vec::with_capacity(request.attachment_contract.storage().len());
    for attachment in request.attachment_contract.storage() {
        storage
            .push(storage::prepare(request, &configuration, attachment, &attachment_digest).await?);
    }
    storage.sort();

    let manifest = AgentVmAttachmentManifest::new(
        request.target.clone(),
        request.guest_bundle.clone(),
        request.bundle.config_digest(),
        attachment_digest,
        network,
        storage,
    )?;
    manifest.validate_bundle(request.bundle)?;
    ensure_dedicated_bundle_path(&manifest)?;
    persist_manifest(request.runtime_share, &manifest).await?;
    manifest.digest().map(Some)
}

fn tap_name(configuration: &Value, attachment: &NetworkAttachment) -> Result<String> {
    let devices = configuration
        .pointer("/linux/netDevices")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            attachment_error(
                ErrorCode::InvalidArgument,
                "KVM network attachment requires linux.netDevices",
            )
        })?;
    devices
        .keys()
        .find(|name| {
            format!("/linux/netDevices/{}", escape_json_pointer(name))
                == attachment.interface().json_pointer()
        })
        .cloned()
        .ok_or_else(|| {
            attachment_error(
                ErrorCode::InvalidArgument,
                format!(
                    "authorized KVM network interface does not select {}",
                    attachment.interface().json_pointer()
                ),
            )
        })
}

async fn verify_host_tap(name: &str) -> Result<()> {
    let name_c = CString::new(name).map_err(|error| {
        attachment_error(
            ErrorCode::InvalidArgument,
            format!("KVM TAP name is not a C string: {error}"),
        )
    })?;
    // SAFETY: `name_c` is NUL-terminated and remains live for the complete call.
    let index = unsafe { libc::if_nametoindex(name_c.as_ptr()) };
    if index == 0 {
        return Err(attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "authorized KVM TAP {name} is not visible in the runtime network namespace: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }

    let flags_path = Path::new("/sys/class/net").join(name).join("tun_flags");
    let encoded = tokio::fs::read_to_string(&flags_path)
        .await
        .map_err(|error| {
            attachment_error(
                ErrorCode::FailedPrecondition,
                format!(
                "authorized KVM interface {name} is not a readable TUN/TAP device at {}: {error}",
                flags_path.display()
            ),
            )
        })?;
    let encoded = encoded.trim();
    let flags = encoded
        .strip_prefix("0x")
        .map_or_else(
            || encoded.parse::<u32>(),
            |hex| u32::from_str_radix(hex, 16),
        )
        .map_err(|error| {
            attachment_error(
                ErrorCode::FailedPrecondition,
                format!("KVM TAP {name} exposes invalid tun_flags {encoded:?}: {error}"),
            )
        })?;
    if flags & (TUN_FLAG | TAP_FLAG) != TAP_FLAG {
        return Err(attachment_error(
            ErrorCode::FailedPrecondition,
            format!("authorized KVM interface {name} is not a TAP device"),
        ));
    }
    Ok(())
}

fn escape_json_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn attachment_error(code: ErrorCode, message: impl Into<String>) -> Error {
    Error::new(code, message).for_operation("prepare-kvm-vm-attachments")
}

#[cfg(test)]
#[path = "kvm_network/tests.rs"]
mod tests;
