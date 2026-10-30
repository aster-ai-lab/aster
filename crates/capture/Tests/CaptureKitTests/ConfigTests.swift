#if os(iOS)
  import Foundation
  @testable import CaptureKit
  import Testing

  struct ConfigTests {
    @Test func legacySetupHostIsAccepted() {
      let url = URL(string: "sample://scrawl?url=https://localhost:47777&token=fixture")!
      guard case .configure(let config)? = CaptureConfig.parseSetupURL(url) else {
        Issue.record("The legacy setup URL was rejected")
        return
      }
      #expect(config.urls == [URL(string: "https://localhost:47777")!])
      #expect(config.token == "fixture")
    }

    @Test func legacyEnvironmentIsReadWithoutKeychain() {
      let config = CaptureConfig.resolved(
        arguments: [],
        environment: [
          "SCRAWL_URL": "https://localhost:47777",
          "SCRAWL_TOKEN": "fixture",
          "SCRAWL_SESSION": "ses_legacy",
        ],
        keychain: nil
      )
      #expect(config?.urls == [URL(string: "https://localhost:47777")!])
      #expect(config?.token == "fixture")
      #expect(config?.sessionID == "ses_legacy")
    }

    @Test func captureEnvironmentTakesPrecedence() {
      let config = CaptureConfig.resolved(
        arguments: [],
        environment: [
          "CAPTURE_URL": "https://localhost:47777",
          "SCRAWL_URL": "https://localhost:59999",
          "CAPTURE_TOKEN": "fixture",
        ],
        keychain: nil
      )
      #expect(config?.urls == [URL(string: "https://localhost:47777")!])
      #expect(config?.token == "fixture")
    }
  }
#endif
