use super::*;
use crate::inference::decision::{
    DecisionContentPart, DecisionLevel, DecisionTurnStatus, DecisionVisibleTurn,
};
use a3s_apofasi::State;

fn ordinal_request() -> DecisionRequest {
    DecisionRequest {
        decision: DecisionDefinition::Ordinal {
            instructions: "Judge whether the new turn is simple or complex.".into(),
            levels: vec![
                DecisionLevel {
                    id: "simple".into(),
                    criterion: "one direct response, lookup, translation, or explanation completes the request without planning or multi-step analysis".into(),
                },
                DecisionLevel {
                    id: "complex".into(),
                    criterion: "multi-step reasoning, comparison, design, debugging, proof, or implementation is required".into(),
                },
            ],
        },
        latest_user: vec![DecisionContentPart::Text {
            text: "explain this proof".into(),
        }],
        visible_conversation: Vec::new(),
        history_partial: false,
        assessment_target: None,
    }
}

fn score_answer(probabilities: &[f32]) -> Answer {
    Answer::Score {
        score: 0.0,
        confidence: 1.0,
        legend: BTreeMap::from([
            (0.to_string(), json!("one direct step")),
            (1.to_string(), json!("multi-step reasoning")),
        ]),
        probabilities: probabilities
            .iter()
            .enumerate()
            .map(|(index, probability)| (index.to_string(), *probability))
            .collect(),
    }
}

#[test]
fn ordinal_contract_preserves_level_ids_and_normalizes_probabilities() {
    let request = ordinal_request();
    let system_request = system_one_request(&request).expect("convert request");
    assert!(matches!(system_request.state, State::Text(_)));
    assert_eq!(system_request.questions.len(), 1);
    let required = required_system_one_request(&request, &system_request);
    assert!(
        matches!(&required.state, State::Text(text) if text == "explain this proof\n\n[history is partial]")
    );
    assert_eq!(required.questions, system_request.questions);

    let response = SystemOneResponse {
        model: "laya".into(),
        answers: IndexMap::from([("decision".into(), score_answer(&[0.8, 0.2]))]),
        usage: Default::default(),
    };
    let mapped = map_system_one_response(&request, &response).expect("map response");
    let DecisionResult::Ordinal { probabilities } = mapped.decision else {
        panic!("expected ordinal result");
    };
    assert!(probabilities["simple"] > 0.79);
    assert!(probabilities["complex"] < 0.21);
    assert!((probabilities.values().sum::<f64>() - 1.0).abs() < 1e-12);
}

#[test]
fn invalid_neural_shape_is_rejected() {
    let request = ordinal_request();
    let response = SystemOneResponse {
        model: "laya".into(),
        answers: IndexMap::from([("decision".into(), Answer::Noul { noul: 0.8 })]),
        usage: Default::default(),
    };
    assert!(map_system_one_response(&request, &response).is_err());
}

#[tokio::test]
async fn real_checkpoint_classifies_auto_turns() {
    let Some(checkpoint) = std::env::var_os("A3S_APOFASI_CHECKPOINT") else {
        eprintln!("skip: set A3S_APOFASI_CHECKPOINT");
        return;
    };
    let service = ApofasiDecisionService::load_with_timeout(
        checkpoint,
        2,
        std::time::Duration::from_secs(20),
    )
    .expect("load real checkpoint");
    let cases = [
        ("What is 2 + 2?", true),
        ("用一句话解释什么是向量检索。", true),
        ("Translate 'good morning' into French.", true),
        ("列出 Rust 的三个所有权规则。", true),
        ("Compare quicksort and merge sort, then recommend one for a real-time system and justify the trade-offs.", false),
        ("设计一个支持千万用户的计费系统，分析一致性、容灾和扩容方案。", false),
        ("Review this authentication design for race conditions, propose a corrected protocol, and prove that replay cannot occur.", false),
        ("给出这段代码后，定位并发死锁的根因、修改实现并补充验证步骤。", false),
        ("Summarize the following paragraph in one sentence.", true),
        ("什么是 HTTP 200？", true),
        ("Write a haiku about rain.", true),
        ("Convert 42 kilometers to miles.", true),
        ("Explain the difference between TCP and UDP in two bullet points.", true),
        ("给我生成一个随机密码。", true),
        ("How do I reverse a string in Python? Show one example.", true),
        ("List three primary colors.", true),
        ("Implement a rate limiter with a sliding window and tests.", false),
        ("分析这份数据库慢查询日志，提出索引和分片方案并评估风险。", false),
        ("Derive Dijkstra's time complexity with a binary heap and prove it.", false),
        ("Build a zero-downtime migration plan for 10 TB of PostgreSQL data, including rollback.", false),
        ("Debug intermittent memory corruption in multithreaded Rust and create a minimal reproduction.", false),
        ("Compare Kubernetes and Nomad for a hybrid-cloud deployment, then design a rollout.", false),
        ("Write a compiler for arithmetic expressions with type checking and tests.", false),
        ("审查这段 OAuth 流程，修复 PKCE 和 state 校验并设计攻击测试。", false),
        ("Name the capital of Japan.", true),
        ("写一句欢迎语。", true),
        ("What is JSON?", true),
        ("Calculate 18% of 250.", true),
        ("Give me a simple regular expression for an email address.", true),
        ("Explain DNS in one paragraph.", true),
        ("Design exactly-once event processing with deduplication and failure recovery.", false),
        ("实现一个支持租户隔离的 RAG 检索系统并评估召回率。", false),
        ("Prove why an LRU cache is O(1) and analyze its concurrent implementation.", false),
        ("Refactor synchronous Python code to async without changing behavior and add performance tests.", false),
        ("Diagnose CUDA OOM in multi-GPU inference and propose remediation without downtime.", false),
        ("Write a threat model and security fixes for a file upload API.", false),
    ];
    let mut complex_routed_simple = 0;
    let mut simple_total = 0;
    let mut simple_routed_simple = 0;
    for (text, expect_simple) in cases {
        let mut request = ordinal_request();
        request.latest_user = vec![DecisionContentPart::Text { text: text.into() }];
        let started = std::time::Instant::now();
        let response = service.decide(&request).await.expect("real decision");
        let DecisionResult::Ordinal { probabilities } = response.decision else {
            panic!("expected ordinal result");
        };
        let simple = probabilities["simple"] >= 0.4;
        eprintln!(
            "auto case elapsed_ms={} expected_simple={expect_simple} simple_probability={} text={text}",
            started.elapsed().as_millis(),
            probabilities["simple"]
        );
        if expect_simple {
            simple_total += 1;
            simple_routed_simple += usize::from(simple);
        } else {
            complex_routed_simple += usize::from(simple);
        }
    }
    assert_eq!(complex_routed_simple, 0, "complex turn routed to economy");
    assert!(
        simple_routed_simple * 10 >= simple_total * 9,
        "simple recall below 90%: {simple_routed_simple}/{simple_total}"
    );
}

#[tokio::test]
async fn real_checkpoint_rejects_oversized_latest_user() {
    let Some(checkpoint) = std::env::var_os("A3S_APOFASI_CHECKPOINT") else {
        eprintln!("skip: set A3S_APOFASI_CHECKPOINT");
        return;
    };
    let service = ApofasiDecisionService::load(checkpoint, 1).expect("load real checkpoint");
    let mut request = ordinal_request();
    request.latest_user = vec![DecisionContentPart::Text {
        text: "multi-step reasoning ".repeat(3000),
    }];
    assert!(matches!(
        service.decide(&request).await,
        Err(DecisionError::Unreachable(_))
    ));
}

#[tokio::test]
async fn real_checkpoint_preserves_latest_user_when_history_is_truncated() {
    let Some(checkpoint) = std::env::var_os("A3S_APOFASI_CHECKPOINT") else {
        eprintln!("skip: set A3S_APOFASI_CHECKPOINT");
        return;
    };
    let service = ApofasiDecisionService::load_with_timeout(
        checkpoint,
        1,
        std::time::Duration::from_secs(20),
    )
    .expect("load real checkpoint");
    let mut request = ordinal_request();
    request.visible_conversation = (0..48)
        .map(|_| DecisionVisibleTurn {
            user: vec![DecisionContentPart::Text {
                text: "old routine notes and friendly conversation ".repeat(4),
            }],
            status: DecisionTurnStatus::Completed,
            steps: Vec::new(),
        })
        .collect();
    request.latest_user = vec![DecisionContentPart::Text {
        text: "Write a haiku about rain.".into(),
    }];
    let started = std::time::Instant::now();
    let response = service
        .decide(&request)
        .await
        .expect("decision with bounded history");
    eprintln!(
        "bounded history elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let DecisionResult::Ordinal { probabilities } = response.decision else {
        panic!("expected ordinal result");
    };
    assert!(
        probabilities["simple"] >= 0.4,
        "latest user was not preserved: {probabilities:?}"
    );
}
