//! CLI glue over the library product cascade.

use anyhow::Result;

pub(crate) use aster_search::product::DEFAULT_TIMEOUT_SECONDS;
use aster_search::product::{
    execute_cascade as execute_product_cascade, CascadeControls,
    CascadeRequest as ProductCascadeRequest, HeadlessBrowser as ProductHeadlessBrowser,
};
pub(crate) use aster_search::EngineTierPlan;

mod plan;

pub(crate) use plan::CliEngineTier;

/// Clap-facing browser backend token.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HeadlessBrowser {
    #[default]
    Moli,
    Chrome,
    Lightpanda,
}

impl HeadlessBrowser {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Moli => "moli",
            Self::Chrome => "chrome",
            Self::Lightpanda => "lightpanda",
        }
    }
}

impl From<HeadlessBrowser> for ProductHeadlessBrowser {
    fn from(value: HeadlessBrowser) -> Self {
        match value {
            HeadlessBrowser::Moli => Self::Moli,
            HeadlessBrowser::Chrome => Self::Chrome,
            HeadlessBrowser::Lightpanda => Self::Lightpanda,
        }
    }
}

/// CLI request shape before conversion into the library product request.
pub(crate) struct CascadeRequest<'a> {
    pub query: aster_search::SearchQuery,
    pub limit: usize,
    pub timeout: std::time::Duration,
    pub proxy: Option<&'a str>,
    pub config: Option<&'a aster_search::SearchConfig>,
    pub browser: HeadlessBrowser,
    pub browser_max_retries: u32,
}

/// Executes the library product cascade with CLI clap types converted away.
pub(crate) async fn execute_cascade(
    plan: &EngineTierPlan,
    request: CascadeRequest<'_>,
    circuit: aster_search::CircuitBreaker,
) -> Result<aster_search::SearchCascadeOutcomeV2> {
    let product_request = ProductCascadeRequest {
        query: request.query,
        limit: request.limit,
        timeout: request.timeout,
        proxy: request.proxy,
        config: request.config,
        browser: request.browser.into(),
        browser_max_retries: request.browser_max_retries,
    };
    execute_product_cascade(
        plan,
        product_request,
        CascadeControls::with_circuit(circuit),
    )
    .await
    .map_err(anyhow::Error::from)
}
