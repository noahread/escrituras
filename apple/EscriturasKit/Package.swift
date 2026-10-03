// swift-tools-version:5.9
import PackageDescription

// Swift wrapper around escrituras-core. Run scripts/build-xcframework.sh first:
// it builds EscriturasFFI.xcframework and generates Sources/EscriturasKit/Generated.
let package = Package(
    name: "EscriturasKit",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "EscriturasKit", targets: ["EscriturasKit"]),
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
        .testTarget(name: "EscriturasKitTests", dependencies: ["EscriturasKit"]),
    ]
)
