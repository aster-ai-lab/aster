//! Closed catalog of caller-facing gateway standards.
//!
//! The gateway owns every path, method, and body shape for model, API, and
//! MCP calls. A supplier checks which entries it supports. A signed-in user
//! may call only the admitted entries. Dispatch consults this catalog:
//! reserved entries are refused before ordinary routing.

/// Which kind of supplier service a standard calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ServiceClass {
    Model,
    Api,
    Mcp,
}

/// Where the caller places the `{supplier}/{model}` name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ModelNamePlacement {
    /// The route lists models and carries no model name.
    None,
    /// JSON field `model`.
    Body,
    /// Path segment `{model}`.
    Path,
    /// JSON field `supplier`. Absent means the platform picks a supplier that offers the standard.
    Supplier,
}

/// Question types accepted by the decision standard. Empty for every other standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DecisionQuestionType {
    /// One choice among named options, with a probability distribution and confidence.
    Choice,
    /// Yes-or-no probability.
    Noul,
    /// Ordered scale, with a probability distribution and confidence.
    Score,
}

impl DecisionQuestionType {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Choice => "choice",
            Self::Noul => "noul",
            Self::Score => "score",
        }
    }
}

/// One fixed caller route that belongs to a standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StandardRoute {
    pub method: &'static str,
    pub path: &'static str,
}

/// Whether a catalog entry may be dispatched now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StandardAdmission {
    /// OpenAI core paths named by `I0.2b`.
    Admitted,
    /// Platform OCR and System One. They never enter chat or embeddings.
    Platform,
    /// Named in the catalog, refused until its own gate admits it.
    Reserved,
}

/// Industry-standard model route owned by the gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ModelStandard {
    OpenaiModels,
    OpenaiChat,
    OpenaiCompletions,
    OpenaiEmbeddings,
    OpenaiResponses,
    AnthropicMessages,
    Rerank,
    /// Image OCR. Callers send the image and no model alias.
    Ocr,
    DecisionSystemOne,
    GeminiGenerate,
    GeminiEmbed,
    OpenaiImages,
    OpenaiAudio,
    /// One entrance for every supplier API. Domain stays in the request, not in the route.
    Api,
    Mcp,
}

/// Parse failure. The message names the closed set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnknownModelStandard {
    value: String,
}

impl std::fmt::Display for UnknownModelStandard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unknown gateway standard {:?}; expected {}",
            self.value,
            expected_standards()
        )
    }
}

impl std::error::Error for UnknownModelStandard {}

const DECISION_QUESTIONS: [DecisionQuestionType; 3] = [
    DecisionQuestionType::Choice,
    DecisionQuestionType::Noul,
    DecisionQuestionType::Score,
];

impl ModelStandard {
    pub(crate) const ALL: [Self; 15] = [
        Self::OpenaiModels,
        Self::OpenaiChat,
        Self::OpenaiCompletions,
        Self::OpenaiEmbeddings,
        Self::OpenaiResponses,
        Self::AnthropicMessages,
        Self::Rerank,
        Self::Ocr,
        Self::DecisionSystemOne,
        Self::GeminiGenerate,
        Self::GeminiEmbed,
        Self::OpenaiImages,
        Self::OpenaiAudio,
        Self::Api,
        Self::Mcp,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiModels => "openai.models",
            Self::OpenaiChat => "openai.chat",
            Self::OpenaiCompletions => "openai.completions",
            Self::OpenaiEmbeddings => "openai.embeddings",
            Self::OpenaiResponses => "openai.responses",
            Self::AnthropicMessages => "anthropic.messages",
            Self::Rerank => "rerank",
            Self::Ocr => "ocr",
            Self::DecisionSystemOne => "decision.systemone",
            Self::GeminiGenerate => "gemini.generate",
            Self::GeminiEmbed => "gemini.embed",
            Self::OpenaiImages => "openai.images",
            Self::OpenaiAudio => "openai.audio",
            Self::Api => "api",
            Self::Mcp => "mcp",
        }
    }

    pub(crate) const fn class(self) -> ServiceClass {
        match self {
            Self::Api => ServiceClass::Api,
            Self::Mcp => ServiceClass::Mcp,
            _ => ServiceClass::Model,
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, UnknownModelStandard> {
        Self::ALL
            .into_iter()
            .find(|standard| standard.as_str() == value)
            .ok_or_else(|| UnknownModelStandard {
                value: value.to_owned(),
            })
    }

    pub(crate) const fn model_placement(self) -> ModelNamePlacement {
        match self {
            Self::OpenaiModels | Self::Ocr | Self::DecisionSystemOne => ModelNamePlacement::None,
            Self::GeminiGenerate | Self::GeminiEmbed => ModelNamePlacement::Path,
            Self::Api | Self::Mcp => ModelNamePlacement::Supplier,
            Self::OpenaiChat
            | Self::OpenaiCompletions
            | Self::OpenaiEmbeddings
            | Self::OpenaiResponses
            | Self::AnthropicMessages
            | Self::Rerank
            | Self::OpenaiImages
            | Self::OpenaiAudio => ModelNamePlacement::Body,
        }
    }

    pub(crate) const fn decision_questions(self) -> &'static [DecisionQuestionType] {
        match self {
            Self::DecisionSystemOne => &DECISION_QUESTIONS,
            _ => &[],
        }
    }

    pub(crate) const fn admission(self) -> StandardAdmission {
        match self {
            Self::OpenaiModels
            | Self::OpenaiChat
            | Self::OpenaiCompletions
            | Self::OpenaiEmbeddings => StandardAdmission::Admitted,
            Self::Ocr | Self::DecisionSystemOne => StandardAdmission::Platform,
            Self::OpenaiResponses
            | Self::AnthropicMessages
            | Self::Rerank
            | Self::GeminiGenerate
            | Self::GeminiEmbed
            | Self::OpenaiImages
            | Self::OpenaiAudio
            | Self::Api
            | Self::Mcp => StandardAdmission::Reserved,
        }
    }

    /// Exact method and path match against the catalog, including `{model}`.
    pub(crate) fn match_route(method: &str, path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|standard| {
            standard
                .routes()
                .iter()
                .any(|route| route.method == method && route_matches(route.path, path))
        })
    }

    pub(crate) fn is_reserved_route(method: &str, path: &str) -> bool {
        Self::match_route(method, path)
            .is_some_and(|standard| standard.admission() == StandardAdmission::Reserved)
    }

    pub(crate) const fn routes(self) -> &'static [StandardRoute] {
        match self {
            Self::OpenaiModels => &[StandardRoute {
                method: "GET",
                path: "/v1/models",
            }],
            Self::OpenaiChat => &[StandardRoute {
                method: "POST",
                path: "/v1/chat/completions",
            }],
            Self::OpenaiCompletions => &[StandardRoute {
                method: "POST",
                path: "/v1/completions",
            }],
            Self::OpenaiEmbeddings => &[StandardRoute {
                method: "POST",
                path: "/v1/embeddings",
            }],
            Self::OpenaiResponses => &[StandardRoute {
                method: "POST",
                path: "/v1/responses",
            }],
            Self::AnthropicMessages => &[
                StandardRoute {
                    method: "POST",
                    path: "/v1/messages",
                },
                StandardRoute {
                    method: "POST",
                    path: "/v1/messages/count_tokens",
                },
            ],
            Self::Rerank => &[StandardRoute {
                method: "POST",
                path: "/v1/rerank",
            }],
            Self::Ocr => &[StandardRoute {
                method: "POST",
                path: "/v1/ocr",
            }],
            Self::DecisionSystemOne => &[StandardRoute {
                method: "POST",
                path: "/v1/systemone",
            }],
            Self::GeminiGenerate => &[
                StandardRoute {
                    method: "POST",
                    path: "/v1beta/models/{model}:generateContent",
                },
                StandardRoute {
                    method: "POST",
                    path: "/v1beta/models/{model}:streamGenerateContent",
                },
            ],
            Self::GeminiEmbed => &[StandardRoute {
                method: "POST",
                path: "/v1beta/models/{model}:embedContent",
            }],
            Self::OpenaiImages => &[StandardRoute {
                method: "POST",
                path: "/v1/images/generations",
            }],
            Self::Api => &[StandardRoute {
                method: "POST",
                path: "/v1/service",
            }],
            Self::Mcp => &[StandardRoute {
                method: "POST",
                path: "/v1/mcp",
            }],
            Self::OpenaiAudio => &[
                StandardRoute {
                    method: "POST",
                    path: "/v1/audio/speech",
                },
                StandardRoute {
                    method: "POST",
                    path: "/v1/audio/transcriptions",
                },
            ],
        }
    }
}

fn route_matches(pattern: &str, path: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once("{model}") else {
        return pattern == path;
    };
    let Some(rest) = path.strip_prefix(prefix) else {
        return false;
    };
    let Some(model) = rest.strip_suffix(suffix) else {
        return false;
    };
    !model.is_empty() && !model.contains('/')
}

fn expected_standards() -> String {
    ModelStandard::ALL
        .into_iter()
        .map(ModelStandard::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}
