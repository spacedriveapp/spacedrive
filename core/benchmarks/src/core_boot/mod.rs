use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct CoreBoot {
	pub data_dir: PathBuf,
	pub core: Arc<sd_core::Core>,
}

impl CoreBoot {
	pub fn new(data_dir: PathBuf, core: Arc<sd_core::Core>) -> Self {
		Self {
			data_dir,
			core,
		}
	}
}

pub async fn boot_isolated_with_core(
	scenario_name: &str,
	override_data_dir: Option<PathBuf>,
) -> anyhow::Result<CoreBoot> {
	let bench_data_dir = override_data_dir.unwrap_or_else(|| {
		dirs::data_dir()
			.unwrap_or(std::env::temp_dir())
			.join("spacedrive-bench")
			.join(scenario_name)
	});
	std::fs::create_dir_all(&bench_data_dir)
		.map_err(|e| anyhow::anyhow!("create bench data dir: {}", e))?;

	let mut bench_cfg = match sd_core::config::AppConfig::load_from(&bench_data_dir) {
		Ok(cfg) => cfg,
		Err(_) => sd_core::config::AppConfig::default_with_dir(bench_data_dir.clone()),
	};
	bench_cfg.job_logging.enabled = true;
	bench_cfg.job_logging.include_debug = true;
	if bench_cfg.job_logging.max_file_size < 50 * 1024 * 1024 {
		bench_cfg.job_logging.max_file_size = 50 * 1024 * 1024;
	}
	bench_cfg
		.save()
		.map_err(|e| anyhow::anyhow!("save bench config: {}", e))?;

	let core = sd_core::Core::new(bench_data_dir.clone())
		.await
		.map_err(|e| anyhow::anyhow!("init core: {}", e))?;
	let core = Arc::new(core);
	Ok(CoreBoot::new(bench_data_dir, core))
}
