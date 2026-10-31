//! Real public-SDK qualification of caller-owned macOS workspace directories.

use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use aster_oci_sdk::{
    runtime_bundle_handoff_directory, ContainerId, ContainerTarget, CreateAttachments,
    CreateRequest, DeleteMode, DeleteRequest, IsolationRequest, LocalIpcEndpoint, OciBundle,
    OperationContext, OperationId, ProcessId, ProcessIo, ProcessTarget, ReadOutputRequest,
    RuntimeClient, StartRequest, StorageAccessMode, StorageAttachmentId, StorageCleanup,
    StorageOwnership, StoragePathRestriction, StoragePathRestrictions, WaitRequest,
    RUNTIME_BUNDLE_HANDOFF_EXTENSION, RUNTIME_BUNDLE_HANDOFF_EXTERNAL_STORAGE_VERSION,
    RUNTIME_BUNDLE_HANDOFF_MOVE_V2, STORAGE_PATH_RESTRICTIONS_EXTENSION,
    STORAGE_PATH_RESTRICTIONS_VERSION,
};
use serde_json::json;
use tokio::time::{sleep, timeout, Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

type QualificationResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> QualificationResult<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [host_binary, shim, image, fixture, root] = args.as_slice() else {
        return Err(
            "usage: qualify_hvf_workspace <host> <shim> <manifest> <fixture> <private-work-root>"
                .into(),
        );
    };
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err("this qualification requires Apple Silicon macOS".into());
    }
    let root = root.canonicalize()?;
    let service = root.join("service");
    let mut host = Command::new(host_binary)
        .args(["macos-hvf-host-service", "--root"])
        .arg(&service)
        .arg("--shim")
        .arg(shim)
        .arg("--system-image-manifest")
        .arg(image)
        .stdin(Stdio::null())
        .stdout(File::create(root.join("host.stdout.log"))?)
        .stderr(File::create(root.join("host.stderr.log"))?)
        .spawn()?;
    let result = timeout(Duration::from_secs(180), qualify(&service, fixture, &root)).await;
    if host.try_wait()?.is_none() {
        host.kill()?;
    }
    host.wait()?;
    let evidence = result??;
    fs::write(
        root.join("workspace-report.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    println!("{}", serde_json::to_string(&evidence)?);
    Ok(())
}

async fn qualify(
    service: &Path,
    fixture: &Path,
    root: &Path,
) -> QualificationResult<serde_json::Value> {
    let endpoint = LocalIpcEndpoint::unix_socket(service.join("runtime.sock"))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let client = loop {
        if let Ok(Ok(client)) =
            timeout(Duration::from_secs(1), RuntimeClient::connect(&endpoint)).await
        {
            break client;
        }
        if Instant::now() >= deadline {
            return Err("Host Service did not start; inspect host.stderr.log".into());
        }
        sleep(Duration::from_millis(50)).await;
    };
    let features = client.features().await?;
    if !features.attachments.supports_extension(
        RUNTIME_BUNDLE_HANDOFF_EXTENSION,
        RUNTIME_BUNDLE_HANDOFF_EXTERNAL_STORAGE_VERSION,
    ) {
        return Err("Host Service does not advertise external caller storage handoff".into());
    }
    if !features.attachments.supports_extension(
        STORAGE_PATH_RESTRICTIONS_EXTENSION,
        STORAGE_PATH_RESTRICTIONS_VERSION,
    ) {
        return Err("Host Service does not advertise Host storage path restrictions".into());
    }
    let readonly = root.join("只读工作区 \"alpha\"\\beta");
    let writable = root.join("读写工作区");
    for path in [&readonly, &writable] {
        fs::create_dir(path)?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        fs::write(path.join("input"), b"caller-owned")?;
    }
    fs::write(writable.join(".env"), b"host-credential")?;
    fs::hard_link(writable.join(".env"), writable.join("credential-alias"))?;
    fs::create_dir_all(writable.join(".aster/loops"))?;
    fs::write(writable.join(".aster/config"), b"host-control")?;
    let restrictions = StoragePathRestrictions::new(vec![StoragePathRestriction::new(
        StorageAttachmentId::new("caller-directory-1")?,
        vec![".env".into(), "credential-alias".into()],
        vec![".env".into(), "credential-alias".into(), ".aster".into()],
        vec![".aster/loops".into()],
    )?])?;
    let id = ContainerId::new("workspace-qualification")?;
    let create_context = operation("workspace-create")?;
    let runtime = service.join("runtime");
    let directory = runtime_bundle_handoff_directory(&runtime, &id, &create_context.operation_id)?;
    fs::create_dir_all(directory.parent().ok_or("handoff has no parent")?)?;
    #[cfg(unix)]
    for path in directory
        .ancestors()
        .skip(1)
        .take_while(|path| *path != runtime)
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    if !Command::new("/usr/bin/ditto")
        .arg(fixture)
        .arg(&directory)
        .status()?
        .success()
    {
        return Err("failed to stage Linux fixture bundle".into());
    }
    #[cfg(unix)]
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let config_path = directory.join("config.json");
    let mut config: serde_json::Value = serde_json::from_slice(&fs::read(&config_path)?)?;
    config["annotations"][RUNTIME_BUNDLE_HANDOFF_EXTENSION] = json!(RUNTIME_BUNDLE_HANDOFF_MOVE_V2);
    config["annotations"][STORAGE_PATH_RESTRICTIONS_EXTENSION] =
        json!(restrictions.to_annotation_value()?);
    config["process"]["args"] = json!([
        "/bin/sh",
        "-c",
        concat!(
            "set -eu; ",
            "test \"$(/bin/busybox cat /workspace-ro/input)\" = caller-owned; ",
            "test \"$(/bin/busybox cat /workspace-rw/input)\" = caller-owned; ",
            "printf 'guest-write' > /workspace-rw/中文.txt; ",
            "if printf denied > /workspace-ro/input 2>/dev/null; then exit 91; fi; ",
            "/bin/busybox mount -o remount,rw /workspace-ro; ",
            "if printf denied > /workspace-ro/input 2>/dev/null; then exit 92; fi; ",
            "if /bin/busybox cat /workspace-rw/.env >/dev/null 2>&1; then exit 93; fi; ",
            "if /bin/busybox cat /workspace-rw/credential-alias >/dev/null 2>&1; then exit 94; fi; ",
            "if printf denied > /workspace-rw/.env 2>/dev/null; then exit 95; fi; ",
            "if /bin/busybox mv /workspace-rw/.aster /workspace-rw/moved 2>/dev/null; then exit 96; fi; ",
            "if printf denied > /workspace-rw/.aster/config 2>/dev/null; then exit 97; fi; ",
            "printf 'loop-state' > /workspace-rw/.aster/loops/output; ",
            "printf 'workspace-contract-passed\\n'; exit 7"
        )
    ]);
    config["process"]["capabilities"] = json!({"bounding":["CAP_SYS_ADMIN", "CAP_DAC_OVERRIDE"],
        "permitted":["CAP_SYS_ADMIN", "CAP_DAC_OVERRIDE"],"effective":["CAP_SYS_ADMIN", "CAP_DAC_OVERRIDE"]});
    // Exercise a privileged Guest remount against the independent Host read-only grant.
    config["linux"]["namespaces"]
        .as_array_mut()
        .ok_or("fixture has no namespaces")?
        .retain(|namespace| namespace["type"] != "user");
    config["linux"]
        .as_object_mut()
        .ok_or("fixture has no Linux configuration")?
        .remove("uidMappings");
    config["linux"]
        .as_object_mut()
        .ok_or("fixture has no Linux configuration")?
        .remove("gidMappings");
    let mounts = config["mounts"]
        .as_array_mut()
        .ok_or("fixture has no mounts")?;
    let first = mounts.len();
    mounts.push(json!({"destination":"/workspace-ro","type":"none","source":readonly,"options":["rbind","ro","nosuid","nodev"]}));
    mounts.push(json!({"destination":"/workspace-rw","type":"none","source":writable,"options":["rbind","rw","nosuid","nodev"]}));
    let encoded = serde_json::to_string(&config)?;
    fs::write(&config_path, &encoded)?;
    #[cfg(unix)]
    fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600))?;
    fs::write(root.join("qualified-config.json"), &encoded)?;
    let bundle = OciBundle::from_json(&directory, encoded)?;
    let mut attachments = CreateAttachments::from_bundle(&bundle, ProcessIo::default())?;
    for (offset, access) in [StorageAccessMode::ReadOnly, StorageAccessMode::ReadWrite]
        .into_iter()
        .enumerate()
    {
        attachments = attachments.attach_storage_mount(
            &bundle,
            first + offset,
            StorageAttachmentId::new(format!("caller-directory-{offset}"))?,
            access,
            StorageOwnership::Caller,
            StorageCleanup::DetachOnly,
        )?;
    }
    attachments = attachments
        .with_runtime_bundle_storage_handoff(&bundle)?
        .with_storage_path_restrictions(&bundle)?;
    fs::write(
        root.join("qualified-attachments.json"),
        serde_json::to_vec_pretty(&attachments)?,
    )?;
    let create = CreateRequest {
        context: create_context,
        id: id.clone(),
        bundle,
        isolation: IsolationRequest::DedicatedVm,
        attachments,
    };
    let created = client.create(create.clone()).await?;
    if client.create(create).await? != created {
        return Err("create replay drifted".into());
    }
    let target = ContainerTarget::exact(id, created.generation);
    client
        .start(StartRequest {
            context: operation("workspace-start")?,
            target: target.clone(),
        })
        .await?;
    let status = client
        .wait(WaitRequest {
            target: target.clone(),
            timeout_ms: Some(20_000),
        })
        .await?;
    let output = client
        .read_output(ReadOutputRequest {
            process: ProcessTarget {
                container: target.clone(),
                process_id: ProcessId::init(),
            },
            after_sequence: 0,
            max_bytes: 16_384,
            wait_timeout_ms: Some(0),
        })
        .await?;
    fs::write(
        root.join("payload-output.json"),
        serde_json::to_vec_pretty(&output)?,
    )?;
    client
        .delete(DeleteRequest {
            context: operation("workspace-delete")?,
            target: target.clone(),
            mode: DeleteMode::Force,
        })
        .await?;
    if status.exit_code != Some(7) || status.signal.is_some() {
        return Err(format!("workspace payload failed: {status:?}").into());
    }
    if fs::read(readonly.join("input"))? != b"caller-owned"
        || fs::read(writable.join("input"))? != b"caller-owned"
        || fs::read(writable.join("中文.txt"))? != b"guest-write"
    {
        return Err("caller data or directory access contract was violated".into());
    }
    if fs::read(writable.join(".env"))? != b"host-credential"
        || fs::read(writable.join("credential-alias"))? != b"host-credential"
        || fs::read(writable.join(".aster/config"))? != b"host-control"
        || fs::read(writable.join(".aster/loops/output"))? != b"loop-state"
    {
        return Err(
            "Host credential, control-directory or writable exception contract failed".into(),
        );
    }
    #[cfg(unix)]
    for path in [&readonly, &writable] {
        if fs::metadata(path)?.permissions().mode() & 0o777 != 0o755 {
            return Err("runtime changed caller directory permissions".into());
        }
    }
    if runtime.join("shares").join(target.id.as_str()).exists() {
        return Err("runtime share leaked after delete".into());
    }
    Ok(
        json!({"schema_version":"aster.oci.hvf-workspace-qualification.v1", "status":"passed",
        "target":target,"exit_status":status,"guest_rw_visible_on_host":true,
        "host_ro_survives_privileged_guest_remount":true,"caller_data_preserved":true,
        "caller_permissions_preserved":true,"runtime_share_clean":true, "host_credential_and_alias_protected":true,"control_directory_pinned":true,"write_exception_usable":true}),
    )
}

fn operation(name: &str) -> aster_oci_sdk::Result<OperationContext> {
    Ok(OperationContext::new(OperationId::new(name)?))
}
