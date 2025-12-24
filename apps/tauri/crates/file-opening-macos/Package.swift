// swift-tools-version: 5.5
import PackageDescription

let package = Package(
    name: "FileOpening",
    platforms: [
        .macOS(.v11),
        .iOS(.v14)
    ],
    products: [
        .library(
            name: "FileOpening",
            type: .static,
            targets: ["FileOpening"]
        )
    ],
    targets: [
        .target(
            name: "FileOpening",
            path: "src-swift"
        )
    ]
)
