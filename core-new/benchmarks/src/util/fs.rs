use std::path::PathBuf;

pub fn ensure_dir(path: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}
