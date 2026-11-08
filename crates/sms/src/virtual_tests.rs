#![cfg(test)]
use super::*;
fn plan() -> ProviderSendPlan {
    ProviderSendPlan {
        recipient: PhoneNumber::parse("+8613800138000").unwrap(),
        content: SmsContent::text("fixture".into()),
        binding: PreparedBinding {
            profile_id: "fixture".into(),
            profile_revision: 1,
            attempt_id: "stable".into(),
        },
    }
}
#[tokio::test]
async fn deterministic_trait_scenarios_have_no_remote_effects() {
    for scenario in [
        VirtualScenario::Accept,
        VirtualScenario::Reject,
        VirtualScenario::Unknown,
    ] {
        assert_eq!(VirtualScenario::parse(scenario.as_str()).unwrap(), scenario);
        let provider = VirtualSmsProvider::new("fixture".into(), 1, scenario).unwrap();
        let port: &dyn SmsProvider = &provider;
        let handle = port.prepare(plan()).await.unwrap();
        let result = port
            .submit(
                handle,
                SubmitContext {
                    deadline: Instant::now() + std::time::Duration::from_secs(1),
                },
            )
            .await;
        assert_eq!(
            result.kind(),
            match scenario {
                VirtualScenario::Accept => SubmissionOutcomeKind::Accepted,
                VirtualScenario::Reject => SubmissionOutcomeKind::DefinitelyNotAccepted,
                VirtualScenario::Unknown => SubmissionOutcomeKind::Indeterminate,
            }
        );
        assert_eq!(
            provider.accepted_count().unwrap(),
            usize::from(scenario == VirtualScenario::Accept)
        );
    }
    assert!(VirtualScenario::parse("anything").is_err());
}
#[tokio::test]
async fn virtual_handles_cannot_cross_instances_or_expired_deadlines() {
    let first = VirtualSmsProvider::new("fixture".into(), 1, VirtualScenario::Accept).unwrap();
    let second = VirtualSmsProvider::new("fixture".into(), 1, VirtualScenario::Accept).unwrap();
    let handle = first.prepare(plan()).await.unwrap();
    assert_eq!(
        second
            .submit(
                handle,
                SubmitContext {
                    deadline: Instant::now() + std::time::Duration::from_secs(1)
                }
            )
            .await
            .kind(),
        SubmissionOutcomeKind::DefinitelyNotAccepted
    );
    let handle = first.prepare(plan()).await.unwrap();
    assert_eq!(
        first
            .submit(
                handle,
                SubmitContext {
                    deadline: Instant::now()
                }
            )
            .await
            .kind(),
        SubmissionOutcomeKind::DefinitelyNotAccepted
    );
    assert_eq!(first.accepted_count().unwrap(), 0);
}
