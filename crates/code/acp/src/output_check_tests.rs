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
