#![cfg(all(test, target_os = "macos", target_arch = "aarch64"))]

use super::{
    driver::Driver,
    fixture::{Fixture, Oracle, LOCAL_CHECK},
};
use agent_client_protocol as acp;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

pub struct RustRun {
    pub work: Result<String, &'static str>,
    pub oracle: Result<Oracle, String>,
    pub cleanup: Result<(), &'static str>,
    pub events: Vec<Value>,
    pub rpc_errors: Vec<Value>,
    pub wire_checks: Value,
    pub tools: usize,
    pub permissions: usize,
    pub prompt_ms: u128,
    pub overflow: bool,
    pub tools_terminal: bool,
}

pub fn wire_checks(events: &[Value]) -> Value {
    let checks: Vec<_> = events.iter().enumerate().filter_map(|(index,event)| {
        let update = &event["update"];
        let output = &update["rawOutput"];
        (update["sessionUpdate"] == "tool_call_update" && output["command"] == LOCAL_CHECK && output["exit_code"].is_i64()).then(|| json!({"event":index,"exit_code":output["exit_code"],"status":update["status"],"output":output["output"]}))
    }).collect();
    let initial_failed = checks.first().is_some_and(|check| {
        check["exit_code"].as_i64().is_some_and(|code| code != 0)
            && check["output"]
                .as_str()
                .is_some_and(|text| text.contains("test result: FAILED"))
    });
    let final_passed = checks.last().is_some_and(|check| {
        check["exit_code"] == 0
            && check["output"].as_str().is_some_and(|text| {
                super::fixture::tests_passed(
                    text,
                    &[
                        "tests::formats_cents",
                        "tests::quantity_rejects_zero",
                        "tests::rounds_half_up",
                    ],
                )
            })
    });
    let verified = events.iter().any(|event| {
        event["method"] == "a3s/turn/outcome"
            && event["params"]["outcome"]["terminal"] == "verified"
            && event["params"]["outcome"]["status"] == "passed"
    });
    json!({"initial_failed_check":initial_failed,"final_passing_check":final_passed,"bound_verified_outcome":verified,"exact_checks":checks})
}

impl RustRun {
    pub async fn execute(
        driver: Driver,
        fixture: &Fixture,
        assets: &Path,
        deadline: std::time::Duration,
    ) -> Self {
        let started = Instant::now();
        let work = tokio::time::timeout(deadline, async {
            let session = driver.open(&fixture.workspace, None).await?;
            driver.prompt(&session, fixture.request()).await
        })
        .await
        .unwrap_or(Err("workflow_deadline"));
        let prompt_ms = started.elapsed().as_millis();
        let (events, rpc_errors, tools, permissions, overflow, tools_terminal) = {
            let capture = driver.capture.lock().unwrap();
            (
                capture.events.clone(),
                capture.rpc_errors.clone(),
                capture.tools.len(),
                capture.permissions,
                capture.overflow,
                capture.tools.values().all(|status| {
                    matches!(
                        status,
                        acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed
                    )
                }),
            )
        };
        let cleanup = driver.close().await;
        let oracle = if cleanup.is_ok() && tools_terminal {
            fixture.check_in_guest(assets).await
        } else {
            Err("oracle_requires_settled_owned_sut".into())
        };
        let wire_checks = wire_checks(&events);
        Self {
            work,
            oracle,
            cleanup,
            events,
            rpc_errors,
            wire_checks,
            tools,
            permissions,
            prompt_ms,
            overflow,
            tools_terminal,
        }
    }

    pub fn passed(&self) -> bool {
        self.work.is_ok()
            && self.oracle.as_ref().is_ok_and(|oracle| oracle.correct)
            && self.cleanup.is_ok()
            && self.tools_terminal
            && !self.overflow
            && self.tools > 0
            && [
                "initial_failed_check",
                "final_passing_check",
                "bound_verified_outcome",
            ]
            .into_iter()
            .all(|key| self.wire_checks[key] == true)
    }

    pub fn retain(
        &self,
        output: &Path,
        fixture: &Fixture,
        qualification_pass: bool,
        metadata: Value,
    ) {
        let workflow_pass = self.passed();
        std::fs::write(
            output.join("wire-notifications.json"),
            serde_json::to_vec_pretty(&self.events).unwrap(),
        )
        .unwrap();
        std::fs::write(output.join("result.json"),serde_json::to_vec_pretty(&json!({"metadata":metadata,"pass":qualification_pass && workflow_pass,"workflow_pass":workflow_pass,"prompt_result":self.work,"oracle":self.oracle,"child_cleanup":self.cleanup,"wire_checks":self.wire_checks,"tool_calls":self.tools,"permission_requests":self.permissions,"prompt_ms":self.prompt_ms,"overflow":self.overflow,"tools_terminal":self.tools_terminal,"rpc_errors":self.rpc_errors,"oracle_execution":"independent quiescent snapshot in qualified Linux VM after ACP reaped; never execute model code on host"})).unwrap()).unwrap();
        for name in super::fixture::ARTIFACTS {
            if let Ok(bytes) = super::artifact::read(&fixture.workspace, name) {
                std::fs::write(output.join(name), bytes).unwrap();
            }
        }
    }
}

#[test]
fn retained_workflow_does_not_hide_an_outer_identity_failure() {
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(root.path()).unwrap();
    let output = root.path().join("evidence");
    std::fs::create_dir(&output).unwrap();
    let mut run = RustRun {
        work: Ok("controlled fixture".into()),
        oracle: Ok(Oracle {
            correct: true,
            compile_ok: true,
            tests_ok: true,
            protected_unchanged: true,
            checker_unchanged: true,
            changelog_present: true,
            current_files: Default::default(),
            diagnostic: "controlled fixture".into(),
        }),
        cleanup: Ok(()),
        events: Vec::new(),
        rpc_errors: Vec::new(),
        wire_checks: json!({"initial_failed_check":true,"final_passing_check":true,"bound_verified_outcome":true}),
        tools: 1,
        permissions: 0,
        prompt_ms: 1,
        overflow: false,
        tools_terminal: true,
    };
    assert!(run.passed());
    run.retain(
        &output,
        &fixture,
        false,
        json!({"identity_unchanged":false}),
    );
    let record: Value =
        serde_json::from_slice(&std::fs::read(output.join("result.json")).unwrap()).unwrap();
    assert_eq!(
        record["pass"], false,
        "the outer qualification failure must be retained"
    );
    assert_eq!(record["workflow_pass"], true);
    run.work = Err("controlled_workflow_failure");
    run.retain(&output, &fixture, true, json!({"identity_unchanged":true}));
    let record: Value =
        serde_json::from_slice(&std::fs::read(output.join("result.json")).unwrap()).unwrap();
    assert_eq!(
        record["pass"], false,
        "an outer pass cannot replace the workflow result"
    );
}

#[test]
fn wire_oracle_requires_failed_then_passing_exact_commands_and_bound_outcome() {
    let check = |code, text| json!({"update":{"sessionUpdate":"tool_call_update","rawOutput":{"command":LOCAL_CHECK,"exit_code":code,"output":text}}});
    let events = vec![
        check(101, "test result: FAILED. 0 passed; 3 failed"),
        check(0, "test tests::formats_cents ... ok\ntest tests::quantity_rejects_zero ... ok\ntest tests::rounds_half_up ... ok\ntest extra::legitimate ... ok\ntest result: ok. 4 passed; 0 failed; 0 ignored"),
        json!({"method":"a3s/turn/outcome","params":{"outcome":{"terminal":"verified","status":"passed"}}}),
    ];
    let valid = wire_checks(&events);
    assert!(
        valid["initial_failed_check"] == true
            && valid["final_passing_check"] == true
            && valid["bound_verified_outcome"] == true
    );
    assert_eq!(wire_checks(&events[1..])["initial_failed_check"], false);
    let mut wrong = events.clone();
    wrong[1]["update"]["rawOutput"]["command"] = format!("{LOCAL_CHECK} || true").into();
    assert_eq!(wire_checks(&wrong)["final_passing_check"], false);
    assert_eq!(wire_checks(&events[..2])["bound_verified_outcome"], false);
    assert_eq!(
        wire_checks(&[check(0, "success")])["final_passing_check"],
        false
    );
}
