fn main() {
	// Only build web assets if the assets feature is enabled
	#[cfg(feature = "assets")]
	{
		use std::process::Command;
		println!("cargo:rerun-if-changed=../web/src");
		println!("cargo:rerun-if-changed=../web/index.html");
		println!("cargo:rerun-if-changed=../web/package.json");

		// Build the web app
		let web_dir = std::env::current_dir()
			.expect("Failed to get current dir")
			.join("../web");

		// Check if pnpm is available
		let pnpm_check = Command::new("pnpm")
			.arg("--version")
			.output();

		if pnpm_check.is_err() {
			panic!("pnpm is required to build web assets. Install it with: npm install -g pnpm");
		}

		// Install dependencies
		println!("Installing web dependencies...");
		let install = Command::new("pnpm")
			.arg("install")
			.current_dir(&web_dir)
			.status()
			.expect("Failed to run pnpm install");

		if !install.success() {
			panic!("pnpm install failed");
		}

		// Build the web app
		println!("Building web app...");
		let build = Command::new("pnpm")
			.arg("build")
			.current_dir(&web_dir)
			.status()
			.expect("Failed to run pnpm build");

		if !build.success() {
			panic!("pnpm build failed");
		}

		println!("Web assets built successfully");
	}
}
