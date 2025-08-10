use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate as bench;
use crate::generator::DatasetGenerator;

#[derive(Parser, Debug)]
#[command(name = "sd-bench")]
#[command(about = "Spacedrive benchmarking harness", long_about = None)]
pub struct Cli {
	#[command(subcommand)]
	pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
	/// Generate benchmark datasets based on a YAML recipe
	Mkdata {
		#[arg(short, long)]
		recipe: PathBuf,
		/// Prefix relative recipe locations with this path (e.g., /Volumes/HDD)
		#[arg(long)]
		dataset_root: Option<PathBuf>,
	},
	/// Generate a table summary from JSON results in a directory
	ResultsTable {
		/// Directory containing JSON summaries (default: benchmarks/results)
		#[arg(long, default_value = "benchmarks/results")]
		results_dir: PathBuf,
		/// Output file (if omitted, prints to stdout)
		#[arg(long)]
		out: Option<PathBuf>,
		/// Format: markdown or csv (default: markdown)
		#[arg(long, default_value = "markdown")]
		format: String,
	},
	/// Generate benchmark datasets for all recipes in a directory
	MkdataAll {
		/// Directory containing recipe YAML files
		#[arg(long, default_value = "benchmarks/recipes")]
		recipes_dir: PathBuf,
		/// Prefix relative recipe locations with this path (e.g., /Volumes/HDD)
		#[arg(long)]
		dataset_root: Option<PathBuf>,
		/// Only include recipe files whose filename matches this regex (applied to file stem), e.g. ^hdd_
		#[arg(long)]
		recipe_filter: Option<String>,
	},
	/// Run a benchmark scenario
	Run {
		#[arg(short, long)]
		scenario: String,
		#[arg(short, long)]
		recipe: PathBuf,
		#[arg(long)]
		out_json: Option<PathBuf>,
		/// Prefix relative recipe locations with this path (e.g., /Volumes/HDD)
		#[arg(long)]
		dataset_root: Option<PathBuf>,
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
		/// Skip dataset generation (assume recipes already generated)
		#[arg(long, default_value_t = false)]
		skip_generate: bool,
		/// Prefix relative recipe locations with this path (e.g., /Volumes/HDD)
		#[arg(long)]
		dataset_root: Option<PathBuf>,
		/// Only include recipe files whose filename matches this regex (applied to file stem), e.g. ^hdd_
		#[arg(long)]
		recipe_filter: Option<String>,
		/// Optional tag appended to output filenames to avoid overwrites (e.g., nvme, seagate). If omitted, derives from dataset_root basename when present.
		#[arg(long)]
		out_tag: Option<String>,
	},
}

pub async fn run(cli: Cli) -> Result<()> {
	match cli.command {
		Commands::Mkdata {
			recipe,
			dataset_root,
		} => mkdata(recipe, dataset_root).await?,
		Commands::MkdataAll {
			recipes_dir,
			dataset_root,
			recipe_filter,
		} => mkdata_all(recipes_dir, dataset_root, recipe_filter).await?,
		Commands::Run {
			scenario,
			recipe,
			out_json,
			dataset_root,
		} => run_scenario(scenario, recipe, out_json, dataset_root).await?,
		Commands::Report { input } => report(input).await?,
		Commands::RunAll {
			scenario,
			recipes_dir,
			out_dir,
			skip_generate,
			dataset_root,
			recipe_filter,
			out_tag,
		} => {
			run_all(
				scenario,
				recipes_dir,
				out_dir,
				skip_generate,
				dataset_root,
				recipe_filter,
				out_tag,
			)
			.await?
		}
		Commands::ResultsTable {
			results_dir,
			out,
			format,
		} => results_table(results_dir, out, &format).await?,
	}
	Ok(())
}

fn apply_dataset_root(
	mut recipe: bench::recipe::Recipe,
	root: Option<PathBuf>,
) -> bench::recipe::Recipe {
	if let Some(r) = root {
		let r = r;
		for loc in &mut recipe.locations {
			if loc.path.is_relative() {
				loc.path = r.join(&loc.path);
			}
		}
	}
	recipe
}

async fn mkdata(recipe_path: PathBuf, dataset_root: Option<PathBuf>) -> Result<()> {
	println!("Generating dataset for {}...", recipe_path.display());
	use std::io::Write as _;
	let _ = std::io::stdout().flush();
	let recipe_str = std::fs::read_to_string(&recipe_path)
		.with_context(|| format!("reading recipe {recipe_path:?}"))?;
	let recipe: bench::recipe::Recipe =
		serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;
	let recipe = apply_dataset_root(recipe, dataset_root);

	// If all location markers exist, skip generation
	let all_ready = recipe
		.locations
		.iter()
		.all(|loc| loc.path.join(".sd-bench-generated").exists());
	if all_ready {
		println!("Dataset already present (markers found), skipping generation.");
		return Ok(());
	}

	let gen = bench::generator::FileSystemGenerator::default();
	gen.generate(&recipe).await?;
	println!("Generated dataset: {}", recipe.name);
	let _ = std::io::stdout().flush();
	Ok(())
}

async fn mkdata_all(
	recipes_dir: PathBuf,
	dataset_root: Option<PathBuf>,
	recipe_filter: Option<String>,
) -> Result<()> {
	let mut entries: Vec<PathBuf> = Vec::new();
	for entry in std::fs::read_dir(&recipes_dir)? {
		let entry = entry?;
		let path = entry.path();
		if matches!(
			path.extension().and_then(|e| e.to_str()),
			Some("yaml" | "yml")
		) {
			entries.push(path);
		}
	}
	entries.sort();
	if entries.is_empty() {
		println!("No recipes found in {}", recipes_dir.display());
		return Ok(());
	}
	// Optional filter by regex on file stem
	let matcher = if let Some(pat) = recipe_filter {
		Some(
			regex::Regex::new(&pat)
				.map_err(|e| anyhow::anyhow!("bad recipe_filter regex: {}", e))?,
		)
	} else {
		None
	};

	for recipe in entries {
		if let Some(rx) = &matcher {
			let stem = recipe
				.file_stem()
				.unwrap_or_default()
				.to_string_lossy()
				.to_string();
			if !rx.is_match(&stem) {
				continue;
			}
		}
		mkdata(recipe, dataset_root.clone()).await?;
	}
	println!("\nCompleted MkdataAll for {}", recipes_dir.display());
	Ok(())
}

async fn run_scenario(
	scenario: String,
	recipe_path: PathBuf,
	out_json: Option<PathBuf>,
	dataset_root: Option<PathBuf>,
) -> Result<()> {
	println!(
		"Running scenario '{}' for recipe {}...",
		scenario,
		recipe_path.display()
	);
	use std::io::Write as _;
	let _ = std::io::stdout().flush();

	// Boot isolated core for benchmark
	println!("Booting isolated core for scenario '{}'...", scenario);
	let boot = bench::core_boot::boot_isolated_with_core(&scenario, None).await?;
	println!(
		"Core boot complete. Data dir: {} | Job logs: {}",
		boot.data_dir.display(),
		boot.job_logs_dir.display()
	);

	// Parse recipe
	let recipe_str = std::fs::read_to_string(&recipe_path)
		.with_context(|| format!("reading recipe {recipe_path:?}"))?;
	let recipe: bench::recipe::Recipe =
		serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;
	let recipe = apply_dataset_root(recipe, dataset_root);

	// Resolve scenario by name
	let mut scenario_impl = bench::scenarios::registry::registered_scenarios()
		.into_iter()
		.find(|s| s.name() == scenario)
		.ok_or_else(|| anyhow::anyhow!(format!("unknown scenario: {}", scenario)))?;

	// Reporters
	let reporters: Vec<Box<dyn bench::reporting::Reporter>> = if let Some(path) = &out_json {
		if let Some(parent) = path.parent() {
			std::fs::create_dir_all(parent).ok();
		}
		vec![Box::new(bench::reporting::JsonSummaryReporter::default())]
	} else {
		vec![]
	};

	// Run
	println!("Dispatching scenario '{}'...", scenario);
	let results = bench::runner::run_scenario(
		&boot,
		&bench::generator::NoopGenerator::default(),
		&mut *scenario_impl,
		&reporters,
		&recipe,
		out_json.as_deref(),
	)
	.await?;
	println!("Scenario '{}' completed.", scenario);

	// Always print a brief summary to stdout; if writing to file, also show where
	if let Some(path) = &out_json {
		println!("\nWrote summary to {}", path.display());
	}
	for r in &results {
		use bench::metrics::BenchmarkRun;
		match r {
			BenchmarkRun::IndexingDiscovery {
				meta,
				files,
				files_per_s,
				dirs,
				dirs_per_s,
				total_gb,
				errors,
				durations,
			}
			| BenchmarkRun::Aggregation {
				meta,
				files,
				files_per_s,
				dirs,
				dirs_per_s,
				total_gb,
				errors,
				durations,
			}
			| BenchmarkRun::ContentIdentification {
				meta,
				files,
				files_per_s,
				dirs,
				dirs_per_s,
				total_gb,
				errors,
				durations,
			} => {
				println!("\nBenchmark summary (job {}):", meta.id);
				if !meta.location_paths.is_empty() {
					println!("- Locations:");
					for p in &meta.location_paths {
						println!("  - {}", p.display());
					}
				}
				if let Some(hw) = &meta.hardware_label {
					println!("- Hardware: {}", hw);
				}
				if let Some(total) = durations.total_s {
					if total > 0.0 {
						println!("- Duration: {:.2}s", total);
					}
				}
				if let Some(dd) = durations.discovery_s {
					println!("- Discovery: {:.2}s", dd);
				}
				if let Some(cd) = durations.content_s {
					println!("- Content: {:.2}s", cd);
				}
				if *files > 0 {
					println!("- Files: {} ({:.1}/s)", files, files_per_s);
				}
				if *dirs > 0 {
					println!("- Directories: {} ({:.1}/s)", dirs, dirs_per_s);
				}
				if *total_gb > 0.0 {
					println!("- Total size: {:.2} GB", total_gb);
				}
				println!("- Errors: {}", errors);
			}
		}
	}

	Ok(())
}

async fn report(input: PathBuf) -> Result<()> {
	tracing::info!(input = %input.display(), "report (modular stub)");
	let data = std::fs::read_to_string(&input)?;
	println!("{}", data);
	Ok(())
}

async fn run_all(
	scenario: String,
	recipes_dir: PathBuf,
	out_dir: PathBuf,
	skip_generate: bool,
	dataset_root: Option<PathBuf>,
	recipe_filter: Option<String>,
	out_tag: Option<String>,
) -> Result<()> {
	std::fs::create_dir_all(&out_dir)?;
	println!(
		"Running 'run-all' for scenario '{}' in {} -> {}",
		scenario,
		recipes_dir.display(),
		out_dir.display()
	);
	let mut entries: Vec<PathBuf> = Vec::new();
	for entry in std::fs::read_dir(&recipes_dir)? {
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

	// Optional filter by regex on file stem
	let matcher = if let Some(pat) = &recipe_filter {
		Some(
			regex::Regex::new(pat)
				.map_err(|e| anyhow::anyhow!("bad recipe_filter regex: {}", e))?,
		)
	} else {
		None
	};

	for recipe in entries {
		if let Some(rx) = &matcher {
			let stem = recipe
				.file_stem()
				.unwrap_or_default()
				.to_string_lossy()
				.to_string();
			if !rx.is_match(&stem) {
				continue;
			}
		}
		println!("\n---");
		println!("Preparing {}", recipe.display());

		// Read recipe to decide whether to generate
		let recipe_str = std::fs::read_to_string(&recipe)
			.with_context(|| format!("reading recipe {recipe:?}"))?;
		let parsed: bench::recipe::Recipe =
			serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;
		let parsed = apply_dataset_root(parsed, dataset_root.clone());

		let all_paths_exist_before = parsed.locations.iter().all(|loc| loc.path.exists());

		if !skip_generate {
			if all_paths_exist_before {
				// If marker files exist for all locations, skip; otherwise generate
				let all_marked = parsed
					.locations
					.iter()
					.all(|loc| loc.path.join(".sd-bench-generated").exists());
				if all_marked {
					println!(
						"Skipping dataset generation (markers found) for '{}'",
						parsed.name
					);
				} else {
					println!(
						"Paths exist but no generation markers found; generating/repairing '{}'",
						parsed.name
					);
					mkdata(recipe.clone(), dataset_root.clone()).await?;
				}
			} else {
				mkdata(recipe.clone(), dataset_root.clone()).await?;
			}
		} else {
			println!("Skipping dataset generation (--skip-generate)");
		}

		// Preflight: ensure recipe location paths exist before running scenario
		let mut missing: Vec<String> = Vec::new();
		for loc in &parsed.locations {
			if !loc.path.exists() {
				missing.push(loc.path.display().to_string());
			}
		}
		if !missing.is_empty() {
			eprintln!(
				"Skipping scenario for {}: missing paths ({}). Run mkdata or remove from recipe.",
				recipe.display(),
				missing.join(", ")
			);
			continue;
		}
		let stem = recipe
			.file_stem()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string();
		let suffix = if let Some(tag) = &out_tag {
			format!("-{}", tag)
		} else if let Some(root) = &dataset_root {
			root.file_name()
				.and_then(|s| s.to_str())
				.map(|s| format!("-{}", s))
				.unwrap_or_default()
		} else {
			String::new()
		};
		let out_json = out_dir.join(format!("{}-{}{}.json", stem, scenario, suffix));
		println!("Executing scenario -> {}", out_json.display());
		run_scenario(
			scenario.clone(),
			recipe.clone(),
			Some(out_json),
			dataset_root.clone(),
		)
		.await?;
	}

	println!("\nCompleted RunAll for {} recipes.", scenario);
	Ok(())
}

async fn results_table(results_dir: PathBuf, out: Option<PathBuf>, format: &str) -> Result<()> {
	#[derive(serde::Deserialize, Debug)]
	struct HostInfo {
		#[serde(default)]
		cpu_model: Option<String>,
		#[serde(default)]
		cpu_physical_cores: Option<usize>,
		#[serde(default)]
		memory_total_gb: Option<u64>,
	}

	#[derive(serde::Deserialize, Debug)]
	struct RunMeta {
		id: Option<uuid::Uuid>,
		recipe_name: Option<String>,
		#[serde(default)]
		location_paths: Option<Vec<std::path::PathBuf>>,
		#[serde(default)]
		hardware_label: Option<String>,
		#[serde(default)]
		host: Option<HostInfo>,
	}

	#[derive(serde::Deserialize, Debug, Default)]
	struct Durations {
		#[serde(default)]
		discovery_s: Option<f64>,
		#[serde(default)]
		processing_s: Option<f64>,
		#[serde(default)]
		content_s: Option<f64>,
		#[serde(default)]
		total_s: Option<f64>,
	}

	#[derive(serde::Deserialize, Debug)]
	#[serde(tag = "scenario", rename_all = "kebab-case")]
	enum BenchmarkRunFile {
		IndexingDiscovery {
			meta: RunMeta,
			files: u64,
			files_per_s: f64,
			dirs: u64,
			dirs_per_s: f64,
			total_gb: f64,
			errors: u64,
			#[serde(default)]
			durations: Durations,
		},
		Aggregation {
			meta: RunMeta,
			files: u64,
			files_per_s: f64,
			dirs: u64,
			dirs_per_s: f64,
			total_gb: f64,
			errors: u64,
			#[serde(default)]
			durations: Durations,
		},
		ContentIdentification {
			meta: RunMeta,
			files: u64,
			files_per_s: f64,
			dirs: u64,
			dirs_per_s: f64,
			total_gb: f64,
			errors: u64,
			#[serde(default)]
			durations: Durations,
		},
	}

	#[derive(serde::Deserialize, Debug)]
	struct ResultsFile {
		runs: Vec<BenchmarkRunFile>,
	}

	let mut rows: Vec<(
		String, // scenario
		String, // recipe
		String, // hardware label
		String, // cpu model
		String, // cpu cores
		String, // memory GB
		f64,    // duration_s
		u64,    // files
		f64,    // files_per_s
		u64,    // dirs
		f64,    // dirs_per_s
		f64,    // total_gb
		u64,    // errors
	)> = Vec::new();

	for entry in std::fs::read_dir(&results_dir)? {
		let entry = entry?;
		let path = entry.path();
		if path.extension().and_then(|e| e.to_str()) != Some("json") {
			continue;
		}
		if let Ok(txt) = std::fs::read_to_string(&path) {
			if let Ok(parsed) = serde_json::from_str::<ResultsFile>(&txt) {
				for run in parsed.runs {
					let (
						scenario,
						meta,
						files,
						files_ps,
						dirs,
						dirs_ps,
						total_gb,
						errors,
						durations,
					) = match run {
						BenchmarkRunFile::IndexingDiscovery {
							meta,
							files,
							files_per_s,
							dirs,
							dirs_per_s,
							total_gb,
							errors,
							durations,
						} => (
							"indexing-discovery".to_string(),
							meta,
							files,
							files_per_s,
							dirs,
							dirs_per_s,
							total_gb,
							errors,
							durations,
						),
						BenchmarkRunFile::Aggregation {
							meta,
							files,
							files_per_s,
							dirs,
							dirs_per_s,
							total_gb,
							errors,
							durations,
						} => (
							"aggregation".to_string(),
							meta,
							files,
							files_per_s,
							dirs,
							dirs_per_s,
							total_gb,
							errors,
							durations,
						),
						BenchmarkRunFile::ContentIdentification {
							meta,
							files,
							files_per_s,
							dirs,
							dirs_per_s,
							total_gb,
							errors,
							durations,
						} => (
							"content-identification".to_string(),
							meta,
							files,
							files_per_s,
							dirs,
							dirs_per_s,
							total_gb,
							errors,
							durations,
						),
					};
					let recipe = meta.recipe_name.unwrap_or_else(|| "?".into());
					let hardware = meta
						.hardware_label
						.or_else(|| {
							meta.location_paths
								.as_ref()
								.and_then(|paths| paths.get(0))
								.and_then(|p| {
									let mut it = p.iter();
									let _root = it.next();
									if let Some(vol) = it.next() {
										if vol.to_string_lossy() == "Volumes" {
											return it
												.next()
												.map(|n| n.to_string_lossy().to_string());
										}
									}
									None
								})
						})
						.unwrap_or_else(|| "?".into());
					let cpu_model = meta
						.host
						.as_ref()
						.and_then(|h| h.cpu_model.clone())
						.unwrap_or_default();
					let cpu_cores = meta
						.host
						.as_ref()
						.and_then(|h| h.cpu_physical_cores)
						.map(|v| v.to_string())
						.unwrap_or_default();
					let mem_gb = meta
						.host
						.as_ref()
						.and_then(|h| h.memory_total_gb)
						.map(|v| v.to_string())
						.unwrap_or_default();
					let duration = durations.total_s.unwrap_or(0.0);
					rows.push((
						scenario, recipe, hardware, cpu_model, cpu_cores, mem_gb, duration, files,
						files_ps, dirs, dirs_ps, total_gb, errors,
					));
				}
			}
		}
	}

	// Sort by scenario, then recipe, then hardware
	rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

	let output = match format.to_lowercase().as_str() {
		// Default whitepaper format now includes phase breakdown under 'Indexing'
		"whitepaper" => {
			use std::collections::HashMap;
			fn phase_for_scenario(s: &str) -> Option<&'static str> {
				match s {
					"indexing-discovery" => Some("Discovery"),
					"aggregation" => Some("Processing"),
					"content-identification" => Some("Content Identification"),
					_ => None,
				}
			}
			// Average files/s and GB/s across shapes per phase and hardware
			let mut by_hw_phase_fps: HashMap<(String, String), Vec<f64>> = HashMap::new();
			let mut by_hw_phase_gbps: HashMap<(String, String), Vec<f64>> = HashMap::new();
			for (sc, rc, hw, _cpu, _cores, _mem, du, _f, fps, _d, _dps, gb, _e) in &rows {
				let Some(phase) = phase_for_scenario(sc) else {
					continue;
				};
				if rc.starts_with("shape_") && *fps > 0.0 {
					by_hw_phase_fps
						.entry((phase.to_string(), hw.clone()))
						.or_default()
						.push(*fps);
					let gbps = if *du > 0.0 { *gb / *du } else { 0.0 };
					if gbps > 0.0 {
						by_hw_phase_gbps
							.entry((phase.to_string(), hw.clone()))
							.or_default()
							.push(gbps);
					}
				}
			}
			let mut entries: Vec<(String, String, f64, f64)> = Vec::new();
			for ((phase, hw), vals) in by_hw_phase_fps.into_iter() {
				if !vals.is_empty() {
					let avg = vals.iter().copied().sum::<f64>() / (vals.len() as f64);
					let gbps_vals = by_hw_phase_gbps
						.remove(&(phase.clone(), hw.clone()))
						.unwrap_or_default();
					let avg_gbps = if !gbps_vals.is_empty() {
						gbps_vals.iter().copied().sum::<f64>() / (gbps_vals.len() as f64)
					} else {
						0.0
					};
					entries.push((phase, hw, avg, avg_gbps));
				}
			}
			fn phase_rank(p: &str) -> i32 {
				match p {
					"Discovery" => 0,
					"Processing" => 1,
					"Content Identification" => 2,
					_ => 9,
				}
			}
			entries.sort_by(|a, b| phase_rank(&a.0).cmp(&phase_rank(&b.0)).then(a.1.cmp(&b.1)));
			let mut s = String::new();
			s.push_str("Phase,Hardware,Files_per_s,GB_per_s\n");
			for (phase, hw, avg_fps, avg_gbps) in entries {
				s.push_str(&format!(
					"{},{},{:.1},{:.2}\n",
					phase, hw, avg_fps, avg_gbps
				));
			}
			s
		}
		"csv" => {
			let mut s = String::new();
			s.push_str(
                "scenario,recipe,hardware,cpu_model,cpu_physical_cores,memory_total_gb,duration_s,files,files_per_s,dirs,dirs_per_s,total_gb,errors\n",
            );
			for (sc, rc, hw, cpu, cores, mem, du, f, fps, d, dps, gb, e) in &rows {
				s.push_str(&format!(
					"{},{},{},{},{},{},{:.2},{},{:.1},{},{:.1},{:.2},{}\n",
					sc, rc, hw, cpu, cores, mem, du, f, fps, d, dps, gb, e
				));
			}
			s
		}
		_ => {
			// markdown
			let mut s = String::new();
			s.push_str("| scenario | recipe | duration (s) | files | files/s | dirs | dirs/s | total GB | errors |\n");
			s.push_str("|---|---|---:|---:|---:|---:|---:|---:|---:|\n");
			for (sc, rc, _hw, _cpu, _cores, _mem, du, f, fps, d, dps, gb, e) in &rows {
				s.push_str(&format!(
					"| {} | {} | {:.2} | {} | {:.1} | {} | {:.1} | {:.2} | {} |\n",
					sc, rc, du, f, fps, d, dps, gb, e
				));
			}
			s
		}
	};

	if let Some(out_path) = out {
		if let Some(parent) = out_path.parent() {
			std::fs::create_dir_all(parent).ok();
		}
		std::fs::write(&out_path, &output)?;
		println!("Wrote results table to {}", out_path.display());
	} else {
		println!("\n{}", output);
	}

	Ok(())
}
