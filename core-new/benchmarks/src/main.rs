use std::{fs, fs::File, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rand::{rngs::StdRng, Rng, SeedableRng};
use regex::Regex;
use sd_core_new as core;
use sd_core_new::infrastructure::actions::handler::ActionHandler;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, Level};
// use uuid::Uuid; // keep fully qualified uuid::Uuid

#[derive(Parser, Debug)]
#[command(name = "sd-bench")]
#[command(about = "Spacedrive benchmarking harness", long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
	/// Generate benchmark datasets based on a YAML recipe
	Mkdata {
		#[arg(short, long)]
		recipe: PathBuf,
	},
	/// Run a benchmark scenario (stub)
	Run {
		#[arg(short, long)]
		scenario: String,
		#[arg(short, long)]
		recipe: PathBuf,
		#[arg(long)]
		out_json: Option<PathBuf>,
	},
	/// Render reports from JSON results (stub)
	Report {
		#[arg(short, long)]
		input: PathBuf,
	},
	/// Run all recipes in a directory sequentially and write per-recipe results
	RunAll {
		/// Scenario name (e.g., indexing-discovery)
		#[arg(short, long, default_value = "indexing-discovery")]
		scenario: String,
		/// Directory containing recipe YAML files
		#[arg(long, default_value = "benchmarks/recipes")]
		recipes_dir: PathBuf,
		/// Output directory for JSON summaries
		#[arg(long, default_value = "benchmarks/results")]
		out_dir: PathBuf,
	},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Recipe {
	name: String,
	seed: Option<u64>,
	locations: Vec<RecipeLocation>,
	#[serde(default)]
	media: Option<RecipeMedia>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeLocation {
	path: PathBuf,
	structure: Structure,
	files: FileSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Structure {
	depth: usize,
	fanout_per_dir: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileSpec {
	total: usize,
	size_buckets: HashMap<String, SizeBucket>,
	#[serde(default)]
	duplicate_ratio: Option<f32>,
	#[serde(default)]
	media_ratio: Option<f32>,
	#[serde(default)]
	extensions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SizeBucket {
	range: [u64; 2],
	share: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeMedia {
	#[serde(default)]
	generate_thumbnails: Option<bool>,
	#[serde(default)]
	synthetic_video: Option<SynthVideo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SynthVideo {
	enabled: bool,
	duration_s: Option<u32>,
	width: Option<u32>,
	height: Option<u32>,
}

#[tokio::main]
async fn main() -> Result<()> {
	init_tracing();
	let cli = Cli::parse();
	match cli.command {
		Commands::Mkdata { recipe } => mkdata(recipe).await?,
		Commands::Run {
			scenario,
			recipe,
			out_json,
		} => run_scenario(scenario, recipe, out_json).await?,
		Commands::Report { input } => report(input).await?,
		Commands::RunAll {
			scenario,
			recipes_dir,
			out_dir,
		} => run_all(scenario, recipes_dir, out_dir).await?,
	}
	Ok(())
}

fn init_tracing() {
	let _ = tracing_subscriber::fmt()
		.with_max_level(Level::INFO)
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.try_init();
}

async fn mkdata(recipe_path: PathBuf) -> Result<()> {
	let recipe_str = fs::read_to_string(&recipe_path)
		.with_context(|| format!("reading recipe {recipe_path:?}"))?;
	let recipe: Recipe = serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;
	info!(name = %recipe.name, "loaded recipe");

	let mut rng: StdRng = match recipe.seed {
		Some(s) => StdRng::seed_from_u64(s),
		None => StdRng::from_entropy(),
	};

	for loc in &recipe.locations {
		// Ensure base directory exists
		fs::create_dir_all(&loc.path).with_context(|| format!("create {:#?}", loc.path))?;

		// Determine counts
		let total_files = loc.files.total as u64;
		let dup_ratio = loc.files.duplicate_ratio.unwrap_or(0.0).clamp(0.0, 0.95) as f64;
		let originals_target = ((total_files as f64) * (1.0 - dup_ratio)).round() as u64;
		let duplicates_target = total_files.saturating_sub(originals_target);

		// Normalize bucket shares
		let mut buckets: Vec<(&String, &SizeBucket)> = loc.files.size_buckets.iter().collect();
		buckets.sort_by(|a, b| a.0.cmp(b.0));
		let share_sum: f32 = buckets.iter().map(|(_, b)| b.share.max(0.0)).sum();
		let norm = if share_sum > 0.0 { share_sum } else { 1.0 };

		// Allocate counts per bucket
		let mut bucket_counts: Vec<u64> = Vec::with_capacity(buckets.len());
		let mut assigned = 0u64;
		for (i, (_, b)) in buckets.iter().enumerate() {
			let portion = (b.share.max(0.0) / norm) as f64;
			let mut count = (portion * (originals_target as f64)).floor() as u64;
			if i == buckets.len() - 1 {
				count = originals_target.saturating_sub(assigned);
			}
			assigned += count;
			bucket_counts.push(count);
		}

		let extensions: Vec<String> = loc
			.files
			.extensions
			.clone()
			.unwrap_or_else(|| vec!["bin".to_string()]);

		// Track created originals for duplicates
		let mut created_files: Vec<PathBuf> = Vec::with_capacity(originals_target as usize);

		// Create original files
		for ((_, bucket), count) in buckets.iter().zip(bucket_counts.iter()) {
			for _ in 0..*count {
				// Choose nested directory
				let mut dir = loc.path.clone();
				let depth = if loc.structure.depth == 0 {
					1
				} else {
					rng.gen_range(1..=loc.structure.depth)
				};
				for _ in 0..depth {
					let idx = if loc.structure.fanout_per_dir == 0 {
						0
					} else {
						rng.gen_range(0..loc.structure.fanout_per_dir)
					};
					dir = dir.join(format!("d{}", idx));
				}
				fs::create_dir_all(&dir).with_context(|| format!("mkdir {:#?}", dir))?;

				// File name + size
				let ext = &extensions[rng.gen_range(0..extensions.len())];
				let fname = format!("f_{:016x}.{}", rng.gen::<u64>(), ext);
				let fpath = dir.join(fname);
				let [min_b, max_b] = bucket.range;
				let size = if max_b > min_b {
					rng.gen_range(min_b..=max_b)
				} else {
					min_b
				};

				let file = File::create(&fpath).with_context(|| format!("create {:#?}", fpath))?;
				file.set_len(size)
					.with_context(|| format!("set_len {} for {:#?}", size, fpath))?;
				created_files.push(fpath);
			}
		}

		// Create duplicate files
		for _ in 0..duplicates_target {
			if created_files.is_empty() {
				break;
			}
			let src_idx = rng.gen_range(0..created_files.len());
			let src = &created_files[src_idx];
			let mut dir = loc.path.clone();
			let depth = if loc.structure.depth == 0 {
				1
			} else {
				rng.gen_range(1..=loc.structure.depth)
			};
			for _ in 0..depth {
				let idx = if loc.structure.fanout_per_dir == 0 {
					0
				} else {
					rng.gen_range(0..loc.structure.fanout_per_dir)
				};
				dir = dir.join(format!("d{}", idx));
			}
			fs::create_dir_all(&dir).with_context(|| format!("mkdir {:#?}", dir))?;
			let ext = src
				.extension()
				.map(|e| format!(".{}", e.to_string_lossy()))
				.unwrap_or_default();
			let dst = dir.join(format!("dup_{:016x}{}", rng.gen::<u64>(), ext));
			match fs::hard_link(src, &dst) {
				Ok(_) => {}
				Err(_) => {
					fs::copy(src, &dst)
						.with_context(|| format!("copy {:#?} -> {:#?}", src, dst))?;
				}
			}
		}

		info!(path = %loc.path.display(), files = %loc.files.total, "generated dataset");
	}
	Ok(())
}

async fn run_scenario(
	scenario: String,
	recipe_path: PathBuf,
	out_json: Option<PathBuf>,
) -> Result<()> {
	info!(%scenario, recipe = %recipe_path.display(), "run scenario (stub)");
	// Boot core in an isolated benchmark data directory to avoid user libraries
	let bench_data_dir = dirs::data_dir()
		.unwrap_or(std::env::temp_dir())
		.join("spacedrive-bench");
	std::fs::create_dir_all(&bench_data_dir)
		.map_err(|e| anyhow::anyhow!("create bench data dir: {}", e))?;

	// Ensure job logging is enabled for benchmarks before core initializes
	let mut bench_cfg = match core::config::AppConfig::load_from(&bench_data_dir) {
		Ok(cfg) => cfg,
		Err(_) => core::config::AppConfig::default_with_dir(bench_data_dir.clone()),
	};
	bench_cfg.job_logging.enabled = true;
	bench_cfg.job_logging.include_debug = true;
	// keep default log directory; increase max size a bit
	if bench_cfg.job_logging.max_file_size < 50 * 1024 * 1024 {
		bench_cfg.job_logging.max_file_size = 50 * 1024 * 1024;
	}
	let job_logs_dir = bench_cfg.job_logs_dir();
	bench_cfg
		.save()
		.map_err(|e| anyhow::anyhow!("save bench config: {}", e))?;

	let core = core::Core::new_with_config(bench_data_dir)
		.await
		.map_err(|e| anyhow::anyhow!("init core: {}", e))?;
	let context = core.context.clone();
	let library = match core.libraries.get_primary_library().await {
		Some(lib) => lib,
		None => {
			// Create a default benchmark library if none are open
			let lib = core
				.libraries
				.create_library("Benchmarks", None, context.clone())
				.await
				.map_err(|e| anyhow::anyhow!("create benchmark library: {}", e))?;
			lib
		}
	};

	// Parse recipe (we may later auto-add locations based on recipe paths)
	let recipe_str = fs::read_to_string(&recipe_path)
		.with_context(|| format!("reading recipe {recipe_path:?}"))?;
	let recipe_cfg: Recipe = serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;

	// Add each recipe location via direct handler to avoid audit_log schema coupling
	let library_id = library.id();

	let mut job_ids: Vec<uuid::Uuid> = Vec::new();
	for loc in &recipe_cfg.locations {
		let action = core::infrastructure::actions::Action::LocationAdd {
			library_id,
			action: core::operations::locations::add::action::LocationAddAction {
				path: loc.path.clone(),
				name: Some(format!("bench:{}", recipe_cfg.name)),
				mode: core::operations::indexing::IndexMode::Shallow,
			},
		};
		let handler = core::operations::locations::add::action::LocationAddHandler::new();
		// Validate then execute directly
		handler
			.validate(context.clone(), &action)
			.await
			.map_err(|e| anyhow::anyhow!("validate location.add: {}", e))?;
		let out = handler
			.execute(context.clone(), action)
			.await
			.map_err(|e| anyhow::anyhow!("execute location.add: {}", e))?;
		info!(output = %out, "location added");
		if let core::infrastructure::actions::output::ActionOutput::Custom { data, .. } = &out {
			if let Some(j) = data.get("job_id").and_then(|v| v.as_str()) {
				if let Ok(id) = uuid::Uuid::parse_str(j) {
					job_ids.push(id);
				}
			}
		}
	}

	// Wait for indexing jobs to complete (basic polling)
	if !job_ids.is_empty() {
		let job_manager = library.jobs().clone();
		loop {
			let mut remaining = 0usize;
			for jid in &job_ids {
				match job_manager.get_job_info(*jid).await {
					Ok(Some(info)) => {
						if !info.status.is_terminal() {
							remaining += 1;
						}
					}
					Ok(None) => {
						// Not found in DB; treat as finished
					}
					Err(_) => {
						remaining += 1;
					}
				}
			}
			if remaining == 0 {
				break;
			}
			tokio::time::sleep(Duration::from_millis(500)).await;
		}
		info!(jobs = job_ids.len(), "indexing jobs completed");
		// Ensure we flush stdout in case output buffering hides the summary
		use std::io::Write as _;
		let _ = std::io::stdout().flush();
		// Print job log file locations for inspection
		for jid in &job_ids {
			let log_path = job_logs_dir.join(format!("{}.log", jid));
			info!(job = %jid, log = %log_path.display(), "job log");
		}

		// Summarize metrics by parsing the job logs
		let re = Regex::new(r"Indexing completed in ([0-9.]+)s:|Files: ([0-9]+) \(([0-9.]+)/s\)|Directories: ([0-9]+) \(([0-9.]+)/s\)|Total size: ([0-9.]+) GB|Errors: ([0-9]+)").unwrap();
		let mut summaries: Vec<serde_json::Value> = Vec::new();
		for jid in &job_ids {
			let log_path = job_logs_dir.join(format!("{}.log", jid));
			if let Ok(txt) = std::fs::read_to_string(&log_path) {
				let mut files = None;
				let mut files_per_s = None;
				let mut dirs = None;
				let mut dirs_per_s = None;
				let mut total_gb = None;
				let mut duration_s = None;
				let mut errors = None;
				for cap in re.captures_iter(&txt) {
					if let Some(d) = cap.get(1) {
						duration_s = d.as_str().parse::<f64>().ok();
					}
					if let Some(f) = cap.get(2) {
						files = f.as_str().parse::<u64>().ok();
					}
					if let Some(fp) = cap.get(3) {
						files_per_s = fp.as_str().parse::<f64>().ok();
					}
					if let Some(di) = cap.get(4) {
						dirs = di.as_str().parse::<u64>().ok();
					}
					if let Some(dp) = cap.get(5) {
						dirs_per_s = dp.as_str().parse::<f64>().ok();
					}
					if let Some(ts) = cap.get(6) {
						total_gb = ts.as_str().parse::<f64>().ok();
					}
					if let Some(e) = cap.get(7) {
						errors = e.as_str().parse::<u64>().ok();
					}
				}
				println!("\nBenchmark summary (job {}):", jid);
				if let Some(s) = duration_s {
					println!("- Duration: {:.2}s", s);
				}
				if let (Some(f), Some(fp)) = (files, files_per_s) {
					println!("- Files: {} ({:.1}/s)", f, fp);
				}
				if let (Some(d), Some(dp)) = (dirs, dirs_per_s) {
					println!("- Directories: {} ({:.1}/s)", d, dp);
				}
				if let Some(gb) = total_gb {
					println!("- Total size: {:.2} GB", gb);
				}
				if let Some(e) = errors {
					println!("- Errors: {}", e);
				}

				summaries.push(serde_json::json!({
					"job_id": jid.to_string(),
					"scenario": scenario,
					"recipe": recipe_path.display().to_string(),
					"duration_s": duration_s,
					"files": files,
					"files_per_s": files_per_s,
					"directories": dirs,
					"directories_per_s": dirs_per_s,
					"total_gb": total_gb,
					"errors": errors,
				}));
			}
		}

		if let Some(path) = out_json {
			let data = serde_json::json!({ "runs": summaries });
			std::fs::write(&path, serde_json::to_string_pretty(&data)?)
				.with_context(|| format!("write summary {:#?}", path))?;
			println!("\nWrote summary to {}", path.display());
		}
	}
	Ok(())
}

async fn report(input: PathBuf) -> Result<()> {
	info!(input = %input.display(), "report (stub)");
	// TODO: parse JSON results and render markdown/CSV
	Ok(())
}

async fn run_all(scenario: String, recipes_dir: PathBuf, out_dir: PathBuf) -> Result<()> {
	// Ensure output directory exists
	fs::create_dir_all(&out_dir)?;

	// List YAML files in recipes_dir
	let mut entries: Vec<PathBuf> = Vec::new();
	for entry in fs::read_dir(&recipes_dir)? {
		let entry = entry?;
		let path = entry.path();
		if let Some(ext) = path.extension() {
			if ext == "yaml" || ext == "yml" {
				entries.push(path);
			}
		}
	}
	entries.sort();

	if entries.is_empty() {
		println!("No recipes found in {}", recipes_dir.display());
		return Ok(());
	}

	for recipe in entries {
		// mkdata for each recipe first
		mkdata(recipe.clone()).await?;
		// compose output path
		let stem = recipe
			.file_stem()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string();
		let out_json = out_dir.join(format!("{}-{}.json", stem, scenario));
		// run scenario
		run_scenario(scenario.clone(), recipe.clone(), Some(out_json)).await?;
	}

	println!("\nCompleted RunAll for {} recipes.", scenario);
	Ok(())
}
