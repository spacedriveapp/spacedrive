// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SpacedriveCompanion",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(
            name: "SpacedriveCompanion",
            targets: ["SpacedriveCompanion"]
        ),
    ],
    dependencies: [
        // Add any external dependencies here if needed
    ],
    targets: [
        .executableTarget(
            name: "SpacedriveCompanion",
            dependencies: [],
            path: "Sources"
        ),
    ]
)


