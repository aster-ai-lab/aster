#if os(iOS)
  import SwiftUI
  import UIKit

  /// A window above the app for freehand red strokes and the recording strip. It only exists from
  /// the start of a recording until the strip has animated away after it.
  /// ReplayKit recordings include it; app-frame recordings leave it out. Every touch except one on
  /// the strip or its bubbles (or single-finger drawing when `isDrawing` is active) passes through
  /// to the app below.
  final class OverlayWindow: UIWindow {
    private var regions: [OverlayRegion: CGRect] = [:]
    private unowned let capture: Capture
    private var drawingTouch: UITouch?
    private var presentationLink: CADisplayLink?
    private var presentationTrace: StartupTrace?
    private var afterPresentation: (@MainActor () -> Void)?
    private var preparationStart: CFTimeInterval?
    var isActivelyDrawing: Bool { drawingTouch != nil }
    /// The red ink, under the strip. UIKit layers rather than SwiftUI, so a touch redraws only the
    /// stroke's live tip.
    let ink = InkView()

    init(windowScene: UIWindowScene, capture: Capture) {
      self.capture = capture
      super.init(windowScene: windowScene)
      windowLevel = .alert + 10
      backgroundColor = .clear
      let host = UIHostingController(rootView: OverlayView(capture: capture) { [weak self] region, frame in
        guard let self else { return }
        regions[region] = frame.isEmpty ? nil : frame
        if region == .strip, !frame.isEmpty, let start = preparationStart {
          preparationStart = nil
          capture.startupObserver?(.init(stage: "prewarmLayout", milliseconds: (CACurrentMediaTime() - start) * 1000, onMainThread: true))
        }
      })
      host.view.backgroundColor = .clear
      rootViewController = host
      ink.frame = bounds
      // Index 0: under the root view whenever UIKit adds it.
      insertSubview(ink, at: 0)
      ink.onStroke = { [unowned capture] samples in capture.record(stroke: samples) }
      ink.onFade = { [unowned capture] strokes, uptime in capture.recordFade(of: strokes, at: uptime) }
      isHidden = true
    }

    func prepareLayout() {
      preparationStart = CACurrentMediaTime()
      // A hidden UIWindow doesn't mount SwiftUI's glass hierarchy. Mount it transparently
      // during the deliberate hold so its first body/layout aren't on recognition's frame.
      alpha = 0
      isHidden = false
      rootViewController?.loadViewIfNeeded()
      rootViewController?.view.frame = bounds
      rootViewController?.view.layoutIfNeeded()
    }

    func present(trace: StartupTrace?) {
      let hasLaidOutControls = regions[.strip] != nil
      isHidden = false
      alpha = 1
      // A transparently prepared strip already has content. Revealing the window does not
      // need another SwiftUI layout; observe its next presentation opportunity directly.
      if hasLaidOutControls {
        trace?.mark("stripLayout")
        presentationLink?.invalidate()
        presentationTrace = trace
        let link = CADisplayLink(target: self, selector: #selector(presented))
        presentationLink = link
        link.add(to: .main, forMode: .common)
      }
    }

    @objc private func presented() {
      presentationLink?.invalidate()
      presentationLink = nil
      if !isHidden, alpha > 0 { presentationTrace?.mark("firstDraw") }
      presentationTrace = nil
      let action = afterPresentation
      afterPresentation = nil
      // Let this display turn commit before cold AVFoundation/VideoToolbox initialization
      // can contend with UIKit, even though the setup itself runs on background queues.
      if let action { DispatchQueue.main.async(execute: action) }
    }

    func afterFirstPresentation(_ action: @escaping @MainActor () -> Void) {
      if presentationLink != nil {
        afterPresentation = action
      } else {
        action()
      }
    }

    func dismiss() {
      presentationLink?.invalidate()
      presentationLink = nil
      presentationTrace = nil
      afterPresentation = nil
      isHidden = true
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) { fatalError() }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
      guard alpha > 0, !isHidden else { return nil }
      if regions.values.contains(where: { $0.insetBy(dx: -8, dy: -8).contains(point) }) {
        return super.hitTest(point, with: event)
      }
      if capture.isRecording || capture.isStarting, capture.isDrawing {
        return self
      }
      return nil
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
      // One finger draws at a time; others are ignored until it lifts.
      guard capture.isRecording || capture.isStarting, capture.isDrawing, drawingTouch == nil, let touch = touches.first else {
        if drawingTouch == nil { super.touchesBegan(touches, with: event) }
        return
      }
      drawingTouch = touch
      ink.began(touch, with: event)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
      guard let active = drawingTouch, touches.contains(active) else {
        super.touchesMoved(touches, with: event)
        return
      }
      ink.moved(active, with: event)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
      guard let active = drawingTouch, touches.contains(active) else {
        super.touchesEnded(touches, with: event)
        return
      }
      ink.ended(active, with: event)
      drawingTouch = nil
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
      guard let active = drawingTouch, touches.contains(active) else {
        super.touchesCancelled(touches, with: event)
        return
      }
      ink.ended(active, with: event)
      drawingTouch = nil
    }

    /// Removes the ink (a recording started or finished).
    func clearInk(animated: Bool) {
      drawingTouch = nil
      ink.clear(animated: animated)
    }
  }

  private struct OverlayView: View {
    let capture: Capture
    let reportFrame: @MainActor (OverlayRegion, CGRect) -> Void

    var body: some View {
      // The ink is `OverlayWindow.ink`, under this view.
      StripPlacement(mode: stripMode, actions: actions, notice: capture.isRecording ? capture.notice : nil)
        .environment(\.captureStartupTrace, capture.startupTrace)
    }

    private var stripMode: CaptureStrip.Mode? {
      if capture.isRecording || capture.isStarting, let startedAt = capture.startedAt {
        return .recording(startedAt: startedAt, isDrawing: capture.isDrawing)
      }
      if let preparedAt = capture.preparedAt {
        return .recording(startedAt: preparedAt, isDrawing: false)
      }
      if capture.isDiscardPending {
        return .discarded
      }
      switch capture.uploadStatus {
      case .idle: return nil
      case .sending: return .sending(.init(progress: capture.sendProgress, stalled: capture.isSendStalled))
      case let .sent(target): return .sent(target)
      case .queued: return .queued
      case .failed: return .failed(capture.deliveryFailure)
      }
    }

    private var actions: CaptureStrip.Actions {
      CaptureStrip.Actions(
        toggleDrawing: { capture.toggleDrawing() },
        discard: { capture.discard() },
        send: {
          UIImpactFeedbackGenerator(style: .light).impactOccurred()
          capture.stop()
        },
        undoDiscard: { capture.undoDiscard() },
        share: { capture.shareFailedRecording() },
        reportFrame: reportFrame
      )
    }
  }
#endif
