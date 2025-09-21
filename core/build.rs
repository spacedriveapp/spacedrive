use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Emit the instructions
	EmitBuilder::builder()
		.git_sha(true)
		.git_commit_timestamp()
		.git_branch()
		.cargo_opt_level()
		.cargo_target_triple()
		.emit()?;

	// Emit build timestamp manually
	println!(
		"cargo:rustc-env=BUILD_TIMESTAMP={}",
		chrono::Utc::now().to_rfc3339()
	);

	// Watch for changes in ops and event modules to trigger client regeneration
	println!("cargo:rerun-if-changed=src/ops");
	println!("cargo:rerun-if-changed=src/infra/event");
	println!("cargo:rerun-if-changed=src/domain");

	// Note: Schema generation is a manual step to avoid circular dependencies
	// To regenerate client types:
	// 1. cargo run --bin generate-schemas
	// 2. cd packages/swift-client && ./generate_client.sh
	// 3. cd packages/ts-client && ./generate_client.sh

	Ok(())
}