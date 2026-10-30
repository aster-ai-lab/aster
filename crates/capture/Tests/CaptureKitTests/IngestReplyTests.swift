import Foundation
@testable import CaptureKit
import Testing

struct IngestReplyTests {
  func reply(_ json: String) -> IngestReply { IngestReply(body: Data(json.utf8)) }

  @Test func deliveredOnlyWhenTheMacSaysSo() {
    #expect(reply(#"{"ok":true,"delivered":true,"state":"sent","sessionTitle":"UP DOWN"}"#) == .delivered(sessionTitle: "UP DOWN"))
    #expect(reply(#"{"ok":true,"delivered":false,"state":"queued","sessionTitle":"UP DOWN","error":"OpenCode API failed"}"#)
      == .queued(sessionTitle: "UP DOWN"))
    #expect(reply(#"{"ok":true,"delivered":false,"state":"processing","sessionTitle":"UP DOWN"}"#) == .processing(sessionTitle: "UP DOWN"))
    #expect(reply(#"{"ok":true,"delivered":false,"state":"saved","sessionTitle":"Mac"}"#) == .saved)
    // A state this build doesn't know is still on the Mac, not delivered.
    #expect(reply(#"{"ok":true,"delivered":false,"state":"later","sessionTitle":"UP DOWN"}"#) == .saved)
  }

  @Test func serversFromBeforeDeliveredReadAsSent() {
    #expect(reply(#"{"ok":true,"sessionTitle":"Problems · Ledger","item":"I-1"}"#) == .delivered(sessionTitle: "Problems · Ledger"))
    #expect(reply(#"{"ok":true,"sessionTitle":""}"#) == .delivered(sessionTitle: "Capture"))
    #expect(reply("") == .delivered(sessionTitle: "Capture"))
    #expect(reply("not json") == .delivered(sessionTitle: "Capture"))
  }

  @Test func usbMarkersFallBackToTheSessionName() {
    #expect(IngestReply(sentMarker: "UP DOWN", delivery: nil) == .delivered(sessionTitle: "UP DOWN"))
    let queued = Data(#"{"ok":true,"delivered":false,"state":"queued","sessionTitle":"UP DOWN"}"#.utf8)
    #expect(IngestReply(sentMarker: "UP DOWN", delivery: queued) == .queued(sessionTitle: "UP DOWN"))
    #expect(IngestReply(sentMarker: "UP DOWN", delivery: Data("{}".utf8)) == .delivered(sessionTitle: "UP DOWN"))
  }
}
