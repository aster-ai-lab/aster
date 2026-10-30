// swift-tools-version: 6.2
import PackageDescription

let package = Package(
  name: "CaptureKit",
  // macOS only so `swift test` can run the platform-independent parts (the outbox) on a Mac;
  // everything UIKit is `#if os(iOS)`.
  platforms: [.iOS(.v18), .macOS(.v15)],
  products: [
    .library(name: "CaptureKit", targets: ["CaptureKit"]),
  ],
  dependencies: [
    .package(url: "https://github.com/swiftlang/swift-testing.git", revision: "48d727cc1cf4eda667c858c501495f1018f69d21"),
  ],
  targets: [
    .target(
      name: "CaptureKit",
      path: "Sources/CaptureKit",
      swiftSettings: [.swiftLanguageMode(.v5)]
    ),
    .testTarget(
      name: "CaptureKitTests",
      dependencies: ["CaptureKit", .product(name: "Testing", package: "swift-testing")],
      path: "Tests/CaptureKitTests",
      swiftSettings: [.swiftLanguageMode(.v5)],
      linkerSettings: (Context.environment["CAPTURE_TESTING_INTEROP_DIR"].map {
        [.unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", $0])]
      } ?? [])
    ),
  ]
)
