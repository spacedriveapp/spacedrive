use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BenchConfig {
	pub seed: Option<u64>,
	pub out_dir: PathBuf,
	pub clean: bool,
}

impl Default for BenchConfig {
	fn default() -> Self {
		let out_dir = dirs::data_dir()
			.unwrap_or_else(|| std::env::temp_dir())
			.join("spacedrive-bench");
		Self {
			seed: None,
			out_dir,
			clean: false,
		}
	}
}
