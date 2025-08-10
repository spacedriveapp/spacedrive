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
		println!("\nBenchmark summary (job {}):", r.id);
		if !r.location_paths.is_empty() {
			println!("- Locations:");
			for p in &r.location_paths {
				println!("  - {}", p.display());
			}
		}
		if let Some(hw) = &r.hardware_label {
			println!("- Hardware: {}", hw);
		}
		if r.duration_s > 0.0 {
			println!("- Duration: {:.2}s", r.duration_s);
		}
		if let Some(dd) = r.discovery_duration_s {
			println!("- Discovery: {:.2}s", dd);
		}
		if let Some(cd) = r.content_duration_s {
			println!("- Content: {:.2}s", cd);
		}
		if r.files > 0 {
			println!("- Files: {} ({:.1}/s)", r.files, r.files_per_s);
		}
		if r.directories > 0 {
			println!(
				"- Directories: {} ({:.1}/s)",
				r.directories, r.directories_per_s
			);
		}
		if r.total_gb > 0.0 {
			println!("- Total size: {:.2} GB", r.total_gb);
		}
		println!("- Errors: {}", r.errors);
		if let Some(p) = r.raw_artifacts.get(0) {
			println!("- Job log: {}", p.display());
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
	struct ScenarioRun {
		id: Option<uuid::Uuid>,
		scenario: Option<String>,
		recipe_name: Option<String>,
		recipe: Option<String>,
		duration_s: Option<f64>,
		#[serde(default)]
		discovery_duration_s: Option<f64>,
		#[serde(default)]
		processing_duration_s: Option<f64>,
		#[serde(default)]
		content_duration_s: Option<f64>,
		files: Option<u64>,
		files_per_s: Option<f64>,
		directories: Option<u64>,
		directories_per_s: Option<f64>,
		total_gb: Option<f64>,
		errors: Option<u64>,
	}

	#[derive(serde::Deserialize, Debug)]
	struct ResultsFile {
		runs: Vec<ScenarioRun>,
	}

	let mut rows: Vec<(String, String, f64, u64, f64, u64, f64, f64, u64)> = Vec::new();
	// (scenario, recipe, duration_s, files, files_per_s, dirs, dirs_per_s, total_gb, errors)

	for entry in std::fs::read_dir(&results_dir)? {
		let entry = entry?;
		let path = entry.path();
		if path.extension().and_then(|e| e.to_str()) != Some("json") {
			continue;
		}
		if let Ok(txt) = std::fs::read_to_string(&path) {
			if let Ok(parsed) = serde_json::from_str::<ResultsFile>(&txt) {
				for run in parsed.runs {
					let scenario = run.scenario.unwrap_or_else(|| "?".into());
					let recipe = run.recipe_name.or(run.recipe).unwrap_or_else(|| "?".into());
					let duration = run.duration_s.unwrap_or(0.0);
					let files = run.files.unwrap_or(0);
					let files_ps = run.files_per_s.unwrap_or(0.0);
					let dirs = run.directories.unwrap_or(0);
					let dirs_ps = run.directories_per_s.unwrap_or(0.0);
					let total_gb = run.total_gb.unwrap_or(0.0);
					let errors = run.errors.unwrap_or(0);
					rows.push((
						scenario, recipe, duration, files, files_ps, dirs, dirs_ps, total_gb,
						errors,
					));
				}
			}
		}
	}

	// Sort by scenario then recipe
	rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

	let output = match format.to_lowercase().as_str() {
		// Default whitepaper format now includes phase breakdown under 'Indexing'
		"whitepaper" => {
			use std::collections::HashMap;
			fn label_for_recipe(recipe: &str) -> Option<&'static str> {
				let r = recipe.to_lowercase();
				if r.starts_with("nvme_") {
					Some("Internal NVMe SSD")
				} else if r.starts_with("hdd_") {
					Some("External HDD (USB 3.0)")
				} else if r.contains("nas") {
					Some("Network Attached Storage (1Gbps)")
				} else if r.contains("usb") {
					Some("External USB 3.2 SSD")
				} else {
					None
				}
			}
			fn phase_for_scenario(s: &str) -> Option<&'static str> {
				match s {
					"indexing-discovery" => Some("Discovery"),
					"aggregation" => Some("Processing"),
					"content-identification" => Some("Content Identification"),
					_ => None,
				}
			}
			#[derive(Clone)]
			struct BestRow {
				files_per_s: f64,
				files: u64,
				dirs: u64,
				gb: f64,
				errors: u64,
				duration_s: f64,
				recipe: String,
			}
			let mut best_by_hw_phase: HashMap<(String, &'static str), BestRow> = HashMap::new();
			for (sc, rc, du, f, fps, d, _dps, gb, e) in &rows {
				let Some(phase) = phase_for_scenario(sc) else {
					continue;
				};
				let Some(hw) = label_for_recipe(rc) else {
					continue;
				};
				let key = (phase.to_string(), hw);
				let candidate = BestRow {
					files_per_s: *fps,
					files: *f,
					dirs: *d,
					gb: *gb,
					errors: *e,
					duration_s: *du,
					recipe: rc.clone(),
				};
				match best_by_hw_phase.get(&key) {
					Some(existing) if existing.files_per_s >= *fps => {}
					_ => {
						best_by_hw_phase.insert(key, candidate);
					}
				}
			}
			let mut entries: Vec<(String, &'static str, BestRow)> = best_by_hw_phase
				.into_iter()
				.map(|((phase, hw), row)| (phase, hw, row))
				.collect();
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
			s.push_str("Phase,Hardware,Files_per_s,GB_per_s,Files,Dirs,GB,Errors,Recipe\n");
			for (phase, hw, row) in entries {
				let gbps = if row.duration_s > 0.0 {
					row.gb / row.duration_s
				} else {
					0.0
				};
				s.push_str(&format!(
					"{},{},{:.1},{:.2},{},{},{:.2},{},{}\n",
					phase,
					hw,
					row.files_per_s,
					gbps,
					row.files,
					row.dirs,
					row.gb,
					row.errors,
					row.recipe
				));
			}
			s
		}
		"csv" => {
			let mut s = String::new();
			s.push_str(
				"scenario,recipe,duration_s,files,files_per_s,dirs,dirs_per_s,total_gb,errors\n",
			);
			for (sc, rc, du, f, fps, d, dps, gb, e) in &rows {
				s.push_str(&format!(
					"{},{},{:.2},{},{:.1},{},{:.1},{:.2},{}\n",
					sc, rc, du, f, fps, d, dps, gb, e
				));
			}
			s
		}
		_ => {
			// markdown
			let mut s = String::new();
			s.push_str("| scenario | recipe | duration (s) | files | files/s | dirs | dirs/s | total GB | errors |\n");
			s.push_str("|---|---|---:|---:|---:|---:|---:|---:|---:|\n");
			for (sc, rc, du, f, fps, d, dps, gb, e) in &rows {
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
