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
    Ok(())
}
