#![cfg(test)]

use super::*;

fn request(allow_free_text: bool) -> Value {
    json!({"_meta":{"question":{"allowFreeText":allow_free_text,"hasChoices":true}},"toolCall":{"title":"选哪份文档？"},"options":[
        {"optionId":"question-0","name":"A.docx","kind":"reject_once"},
        {"optionId":"question-1","name":"B.docx","kind":"reject_once"}
    ]})
}

#[test]
fn an_explicit_second_choice_is_preserved_without_granting_a_tool() {
    let question = Question::from_permission(&request(true)).unwrap().unwrap();
    assert_eq!(question.choices[1].label, "B.docx");
    let values = serde_json::from_value(json!({"choice":"question-1","answer":""})).unwrap();
    assert_eq!(
        question.answer(&values),
        Some(json!({"outcome":{"outcome":"selected","optionId":"question-1"}}))
    );
    for values in [
        json!({}),
        json!({"choice":"allow-once"}),
        json!({"choice":"question-8"}),
        json!({"answer":"B","choice":"question-0"}),
        json!({"approved":true}),
    ] {
        assert!(
            question
                .answer(&serde_json::from_value(values).unwrap())
                .is_none()
        );
    }
}

#[test]
fn text_only_questions_have_no_synthetic_choice_and_do_not_fabricate_an_answer() {
    let mut params = request(true);
    params["_meta"]["question"]["hasChoices"] = json!(false);
    params["options"].as_array_mut().unwrap().truncate(1);
    let question = Question::from_permission(&params).unwrap().unwrap();
    assert!(question.choices.is_empty());
    let values = serde_json::from_value(json!({"answer":"  仅审查 B 文档  "})).unwrap();
    assert_eq!(
        question.answer(&values).unwrap()["_meta"]["followup_message"],
        "仅审查 B 文档"
    );
    assert!(
        question
            .answer(&serde_json::from_value(json!({"choice":"question-0"})).unwrap())
            .is_none()
    );
    assert!(
        Question::from_permission(&request(false))
            .unwrap()
            .unwrap()
            .answer(&values)
            .is_none()
    );
}

#[test]
fn untrusted_marker_and_granting_or_duplicate_options_fail_closed() {
    assert!(
        Question::from_permission(&json!({"options":[{"optionId":"allow-once"}]}))
            .unwrap()
            .is_none()
    );
    let mut variants = vec![];
    let mut missing = request(true);
    missing["_meta"]["question"]
        .as_object_mut()
        .unwrap()
        .remove("hasChoices");
    variants.push(missing);
    let mut granting = request(true);
    granting["options"][0]["kind"] = json!("allow_once");
    variants.push(granting);
    let mut granting_id = request(true);
    granting_id["options"][0]["optionId"] = json!("allow-once");
    variants.push(granting_id);
    let mut repeated = request(true);
    repeated["options"][1]["optionId"] = json!("question-0");
    variants.push(repeated);
    for params in variants {
        assert!(Question::from_permission(&params).is_err());
    }
}
