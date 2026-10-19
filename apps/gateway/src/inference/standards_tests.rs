#![cfg(test)]

use super::standards::{ModelNamePlacement, ModelStandard, StandardAdmission};

#[test]
fn catalog_is_the_closed_plan() {
    let ids: Vec<_> = ModelStandard::ALL
        .iter()
        .map(|item| item.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            "openai.models",
            "openai.chat",
            "openai.completions",
            "openai.embeddings",
            "openai.responses",
            "anthropic.messages",
            "rerank",
            "ocr",
            "decision.systemone",
            "gemini.generate",
            "gemini.embed",
            "openai.images",
            "openai.audio",
            "api",
            "mcp",
        ]
    );
}

#[test]
fn parse_roundtrips_and_names_the_closed_set() {
    for standard in ModelStandard::ALL {
        assert_eq!(ModelStandard::parse(standard.as_str()).unwrap(), standard);
    }
    let error = ModelStandard::parse("vertex").unwrap_err();
    let message = error.to_string();
    assert!(message.contains("vertex"));
    assert!(message.contains("openai.chat"));
    assert!(message.contains("decision.systemone"));
}

#[test]
fn routes_are_fixed_and_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for standard in ModelStandard::ALL {
        let routes = standard.routes();
        assert!(!routes.is_empty(), "{}", standard.as_str());
        for route in routes {
            assert!(route.method == "GET" || route.method == "POST");
            assert!(route.path.starts_with('/'));
            assert!(seen.insert((route.method, route.path)), "{}", route.path);
            assert!(!route.path.contains("://"));
        }
    }
}

#[test]
fn model_name_placement_follows_the_route() {
    assert_eq!(
        ModelStandard::OpenaiModels.model_placement(),
        ModelNamePlacement::None
    );
    assert_eq!(
        ModelStandard::Ocr.model_placement(),
        ModelNamePlacement::None
    );
    assert_eq!(
        ModelStandard::DecisionSystemOne.model_placement(),
        ModelNamePlacement::None
    );
    assert_eq!(
        ModelStandard::GeminiGenerate.model_placement(),
        ModelNamePlacement::Path
    );
    assert_eq!(
        ModelStandard::GeminiEmbed.model_placement(),
        ModelNamePlacement::Path
    );
    assert_eq!(
        ModelStandard::Api.model_placement(),
        ModelNamePlacement::Supplier
    );
    assert_eq!(
        ModelStandard::Mcp.model_placement(),
        ModelNamePlacement::Supplier
    );
    assert_eq!(ModelStandard::Mcp.routes()[0].path, "/v1/mcp");
    for standard in ModelStandard::ALL {
        if matches!(
            standard,
            ModelStandard::OpenaiModels
                | ModelStandard::Ocr
                | ModelStandard::DecisionSystemOne
                | ModelStandard::GeminiGenerate
                | ModelStandard::GeminiEmbed
                | ModelStandard::Api
                | ModelStandard::Mcp
        ) {
            continue;
        }
        assert_eq!(standard.model_placement(), ModelNamePlacement::Body);
        for route in standard.routes() {
            assert!(!route.path.contains("{model}"));
        }
    }
    for route in ModelStandard::GeminiGenerate.routes() {
        assert!(route.path.contains("{model}"));
    }
}

#[test]
fn only_the_decision_standard_has_question_types() {
    assert_eq!(
        ModelStandard::DecisionSystemOne
            .decision_questions()
            .iter()
            .map(|question| question.as_str())
            .collect::<Vec<_>>(),
        vec!["choice", "noul", "score"]
    );
    for standard in ModelStandard::ALL {
        if standard == ModelStandard::DecisionSystemOne {
            continue;
        }
        assert!(standard.decision_questions().is_empty());
    }
}

#[test]
fn companion_routes_stay_on_their_standard() {
    let messages: Vec<_> = ModelStandard::AnthropicMessages
        .routes()
        .iter()
        .map(|route| route.path)
        .collect();
    assert_eq!(messages, vec!["/v1/messages", "/v1/messages/count_tokens"]);
    let gemini: Vec<_> = ModelStandard::GeminiGenerate
        .routes()
        .iter()
        .map(|route| route.path)
        .collect();
    assert_eq!(
        gemini,
        vec![
            "/v1beta/models/{model}:generateContent",
            "/v1beta/models/{model}:streamGenerateContent",
        ]
    );
    let audio: Vec<_> = ModelStandard::OpenaiAudio
        .routes()
        .iter()
        .map(|route| route.path)
        .collect();
    assert_eq!(audio, vec!["/v1/audio/speech", "/v1/audio/transcriptions"]);
}

#[test]
fn admission_keeps_only_the_openai_core_and_platform_models() {
    let admitted: Vec<_> = ModelStandard::ALL
        .into_iter()
        .filter(|standard| standard.admission() == StandardAdmission::Admitted)
        .map(ModelStandard::as_str)
        .collect();
    assert_eq!(
        admitted,
        vec![
            "openai.models",
            "openai.chat",
            "openai.completions",
            "openai.embeddings",
        ]
    );
    assert_eq!(ModelStandard::Ocr.admission(), StandardAdmission::Platform);
    assert_eq!(
        ModelStandard::DecisionSystemOne.admission(),
        StandardAdmission::Platform
    );
    for standard in ModelStandard::ALL {
        let routes = standard.routes();
        assert!(!routes.is_empty());
        match standard.admission() {
            StandardAdmission::Admitted | StandardAdmission::Platform => {
                assert!(!ModelStandard::is_reserved_route(
                    routes[0].method,
                    routes[0].path
                ));
            }
            StandardAdmission::Reserved => {
                for route in routes {
                    if route.path.contains("{model}") {
                        continue;
                    }
                    assert!(ModelStandard::is_reserved_route(route.method, route.path));
                    assert!(
                        ModelStandard::match_route("GET", route.path).is_none()
                            || route.method == "GET"
                    );
                }
            }
        }
    }
    assert_eq!(
        ModelStandard::match_route("POST", "/v1beta/models/gemma:generateContent"),
        Some(ModelStandard::GeminiGenerate)
    );
    assert_eq!(
        ModelStandard::match_route("POST", "/v1beta/models/gemma:streamGenerateContent"),
        Some(ModelStandard::GeminiGenerate)
    );
    assert_eq!(
        ModelStandard::match_route("POST", "/v1beta/models/gemma:embedContent"),
        Some(ModelStandard::GeminiEmbed)
    );
    assert!(ModelStandard::is_reserved_route(
        "POST",
        "/v1beta/models/gemma:generateContent"
    ));
    assert!(ModelStandard::match_route("POST", "/v1beta/models/:generateContent").is_none());
    assert!(ModelStandard::match_route("POST", "/v1beta/models/a/b:generateContent").is_none());
    assert!(ModelStandard::match_route("GET", "/v1/messages").is_none());
}
