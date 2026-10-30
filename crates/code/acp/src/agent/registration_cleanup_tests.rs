#![cfg(test)]

use super::*;
use a3s_code_core::capability::{
    CapabilityAdapterError, CapabilityContribution, CapabilityDescriptor, CapabilityEffect,
    CapabilityEffectError, CapabilityKind, CapabilityProjectionAdapter, CapabilitySet,
    CapabilitySource, CapabilityValue, CodeCatalogGeneration, PreparedCapability,
    SessionCapabilityBatch, Sha256Digest,
};
use a3s_code_core::tools::Tool;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

struct OwnedRegistration {
    tool: Arc<dyn Tool>,
    releases: Arc<AtomicUsize>,
}

#[async_trait]
impl CapabilityProjectionAdapter for OwnedRegistration {
    async fn prepare(
        self: Box<Self>,
        _: CancellationToken,
    ) -> Result<PreparedCapability, CapabilityAdapterError> {
        let mut prepared = PreparedCapability::new(CapabilityValue::Tool(self.tool));
        prepared.push_effect(RegistrationResource(self.releases))?;
        Ok(prepared)
    }
}

struct RegistrationResource(Arc<AtomicUsize>);

#[async_trait]
impl CapabilityEffect for RegistrationResource {
    fn name(&self) -> &str {
        "test.registration-resource"
    }

    async fn close(self: Box<Self>) -> Result<(), CapabilityEffectError> {
        tokio::task::yield_now().await;
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

async fn own_registration_resource(
    session: &AgentSession,
    tool: Arc<dyn Tool>,
    releases: Arc<AtomicUsize>,
) {
    let digest = Sha256Digest::new(format!("sha256:{}", "1".repeat(64))).unwrap();
    let source = CapabilitySource::host("registration-cleanup", digest.clone()).unwrap();
    let descriptor = CapabilityDescriptor::new(
        &source,
        CapabilityKind::Tool,
        tool.name(),
        tool.name(),
        digest,
        [],
    )
    .unwrap();
    let id = descriptor.id().clone();
    let contribution = CapabilityContribution::new(source, [descriptor]).unwrap();
    let target =
        CapabilitySet::from_contributions(CodeCatalogGeneration::new(1), [contribution]).unwrap();
    let mut batch = SessionCapabilityBatch::new(target).unwrap();
    batch
        .stage(id, OwnedRegistration { tool, releases })
        .unwrap();
    session
        .apply_capability_batch(batch, CancellationToken::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn native_registration_failure_closes_and_settles_the_open_session() {
    for failure in [
        "workspace_unavailable",
        "office_read",
        "office_file",
        "output_check",
    ] {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let workspace = std::fs::canonicalize(workspace).unwrap();
        let mut launch = test_launch(Some(root.path().join("sessions")));
        launch.workspace = workspace.clone();
        launch.config.memory_dir = Some(root.path().join("memory"));
        let model = launch.model_id.clone();
        let agent = A3sCodeAgent::new(launch);
        let live = agent
            .open_core_session(
                "registration-cleanup",
                SessionSetup {
                    workspace: workspace.clone(),
                    model_id: &model,
                    effort: None,
                    host: crate::host_context::StudioHost {
                        office_tools: true,
                        ..Default::default()
                    },
                    components: &[],
                    mcp_servers: Vec::new(),
                },
                false,
            )
            .await
            .unwrap();
        let session = Arc::clone(&live.session);
        let cancellation = session.session_cancel_token();
        let releases = Arc::new(AtomicUsize::new(0));
        let tool: Arc<dyn Tool> = if failure == "office_read" {
            Arc::new(crate::office_read::OfficeReadTool::new(&workspace).unwrap())
        } else if failure == "office_file" {
            Arc::new(crate::office_file::OfficeFileTool::new(&workspace).unwrap())
        } else {
            Arc::new(crate::output_check::OutputCheckTool)
        };
        own_registration_resource(&session, tool, Arc::clone(&releases)).await;
        assert!(!session.is_closed());
        assert_eq!(releases.load(Ordering::SeqCst), 0);
        if failure == "workspace_unavailable" {
            std::fs::remove_dir_all(&workspace).unwrap();
        }
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            agent.finish_plugin_tools(live, Vec::new()),
        )
        .await
        .expect("registration failure must settle before its deadline");
        let error = match result {
            Err(error) => error,
            Ok(_) => {
                session.close().await;
                panic!("accepted {failure}");
            }
        };
        let closed_before_fixture_cleanup = session.is_closed();
        let cancelled_before_fixture_cleanup = cancellation.is_cancelled();
        let settled_before_fixture_cleanup = releases.load(Ordering::SeqCst);
        // Keep the session Arc alive and clean up even when this regression fails.
        session.close().await;
        println!(
            "REGISTRATION_CLEANUP {}",
            serde_json::json!({
                "failure": failure,
                "closed": closed_before_fixture_cleanup,
                "cancelled": cancelled_before_fixture_cleanup,
                "resources_settled": settled_before_fixture_cleanup,
                "error": format!("{error:?}"),
            })
        );
        assert!(closed_before_fixture_cleanup, "unclosed {failure}");
        assert!(cancelled_before_fixture_cleanup, "uncancelled {failure}");
        assert_eq!(settled_before_fixture_cleanup, 1, "unsettled {failure}");
        assert_eq!(releases.load(Ordering::SeqCst), 1, "duplicate cleanup");
        if failure != "workspace_unavailable" {
            assert!(format!("{error:?}").contains(failure));
        }
    }
}
