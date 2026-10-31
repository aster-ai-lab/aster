//! The ACP permission ask for one tool call, with the reason it needs approval.

use super::*;
use agent_client_protocol as acp;
use aster_core::permissions::{
    InteractiveApprovalMode, InteractiveToolGuardrail, ToolRiskAssessment, ToolRiskLevel,
    ToolRiskReason,
};
use std::path::Path;

/// Where a bash call would run after this approval. Industry practice (Codex /
/// Claude Code) requires the first HITL prompt to name the boundary: sandboxed
/// default, host escalation, or a single mediated network origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecutionBoundary {
    SandboxedDefault,
    HostEscalation,
    NetworkGrant,
    Other,
}

impl ExecutionBoundary {
    pub(super) fn from_tool(tool_name: &str, args: &serde_json::Value) -> Self {
        if tool_name != "bash" {
            return Self::Other;
        }
        match args
            .get("sandbox_permissions")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("use_default")
        {
            "require_escalated" => Self::HostEscalation,
            "request_network_grant" => Self::NetworkGrant,
            _ => Self::SandboxedDefault,
        }
    }

    fn prompt_clause(self) -> Option<&'static str> {
        match self {
            Self::SandboxedDefault => {
                Some("it stays inside the workspace sandbox (no host or open network)")
            }
            Self::HostEscalation => Some("it runs on the local host without sandbox isolation"),
            Self::NetworkGrant => {
                Some("it stays sandboxed and opens exactly one approved network origin")
            }
            Self::Other => None,
        }
    }

    fn allow_label(self) -> &'static str {
        match self {
            Self::HostEscalation => "Yes, run on host",
            Self::NetworkGrant => "Yes, allow this network origin",
            Self::SandboxedDefault | Self::Other => "Yes, proceed",
        }
    }
}

pub(super) fn interactive_mode_for_posture(posture: PermissionPosture) -> InteractiveApprovalMode {
    match posture {
        PermissionPosture::Default => InteractiveApprovalMode::Default,
        PermissionPosture::Plan => InteractiveApprovalMode::Plan,
        PermissionPosture::Auto => InteractiveApprovalMode::Auto,
        PermissionPosture::Yolo => InteractiveApprovalMode::Force,
    }
}

pub(super) fn tool_permission_ask(
    session_id: &acp::SessionId,
    tool_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
    workspace: Option<&Path>,
    posture: PermissionPosture,
) -> acp::RequestPermissionRequest {
    let boundary = ExecutionBoundary::from_tool(tool_name, args);
    let raw_input = prepared_raw_input(tool_name, args);
    let why = approval_reason(&assess(tool_name, args, workspace, posture), boundary, args);
    let mut fields = acp::ToolCallUpdateFields::new()
        .title(permission_title(tool_name, boundary))
        .kind(tool_kind_for(tool_name))
        .status(acp::ToolCallStatus::Pending)
        .content(vec![acp::ToolCallContent::from(acp::ContentBlock::Text(
            acp::TextContent::new(why),
        ))]);
    if let Some(raw_input) = raw_input {
        fields = fields.raw_input(raw_input);
    }
    let mut options = vec![acp::PermissionOption::new(
        acp::PermissionOptionId::new("allow-once"),
        boundary.allow_label(),
        acp::PermissionOptionKind::AllowOnce,
    )];
    // Host escalation and one-origin grants are scoped approvals. Do not offer
    // "don't ask again" for them — that would widen beyond the disclosed boundary.
    let learnable =
        if matches!(boundary, ExecutionBoundary::SandboxedDefault) && tool_name == "bash" {
            args.get("command")
                .and_then(serde_json::Value::as_str)
                .and_then(crate::approval_learning::learnable_rule)
        } else {
            None
        };
    if let Some(rule) = learnable {
        options.push(acp::PermissionOption::new(
            acp::PermissionOptionId::new("allow-always"),
            format!("Yes, and don't ask again for `{rule}` in this workspace"),
            acp::PermissionOptionKind::AllowAlways,
        ));
    }
    options.push(acp::PermissionOption::new(
        acp::PermissionOptionId::new("reject-once"),
        "No",
        acp::PermissionOptionKind::RejectOnce,
    ));
    acp::RequestPermissionRequest::new(
        session_id.clone(),
        acp::ToolCallUpdate::new(acp::ToolCallId::new(tool_id.to_string()), fields),
        options,
    )
}

#[cfg(test)]
pub(super) fn tool_permission_request(
    session_id: &acp::SessionId,
    tool_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
) -> acp::RequestPermissionRequest {
    tool_permission_ask(
        session_id,
        tool_id,
        tool_name,
        args,
        None,
        PermissionPosture::Default,
    )
}

fn permission_title(tool_name: &str, boundary: ExecutionBoundary) -> String {
    match boundary {
        ExecutionBoundary::HostEscalation => format!("{tool_name} (host)"),
        ExecutionBoundary::NetworkGrant => format!("{tool_name} (network grant)"),
        ExecutionBoundary::SandboxedDefault | ExecutionBoundary::Other => tool_title(tool_name),
    }
}

fn assess(
    tool_name: &str,
    args: &serde_json::Value,
    workspace: Option<&Path>,
    posture: PermissionPosture,
) -> ToolRiskAssessment {
    let mode = interactive_mode_for_posture(posture);
    match workspace {
        Some(root) => InteractiveToolGuardrail::new(mode)
            .with_workspace(root)
            .assess(tool_name, args),
        None => InteractiveToolGuardrail::new(mode).assess(tool_name, args),
    }
}

/// One sentence naming the risk level, execution boundary, and reason codes.
pub(super) fn approval_reason(
    assessment: &ToolRiskAssessment,
    boundary: ExecutionBoundary,
    args: &serde_json::Value,
) -> String {
    let reasons = assessment
        .reasons
        .iter()
        .map(|reason| reason_text(*reason))
        .collect::<Vec<_>>();
    let mut because = if reasons.is_empty() {
        "the session's permission policy asks before this call".to_string()
    } else {
        reasons.join("; ")
    };
    if let Some(clause) = boundary.prompt_clause() {
        because.push_str("; ");
        because.push_str(clause);
    }
    if let Some(origin) = network_grant_origin(args) {
        because.push_str("; origin ");
        because.push_str(&origin);
    }
    format!(
        "Needs approval ({} risk): {because}.",
        level_text(assessment.level)
    )
}

fn network_grant_origin(args: &serde_json::Value) -> Option<String> {
    let grant = args.get("network_grant")?;
    let host = grant.get("host")?.as_str()?.trim();
    if host.is_empty() {
        return None;
    }
    match grant.get("port").and_then(serde_json::Value::as_u64) {
        Some(port) => Some(format!("{host}:{port}")),
        None => Some(host.to_string()),
    }
}

fn level_text(level: ToolRiskLevel) -> &'static str {
    match level {
        ToolRiskLevel::Routine => "routine",
        ToolRiskLevel::Bounded => "bounded",
        ToolRiskLevel::High => "high",
        ToolRiskLevel::Critical => "critical",
    }
}

fn reason_text(reason: ToolRiskReason) -> &'static str {
    match reason {
        ToolRiskReason::KnownReadOnly => "it only reads",
        ToolRiskReason::BoundedWorkspaceMutation => "it changes files inside the workspace",
        ToolRiskReason::BoundedVersionControlMutation => "it changes version-control state",
        ToolRiskReason::UnboundedCommandExecution => {
            "it runs a command whose effects are not bounded to the workspace"
        }
        ToolRiskReason::DynamicOrDelegatedExecution => "it runs dynamic or delegated code",
        ToolRiskReason::ExternalOrUnknownIntegration => {
            "it calls an external or unknown integration"
        }
        ToolRiskReason::EnvironmentSensitiveOperation => {
            "it touches network, credential, or host state"
        }
        ToolRiskReason::ForceOrDestructiveOperation => "it forces or destroys data",
        ToolRiskReason::MalformedOrUnknownOperation => "the operation could not be classified",
        ToolRiskReason::WorkspaceBoundaryEscape => "it reaches outside the workspace",
        ToolRiskReason::SymlinkBoundaryEscape => "it follows a symlink outside the workspace",
        ToolRiskReason::CatastrophicOperation => "it matches the catastrophic-operation floor",
        ToolRiskReason::CompositeInvocation => "it combines several operations",
    }
}

impl AsterCodeAgent {
    /// Session posture for HITL risk copy. Missing sessions use Default.
    pub(super) async fn session_permission_posture(
        &self,
        session_id: &acp::SessionId,
    ) -> PermissionPosture {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id.0.as_ref())
            .map(|live| {
                live.permissions
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .posture
            })
            .unwrap_or_default()
    }

    /// Ask the ACP client (pager) whether a tool may run. Fail closed when no client is wired.
    /// A pager allow is a confirmation fact. Core still projects the admitted policy, and a deny is not executed.
    pub(super) async fn request_tool_permission(
        &self,
        session_id: &acp::SessionId,
        tool_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
        workspace: &std::path::Path,
    ) -> bool {
        // The unattended launch is the operator's standing approval. It must
        // not learn a prefix into the graded workspace either.
        if self.unattended.is_some() {
            return true;
        }
        // A rule the user chose "don't ask again" for in this workspace
        // skips the round-trip; see approval_learning for what can match.
        if crate::approval_learning::is_previously_approved(workspace, tool_name, args) {
            tracing::debug!(tool = tool_name, "tool call approved by a learned rule");
            return true;
        }
        let posture = self.session_permission_posture(session_id).await;
        let request = permission_ask::tool_permission_ask(
            session_id,
            tool_id,
            tool_name,
            args,
            Some(workspace),
            posture,
        );
        let client = self.client.borrow().clone();
        let Some(client) = client else {
            return false;
        };
        let ask = client.request_permission(request);
        let response = match tokio::time::timeout(ask_deadline(), ask).await {
            Ok(result) => result,
            Err(_elapsed) => {
                // Fail closed: an unanswered ask denies the call instead of
                // parking the turn forever. The bell rang; nobody came.
                tracing::warn!(
                    session = %session_id.0,
                    tool = tool_name,
                    "permission ask timed out unanswered; denying"
                );
                return false;
            }
        };
        match response {
            Ok(response) => {
                let approved = permission_response_approved(&response);
                if approved && permission_response_always(&response) && tool_name == "bash" {
                    let command = args
                        .get("command")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    match crate::approval_learning::record_always_allowed_bash(workspace, command) {
                        Ok(Some(rule)) => tracing::info!(
                            rule = %rule,
                            "learned a bash rule for this workspace"
                        ),
                        Ok(None) => {}
                        Err(error) => tracing::warn!(
                            %error,
                            "could not persist the allow-always rule; it applies to this call only"
                        ),
                    }
                }
                approved
            }
            Err(_) => false,
        }
    }

    /// Ask the same ACP permission channel for a structured answer.
    ///
    /// Options use `RejectOnce` so a YOLO `AllowOnce` drain cannot answer.
    /// Free text with no options stays unanswered; the run remains parked.
    pub(super) async fn request_question_answer(
        &self,
        session_id: &acp::SessionId,
        question_id: &str,
        question: &str,
        options: &[String],
        allow_free_text: bool,
    ) -> Option<String> {
        let request = question_permission_request(
            session_id,
            question_id,
            question,
            options,
            allow_free_text,
        )?;
        let client = self.client.borrow().clone();
        let client = client?;
        let ask = client.request_permission(request);
        let response = match tokio::time::timeout(ask_deadline(), ask).await {
            Ok(result) => result.ok()?,
            Err(_elapsed) => {
                // Unanswered: the run keeps its existing unanswered-question
                // behavior instead of parking on the wire forever.
                tracing::warn!(
                    session = %session_id.0,
                    question_id = %question_id,
                    "question ask timed out unanswered"
                );
                return None;
            }
        };
        // A typed free-text answer rides the followup meta and outranks the
        // choice-row mapping: the selected row only names how the dialog was
        // dismissed, not what the user answered.
        if let Some(text) = followup_message(&response) {
            return Some(text.to_string());
        }
        answer_from_permission_outcome(&response.outcome, options)
    }
}

/// Whether a `request_permission` response grants the tool.
pub(super) fn permission_response_approved(response: &acp::RequestPermissionResponse) -> bool {
    match &response.outcome {
        acp::RequestPermissionOutcome::Selected(selected) => {
            // YOLO picks the first AllowOnce (`allow-once`). RejectOnce / cancel stay false.
            matches!(selected.option_id.0.as_ref(), "allow-once" | "allow-always")
        }
        acp::RequestPermissionOutcome::Cancelled => false,
        _ => false,
    }
}

/// Whether the user answered "don't ask again". Only this answer learns.
pub(super) fn permission_response_always(response: &acp::RequestPermissionResponse) -> bool {
    matches!(
        &response.outcome,
        acp::RequestPermissionOutcome::Selected(selected)
            if selected.option_id.0.as_ref() == "allow-always"
    )
}

/// The free text a user typed into a permission dialog's followup input; the
/// pager attaches it as `followup_message` response meta (its
/// `dispatch_permission_followup` path). Trimmed; empty/absent yields `None`.
pub(super) fn followup_message(response: &acp::RequestPermissionResponse) -> Option<&str> {
    response
        .meta
        .as_ref()
        .and_then(|meta| meta.get("followup_message"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// Question choices are not permission grants. `RejectOnce` keeps YOLO from
/// selecting them via its `AllowOnce` drain.
pub(super) fn question_permission_options(options: &[String]) -> Vec<acp::PermissionOption> {
    options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            let text = option.trim();
            if text.is_empty() {
                return None;
            }
            Some(acp::PermissionOption::new(
                acp::PermissionOptionId::new(format!("question-{index}")),
                text.to_string(),
                acp::PermissionOptionKind::RejectOnce,
            ))
        })
        .collect()
}

pub(super) fn claim_question_prompt(
    seen: &mut std::collections::HashSet<String>,
    question_id: &str,
) -> bool {
    seen.insert(question_id.to_string())
}

pub(super) fn question_permission_request(
    session_id: &acp::SessionId,
    question_id: &str,
    question: &str,
    options: &[String],
    allow_free_text: bool,
) -> Option<acp::RequestPermissionRequest> {
    let mut choices = question_permission_options(options);
    if choices.is_empty() {
        // A free-text-only ask still needs a dialog to type into: one row
        // that carries no answer of its own (its index maps past the empty
        // options list in `answer_from_permission_outcome`, so a bare Enter
        // resolves the dialog without fabricating an answer).
        if !allow_free_text {
            return None;
        }
        choices = vec![acp::PermissionOption::new(
            acp::PermissionOptionId::new("question-0"),
            "Answer in text".to_owned(),
            acp::PermissionOptionKind::RejectOnce,
        )];
    }
    Some(
        acp::RequestPermissionRequest::new(
            session_id.clone(),
            acp::ToolCallUpdate::new(
                acp::ToolCallId::new(question_id.to_string()),
                acp::ToolCallUpdateFields::new()
                    .title(question.to_string())
                    .kind(acp::ToolKind::Other)
                    .status(acp::ToolCallStatus::Pending),
            ),
            choices,
        )
        // The marker lets a client render choices by name (they all ride the
        // RejectOnce kind) and offer the free-text row only when allowed.
        .meta(
            serde_json::json!({
                QUESTION_META_KEY: {
                    "allowFreeText": allow_free_text,
                    "hasChoices": question_has_choices(options),
                },
            })
            .as_object()
            .cloned(),
        ),
    )
}

/// A choice opens the permission dialog. Blank options do not.
pub(super) fn question_has_choices(options: &[String]) -> bool {
    options.iter().any(|option| !option.trim().is_empty())
}

/// How long an `request_permission` ask may park before the agent fails
/// closed. Default 30 minutes (the same budget `ask_user` gives an
/// unanswered questionnaire); `ASTER_PERMISSION_ASK_TIMEOUT_SECS`
/// overrides, `0` disables the bound entirely.
pub(super) fn ask_deadline() -> std::time::Duration {
    const DEFAULT_SECS: u64 = 1800;
    let configured = std::env::var("ASTER_PERMISSION_ASK_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok());
    let secs = match configured {
        // Explicit disable: no bound.
        Some(0) => u64::MAX,
        Some(secs) => secs,
        None => DEFAULT_SECS,
    };
    std::time::Duration::from_secs(secs)
}

pub(super) fn answer_from_permission_outcome(
    outcome: &acp::RequestPermissionOutcome,
    options: &[String],
) -> Option<String> {
    let acp::RequestPermissionOutcome::Selected(selected) = outcome else {
        return None;
    };
    let index = selected
        .option_id
        .0
        .as_ref()
        .strip_prefix("question-")?
        .parse::<usize>()
        .ok()?;
    let text = options.get(index)?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod execution_boundary_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bash_sandbox_permissions_select_the_closed_execution_boundary() {
        assert_eq!(
            ExecutionBoundary::from_tool("bash", &json!({"command": "ls"})),
            ExecutionBoundary::SandboxedDefault
        );
        assert_eq!(
            ExecutionBoundary::from_tool(
                "bash",
                &json!({"command": "ls", "sandbox_permissions": "use_default"})
            ),
            ExecutionBoundary::SandboxedDefault
        );
        assert_eq!(
            ExecutionBoundary::from_tool(
                "bash",
                &json!({
                    "command": "docker ps",
                    "sandbox_permissions": "require_escalated",
                    "justification": "needs docker socket"
                })
            ),
            ExecutionBoundary::HostEscalation
        );
        assert_eq!(
            ExecutionBoundary::from_tool(
                "bash",
                &json!({
                    "command": "curl https://example.com",
                    "sandbox_permissions": "request_network_grant",
                    "network_grant": {"host": "example.com", "port": 443}
                })
            ),
            ExecutionBoundary::NetworkGrant
        );
        assert_eq!(
            ExecutionBoundary::from_tool("write", &json!({"path": "a.rs"})),
            ExecutionBoundary::Other
        );
    }

    #[test]
    fn permission_ask_discloses_host_and_network_boundaries_without_allow_always() {
        let session_id = acp::SessionId::new("s");
        let host = tool_permission_ask(
            &session_id,
            "t1",
            "bash",
            &json!({
                "command": "docker ps",
                "sandbox_permissions": "require_escalated",
                "justification": "needs docker"
            }),
            None,
            PermissionPosture::Default,
        );
        assert_eq!(host.tool_call.fields.title.as_deref(), Some("bash (host)"));
        let host_why = permission_content_text(&host);
        assert!(
            host_why.contains("local host without sandbox isolation"),
            "{host_why}"
        );
        assert_eq!(host.options[0].name, "Yes, run on host");
        assert!(host
            .options
            .iter()
            .all(|option| option.kind != acp::PermissionOptionKind::AllowAlways));

        let network = tool_permission_ask(
            &session_id,
            "t2",
            "bash",
            &json!({
                "command": "curl https://crates.io",
                "sandbox_permissions": "request_network_grant",
                "network_grant": {"host": "crates.io", "port": 443}
            }),
            None,
            PermissionPosture::Auto,
        );
        assert_eq!(
            network.tool_call.fields.title.as_deref(),
            Some("bash (network grant)")
        );
        let network_why = permission_content_text(&network);
        assert!(
            network_why.contains("exactly one approved network origin"),
            "{network_why}"
        );
        assert!(
            network_why.contains("origin crates.io:443"),
            "{network_why}"
        );
        assert_eq!(network.options[0].name, "Yes, allow this network origin");
        assert!(network
            .options
            .iter()
            .all(|option| option.kind != acp::PermissionOptionKind::AllowAlways));
    }

    #[test]
    fn sandboxed_learnable_bash_still_offers_allow_always() {
        let session_id = acp::SessionId::new("s");
        let request = tool_permission_ask(
            &session_id,
            "t3",
            "bash",
            &json!({"command": "git status --short"}),
            None,
            PermissionPosture::Default,
        );
        let why = permission_content_text(&request);
        assert!(
            why.contains("workspace sandbox (no host or open network)"),
            "{why}"
        );
        assert_eq!(request.options[0].name, "Yes, proceed");
        assert!(request
            .options
            .iter()
            .any(|option| option.kind == acp::PermissionOptionKind::AllowAlways));
    }

    #[test]
    fn posture_maps_onto_the_interactive_approval_closed_set() {
        assert_eq!(
            interactive_mode_for_posture(PermissionPosture::Default),
            InteractiveApprovalMode::Default
        );
        assert_eq!(
            interactive_mode_for_posture(PermissionPosture::Plan),
            InteractiveApprovalMode::Plan
        );
        assert_eq!(
            interactive_mode_for_posture(PermissionPosture::Auto),
            InteractiveApprovalMode::Auto
        );
        assert_eq!(
            interactive_mode_for_posture(PermissionPosture::Yolo),
            InteractiveApprovalMode::Force
        );
    }

    fn permission_content_text(request: &acp::RequestPermissionRequest) -> String {
        let content = request
            .tool_call
            .fields
            .content
            .as_ref()
            .expect("permission content");
        content
            .iter()
            .filter_map(|block| match block {
                acp::ToolCallContent::Content(block) => match &block.content {
                    acp::ContentBlock::Text(text) => Some(text.text.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
