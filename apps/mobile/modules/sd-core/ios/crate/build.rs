#[cfg(target_os = "ios")]
use std::env;

fn main() {
	#[cfg(target_os = "ios")]
	{
		let deployment_target =
			env::var("IPHONEOS_DEPLOYMENT_TARGET").unwrap_or_else(|_| String::from("14.0"));

		swift_rs::SwiftLinker::new(deployment_target.as_str())
			.with_package("sd-mobile-ios", "./")
			.link();
	}
}
