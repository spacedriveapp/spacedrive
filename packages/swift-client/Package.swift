// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SpacedriveClient",
    platforms: [
        .macOS(.v13),
        .iOS(.v16)
    ],
    products: [
        .library(
            name: "SpacedriveClient",
            targets: ["SpacedriveClient"]
        ),
    ],
    dependencies: [
        // No external dependencies needed - everything is generated!
    ],
    targets: [
        .target(
            name: "SpacedriveClient",
            dependencies: [],
            path: "Sources/SpacedriveClient"
        ),
        .testTarget(
            name: "SpacedriveClientTests",
            dependencies: ["SpacedriveClient"],
            path: "Tests/SpacedriveClientTests"
        ),
    ]
)
