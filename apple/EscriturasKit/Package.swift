// swift-tools-version:5.9
import PackageDescription

// Swift wrapper around escrituras-core, and the native macOS app built on it.
// Run scripts/build-xcframework.sh first: it builds EscriturasFFI.xcframework
// and generates Sources/EscriturasKit/Generated.
let package = Package(
    name: "EscriturasKit",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "EscriturasKit", targets: ["EscriturasKit"]),
        .executable(name: "Escrituras", targets: ["Escrituras"]),
    ],
    targets: [
        .binaryTarget(name: "EscriturasFFI", path: "EscriturasFFI.xcframework"),
        .target(
            name: "EscriturasKit",
            dependencies: ["EscriturasFFI"],
            linkerSettings: [
                // System libraries used by the Rust code (ONNX Runtime, TLS, proxies)
                .linkedLibrary("c++"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("Foundation"),
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration"),
            ]
        ),
        // The app's models and views, in a library so they can be tested
        .target(name: "EscriturasApp", dependencies: ["EscriturasKit"]),
        // The app executable: just the @main entry point
        .executableTarget(name: "Escrituras", dependencies: ["EscriturasApp"]),
        .testTarget(name: "EscriturasKitTests", dependencies: ["EscriturasKit"]),
        .testTarget(name: "EscriturasAppTests", dependencies: ["EscriturasApp"]),
    ]
)
