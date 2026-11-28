use super::*;
use serde_json::{json, Value};

#[test]
fn messages_return_a_parallel_batch_together() {
    let client = AnthropicClient::new("test-key".into(), "deepseek-flash".into());
    let mut assistant = Message::assistant("Reading both records.");
    assistant.content.extend(
        ["toolu_first", "toolu_second"].map(|id| ContentBlock::ToolUse {
            id: id.into(),
            name: "read_record".into(),
            input: json!({}),
        }),
    );
    let first = Message::tool_result_with_images(
        "toolu_first",
        "北区",
        &[Attachment::png(vec![1, 2, 3])],
        false,
    );
    let second = Message::tool_result("toolu_second", "record unavailable", true);
    let messages = [
        Message::user("Read both records."),
        assistant,
        first,
        second,
    ];

    let native = client.build_request(&messages, None, &[]);
    let wire = native["messages"].as_array().unwrap();
    assert_eq!(
        wire.len(),
        3,
        "both results immediately follow the assistant"
    );
    assert_eq!(wire[2]["role"], "user");
    let results = wire[2]["content"].as_array().unwrap();
    let expected: Vec<Value> = messages[2..]
        .iter()
        .map(|message| {
            let mut block = serde_json::to_value(&message.content[0]).unwrap();
            block["cache_control"] = json!({"type": "ephemeral"});
            block
        })
        .collect();
    assert_eq!(results, &expected);

    let mut history = messages.to_vec();
    history.push(Message::user("Use both results in the answer."));
    let native = client.build_request(&history, None, &[]);
    let wire = native["messages"].as_array().unwrap();
    assert_eq!(wire.len(), 4);
    assert_eq!(wire[2]["content"].as_array().unwrap().len(), 2);
    assert_eq!(
        wire[3]["content"][0]["text"],
        "Use both results in the answer."
    );
}
