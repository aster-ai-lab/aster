#![cfg(test)]

use super::*;

#[test]
fn ordinary_turns_receive_delivery_checks_without_user_host_context() {
    let host = host_for_session("task", None).expect("default host requirements");
    assert_eq!(host["guidelines"], DELIVERY_CONTRACT.trim());
    assert!(
        host["guidelines"]
            .as_str()
            .unwrap()
            .contains("ToWork delivery requirements")
    );
    assert!(host.get("role").is_none());
}

#[test]
fn new_sessions_carry_content_and_file_scope_checks_beside_caller_instructions() {
    let host = host_for_session(
        "task",
        Some(&json!({"guidelines": "Follow the supplied material."})),
    )
    .expect("host requirements");
    let params = session_new_params("fixture-workspace", &[], &[], Some(&host), &[], None);
    let guidelines = params["_meta"]["a3sHost"]["guidelines"]
        .as_str()
        .expect("standing host instructions");

    assert!(guidelines.starts_with("Follow the supplied material.\n\n"));
    assert_eq!(guidelines.matches(DELIVERY_CONTRACT.trim()).count(), 1);
    for required in [
        "number together with its counting word or measurement unit",
        "including their original numeric spelling",
        "facts retained in summaries and rewrites too",
        "unless the user explicitly requests that particular conversion or change",
        "Create only the deliverable files and worksheets the user requested",
        "validate the planned contents before publication",
        "keep unrequested source copies and intermediate drafts out of final delivery",
        "Do not turn goals, targets or plans into completed events or actual periods",
        "Label assumptions “待核实” as unverified hypotheses",
        "do not imply supporting source evidence or confirmed causes",
        "Verify every derived numerical conclusion with available tools",
        "retaining source inputs and formula",
        "Compare the same periods, coverage, units and exclusions",
        "do not compare unmatched bases",
        "Format derived numbers without thousands grouping separators",
        "unless explicitly required by the user or source",
        "Use “事实” only for direct source observations; label your deductions as analysis",
        "Put explanations and missing-value lists in the reply unless requested as worksheets",
        "Do not create unrequested probe files",
        "Keep the answer concise; omit unrequested analysis and tool details unless needed to explain a limitation",
        "Check each in the reply and files before finishing",
        "infer missing components or causes from totals alone",
    ] {
        assert!(
            guidelines.contains(required),
            "missing delivery rule: {required}"
        );
    }
}

#[test]
fn long_host_text_cannot_truncate_delivery_checks_at_the_acp_boundary() {
    let host = host_for_session(
        "task",
        Some(&json!({"guidelines": format!("用户约束\0{}", "字".repeat(8000))})),
    )
    .expect("host");
    let guidelines = host["guidelines"].as_str().expect("guidelines");
    assert_eq!(
        guidelines.chars().count(),
        8000 + 2 + DELIVERY_CONTRACT.trim().chars().count()
    );
    assert!(guidelines.chars().count() <= 12000);
    assert!(guidelines.starts_with("用户约束字"));
    assert!(!guidelines.contains('\0'));
    assert!(guidelines.ends_with(DELIVERY_CONTRACT.trim()));
}
