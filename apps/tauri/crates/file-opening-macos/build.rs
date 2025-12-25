fn main() {
	swift_rs::SwiftLinker::new("11.0")
		.with_ios("11.0")
		.with_package("FileOpening", "./src-swift/")
		.link();
}
