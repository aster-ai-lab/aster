#![cfg(test)]

use super::*;

#[tokio::test]
async fn missing_values_stay_missing_and_unknown_arguments_are_refused() {
    let ctx = ToolContext::new(std::path::PathBuf::from("."));
    let output = OutputCheckTool
        .execute(
            &json!({"action":"percentage_change","pairs":[
                {"current":null,"previous":10}, {"current":10,"previous":0},
                {"current":110,"previous":100}
            ]}),
            &ctx,
        )
        .await
        .unwrap();
    let result: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(result["results"][0]["value"], Value::Null);
    assert_eq!(result["results"][1]["display"], Value::Null);
    assert_eq!(result["results"][2]["display"], "+10%");
    for args in [
        json!({"action":"text_length","text":"abc","file_path":"secret"}),
        json!({"action":"unknown"}),
        json!({"action":"text_length","text":"字".repeat(32769)}),
        json!({"action":"percentage_change","pairs":vec![json!({"current":1,"previous":1});513]}),
        json!({"action":"percentage_change","pairs":[{"current":1}]}),
        json!({"action":"percentage_change","pairs":[{"previous":1}]}),
        json!({"action":"percentage_change"}),
        json!({"action":"text_length"}),
    ] {
        assert!(!OutputCheckTool.execute(&args, &ctx).await.unwrap().success);
    }
}

#[tokio::test]
async fn provenance_reports_missing_unexpected_duplicate_and_changed_records() {
    let output = OutputCheckTool
        .execute(
            &json!({
                "action":"record_provenance",
                "key":"id",
                "source":[
                    {"id":"A","category":"one","value":1},
                    {"id":"B","category":"two","value":2}
                ],
                "output":[
                    {"id":"A","category":"renamed","value":1},
                    {"id":"A","category":"renamed","value":1},
                    {"id":"C","category":"three","value":3}
                ]
            }),
            &ToolContext::new(".".into()),
        )
        .await
        .unwrap();
    let result: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(result["valid"], false);
    assert_eq!(result["missing"], json!(["B"]));
    assert_eq!(result["unexpected"], json!(["C"]));
    assert_eq!(result["duplicateOutputKeys"], json!(["A"]));
    assert_eq!(result["changed"][0]["key"], "A");
    assert_eq!(result["changed"][0]["fields"], json!(["category"]));
}

#[tokio::test]
async fn provenance_accepts_source_records_and_task_authorized_derived_records() {
    let output = OutputCheckTool
        .execute(
            &json!({
                "action":"record_provenance",
                "key":"id",
                "source":[{"id":"A","category":"one","value":1}],
                "output":[
                    {"id":"A","category":"one","value":1},
                    {"id":"TOTAL","category":"derived","value":1}
                ],
                "allow_derived_keys":["TOTAL"]
            }),
            &ToolContext::new(".".into()),
        )
        .await
        .unwrap();
    let result: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(result["valid"], true);
    assert_eq!(result["unexpected"], json!([]));
}

#[tokio::test]
async fn provenance_refuses_invalid_record_keys_and_unknown_arguments() {
    for args in [
        json!({"action":"record_provenance","key":"id","source":[{"value":1}],"output":[]}),
        json!({"action":"record_provenance","key":"id","source":[],"output":[],"extra":true}),
    ] {
        let output = OutputCheckTool
            .execute(&args, &ToolContext::new(".".into()))
            .await
            .unwrap();
        assert!(!output.success, "{}", output.content);
    }
}
