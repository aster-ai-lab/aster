//! Browser runtime lifecycle API for headless web search.
//!
//! This is a stable Aster Code entry point over the browser management surface
//! provided by aster-use-browser through aster-search's public Browser dependency.
//! Status checks are read-only; install, update, and repair are explicit host
//! operations and are never triggered by inspection.

use aster_search::aster_use_browser::{self as browser, UseError, UseResult};

pub use browser::{BrowserInstallSource, BrowserRuntimeStatus, ManagedBrowser};

pub(crate) const LIGHTPANDA_UNSUPPORTED: &str =
    "Lightpanda is no longer supported by Aster because it has no native Windows build. Select Moli or Chrome in search.headless.backend.";

/// Inspect a runtime without installing it. Legacy Lightpanda requests are unsupported.
pub fn browser_status(browser: ManagedBrowser) -> BrowserRuntimeStatus {
    if let Err(error) = supported_browser(browser) {
        return BrowserRuntimeStatus {
            browser,
            available: false,
            source: BrowserInstallSource::Unsupported,
            path: None,
            version: None,
            cache_dir: None,
            detail: error.to_string(),
        };
    }
    browser::browser_status(browser)
}

/// Inspect the managed runtimes supported by Aster. Moli has its own lifecycle API.
pub fn browser_statuses() -> Vec<BrowserRuntimeStatus> {
    vec![browser_status(ManagedBrowser::Chrome)]
}

/// Install a supported runtime; unsupported backends fail before filesystem or network access.
pub async fn install_browser(browser: ManagedBrowser) -> UseResult<BrowserRuntimeStatus> {
    supported_browser(browser)?;
    browser::install_browser(browser).await
}

/// Update a supported runtime; unsupported backends fail before filesystem or network access.
pub async fn update_browser(browser: ManagedBrowser) -> UseResult<BrowserRuntimeStatus> {
    supported_browser(browser)?;
    browser::update_browser(browser).await
}

/// Repair a supported runtime; unsupported backends fail before filesystem or network access.
pub async fn repair_browser(browser: ManagedBrowser) -> UseResult<BrowserRuntimeStatus> {
    supported_browser(browser)?;
    browser::repair_browser(browser).await
}

fn supported_browser(browser: ManagedBrowser) -> UseResult<()> {
    match browser {
        ManagedBrowser::Chrome => Ok(()),
        ManagedBrowser::Lightpanda => Err(UseError::new(
            "use.browser.unsupported_backend",
            LIGHTPANDA_UNSUPPORTED,
        )),
    }
}

#[cfg(test)]
#[path = "search_runtime_tests.rs"]
mod tests;
