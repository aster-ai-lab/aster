use super::*;

#[test]
fn browser_statuses_cover_supported_managed_runtimes() {
    let statuses = browser_statuses();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].browser, ManagedBrowser::Chrome);
    let legacy = browser_status(ManagedBrowser::Lightpanda);
    assert!(!legacy.available);
    assert_eq!(legacy.source, BrowserInstallSource::Unsupported);
    assert!(legacy.path.is_none());
    assert_eq!(legacy.detail, LIGHTPANDA_UNSUPPORTED);
}

#[tokio::test]
async fn refuses_all_lightpanda_lifecycle_operations() {
    for result in [
        install_browser(ManagedBrowser::Lightpanda).await,
        update_browser(ManagedBrowser::Lightpanda).await,
        repair_browser(ManagedBrowser::Lightpanda).await,
    ] {
        let error = result.unwrap_err();
        assert_eq!(error.code, "use.browser.unsupported_backend");
        assert_eq!(error.message, LIGHTPANDA_UNSUPPORTED);
    }
}
