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
        .package(path: "../../packages/swift-client")
    ],
    targets: [
        .executableTarget(
            name: "SpacedriveCompanion",
            dependencies: [
                .product(name: "SpacedriveClient", package: "swift-client")
            ],
            path: "Sources"
        ),
    ]
)


