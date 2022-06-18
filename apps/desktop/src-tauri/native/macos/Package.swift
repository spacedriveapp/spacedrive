// swift-tools-version: 5.5
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
	name: "sd-desktop-macos",
	platforms: [
        .macOS(.v10_15), // macOS Catalina. Earliest version that is officially supported by Apple.
    ],
	products: [
		// Products define the executables and libraries a package produces, and make them visible to other packages.
		.library(
			name: "sd-desktop-macos",
			type: .static,
			targets: ["sd-desktop-macos"]
		),
	],
	dependencies: [
		// Dependencies declare other packages that this package depends on.
		.package(url: "https://github.com/brendonovich/swift-rs.git", branch: "autorelease"),
	],
	targets: [
		// Targets are the basic building blocks of a package. A target can define a module or a test suite.
		// Targets can depend on other targets in this package, and on products in packages this package depends on.
		.target(
			name: "sd-desktop-macos",
			dependencies: [
				.product(name: "SwiftRs", package: "swift-rs")
			]),
	]
)
