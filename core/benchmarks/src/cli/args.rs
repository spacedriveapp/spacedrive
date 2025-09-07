use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GlobalArgs {
    pub seed: Option<u64>,
    pub out_dir: Option<PathBuf>,
    pub clean: bool,
}
