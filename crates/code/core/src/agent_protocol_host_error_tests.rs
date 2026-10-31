use super::*;

#[test]
fn cancellation_runtime_code_requires_the_typed_source() {
    let typed = AgentProtocolHostError::Code(CodeError::Internal(anyhow::Error::new(
        RunCancellationPending {
            run_id: "pending-run".into(),
        },
    )));
    assert_eq!(typed.code(), "INTERNAL_ERROR");
    assert_eq!(
        typed.runtime_code(),
        "aster.code.agent_protocol.run_cancellation_pending"
    );
    let prose = AgentProtocolHostError::Code(CodeError::Internal(anyhow::anyhow!(
        "Run 'pending-run' cancellation is pending original worker retirement"
    )));
    assert_eq!(prose.runtime_code(), "INTERNAL_ERROR");
    let cancelled = AgentProtocolHostError::Code(CodeError::run_cancelled("other-run".into()));
    assert_eq!(cancelled.runtime_code(), "RUN_CANCELLED");
    assert_eq!(
        AgentProtocolHostError::RunUnavailable.runtime_code(),
        AgentProtocolHostError::RunUnavailable.code()
    );
}
