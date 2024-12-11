// swift-tools-version: 5.5
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "sd-mobile-ios",
    platforms: [
        .iOS(.v14),
    ],
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "sd-mobile-ios",
            type: .static,
            targets: ["sd-mobile-ios"]
        ),
    ],
    dependencies: [
        // Dependencies declare other packages that this package depends on.
        .package(url: "https://github.com/brendonovich/swift-rs", branch: "specta"),
    ],
    targets: [
        // Targets are the basic building blocks of a package. A target can define a module or a test suite.
        // Targets can depend on other targets in this package, and on products in packages this package depends on.
        .target(
            name: "sd-mobile-ios",
            dependencies: [
                .product(name: "SwiftRs", package: "swift-rs")
            ],
            path: "src-swift"
        ),
    ]
)

