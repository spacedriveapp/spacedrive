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
		/// Scenario names to run (e.g., indexing-discovery, aggregation, content-identification). If not specified, runs all scenarios.
		#[arg(short, long)]
		scenarios: Option<Vec<String>>,
		/// Directory containing recipe YAML files
		#[arg(long, default_value = "benchmarks/recipes")]
		recipes_dir: PathBuf,
		/// Output directory for JSON summaries
		#[arg(long, default_value = "benchmarks/results")]
		out_dir: PathBuf,
		/// Skip dataset generation (assume recipes already generated)
		#[arg(long, default_value_t = false)]
		skip_generate: bool,
		/// Dataset locations to benchmark (e.g., "/Volumes/HDD" "/Users/me/benchdata")
		/// Hardware type will be automatically detected from the volume
		#[arg(long, num_args = 1.., value_delimiter = ' ')]
		locations: Option<Vec<String>>,
		/// Only include recipe files whose filename matches this regex (applied to file stem), e.g. ^shape_
		#[arg(long)]
		recipe_filter: Option<String>,
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
			scenarios,
			recipes_dir,
			out_dir,
			skip_generate,
			locations,
			recipe_filter,
		} => {
			run_all(
				scenarios,
				recipes_dir,
				out_dir,
				skip_generate,
				locations,
				recipe_filter,
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
		for loc in &mut recipe.locations {
			if loc.path.is_relative() {
				// If the location path starts with "benchdata/", replace it with just the recipe name
				if let Some(path_str) = loc.path.to_str() {
					if path_str.starts_with("benchdata/") {
						// Extract the part after "benchdata/"
						let recipe_subdir = path_str.strip_prefix("benchdata/").unwrap_or(path_str);
						loc.path = r.join(recipe_subdir);
					} else {
						loc.path = r.join(&loc.path);
					}
				} else {
					loc.path = r.join(&loc.path);
				}
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
	// Extract hardware hint from output filename if present
	let hardware_hint = out_json.as_ref().and_then(|path| {
		path.file_stem()
			.and_then(|stem| stem.to_str())
			.and_then(|s| {
				// Extract suffix after last hyphen (e.g., "nvme" from "shape_small-indexing-discovery-nvme")
				s.rsplit('-')
					.next()
					.filter(|suffix| matches!(*suffix, "nvme" | "hdd" | "ssd" | "nas" | "usb"))
					.map(|s| s.to_string())
			})
	});
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

	// Set hardware hint if available
	if hardware_hint.is_some() {
		scenario_impl.set_hardware_hint(hardware_hint.clone());
	}

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
	scenarios: Option<Vec<String>>,
	recipes_dir: PathBuf,
	out_dir: PathBuf,
	skip_generate: bool,
	locations: Option<Vec<String>>,
	recipe_filter: Option<String>,
) -> Result<()> {
	// Parse locations and detect hardware automatically
	let location_configs: Vec<(PathBuf, String)> = if let Some(locs) = locations {
		locs.into_iter()
			.map(|loc| {
				let path = PathBuf::from(&loc);
				// Use the existing hardware detection function
				let hardware_label =
					crate::metrics::derive_hardware_label_from_paths(&[path.clone()])
						.unwrap_or_else(|| "Unknown".to_string());

				// Extract a simple tag from the hardware label for filename
				let tag = if hardware_label.contains("NVMe") {
					"nvme"
				} else if hardware_label.contains("HDD") {
					"hdd"
				} else if hardware_label.contains("SSD") && !hardware_label.contains("NVMe") {
					"ssd"
				} else if hardware_label.contains("Network") {
					"nas"
				} else {
					"unknown"
				};

				println!("Detected {} as {}", path.display(), hardware_label);
				(path, tag.to_string())
			})
			.collect()
	} else {
		// Default to current directory
		vec![(PathBuf::from("."), "local".to_string())]
	};

	// Determine which scenarios to run
	let scenarios_to_run = if let Some(s) = scenarios {
		s
	} else {
		// Default to all scenarios
		vec![
			"indexing_discovery".to_string(),
			"aggregation".to_string(),
			"content_identification".to_string(),
		]
	};
	std::fs::create_dir_all(&out_dir)?;

	// Collect all recipe files
	let mut recipe_paths: Vec<PathBuf> = Vec::new();
	for entry in std::fs::read_dir(&recipes_dir)? {
		let entry = entry?;
		let path = entry.path();
		if let Some(ext) = path.extension() {
			if ext == "yaml" || ext == "yml" {
				recipe_paths.push(path);
			}
		}
	}
	recipe_paths.sort();
	if recipe_paths.is_empty() {
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

	// Filter recipes based on regex
	let filtered_recipes: Vec<PathBuf> = recipe_paths
		.into_iter()
		.filter(|recipe| {
			if let Some(rx) = &matcher {
				let stem = recipe
					.file_stem()
					.unwrap_or_default()
					.to_string_lossy()
					.to_string();
				rx.is_match(&stem)
			} else {
				true
			}
		})
		.collect();

	println!(
		"Running {} scenarios on {} locations for {} recipes",
		scenarios_to_run.len(),
		location_configs.len(),
		filtered_recipes.len()
	);
	println!("Scenarios: {:?}", scenarios_to_run);
	println!("Locations: {:?}", location_configs);
	println!("Output directory: {}", out_dir.display());

	let mut total_runs = 0;

	// Iterate over all combinations of location, scenario, and recipe
	for (location_path, hardware_tag) in &location_configs {
		println!(
			"\n=== Running benchmarks on {} ({}) ===",
			location_path.display(),
			hardware_tag
		);

		for scenario in &scenarios_to_run {
			println!("\n--- Scenario: {} ---", scenario);

			for recipe_path in &filtered_recipes {
				let recipe_stem = recipe_path
					.file_stem()
					.unwrap_or_default()
					.to_string_lossy()
					.to_string();

				println!("\nProcessing recipe: {}", recipe_stem);

				// Read and parse recipe
				let recipe_str = std::fs::read_to_string(&recipe_path)
					.with_context(|| format!("reading recipe {recipe_path:?}"))?;
				let mut parsed: bench::recipe::Recipe =
					serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;

				// Apply location path to recipe
				let parsed = apply_dataset_root(parsed, Some(location_path.clone()));

				// Generate dataset if needed
				if !skip_generate {
					let all_marked = parsed
						.locations
						.iter()
						.all(|loc| loc.path.join(".sd-bench-generated").exists());

					if !all_marked {
						println!("Generating dataset at {}", location_path.display());
						mkdata(recipe_path.clone(), Some(location_path.clone())).await?;
					} else {
						println!("Dataset already exists (markers found)");
					}
				}

				// Verify paths exist
				let mut missing: Vec<String> = Vec::new();
				for loc in &parsed.locations {
					if !loc.path.exists() {
						missing.push(loc.path.display().to_string());
					}
				}
				if !missing.is_empty() {
					eprintln!(
						"Skipping {}: missing paths ({})",
						recipe_stem,
						missing.join(", ")
					);
					continue;
				}

				// Construct output filename with hardware tag
				let out_json = out_dir.join(format!(
					"{}-{}-{}.json",
					recipe_stem, scenario, hardware_tag
				));

				println!("Executing -> {}", out_json.display());

				run_scenario(
					scenario.clone(),
					recipe_path.clone(),
					Some(out_json),
					Some(location_path.clone()),
				)
				.await?;

				total_runs += 1;
			}
		}
	}

	println!("\n=== Completed {} benchmark runs ===", total_runs);
	Ok(())
}

async fn results_table(results_dir: PathBuf, out: Option<PathBuf>, format: &str) -> Result<()> {
	// Parse format to determine which reporter to use
	let reporter: Box<dyn bench::reporting::Reporter> = match format.to_lowercase().as_str() {
		"csv" => Box::new(bench::reporting::CsvReporter::default()),
		"json" => Box::new(bench::reporting::JsonSummaryReporter::default()),
		"markdown" | "md" => {
			// For now, we'll handle markdown separately as it's not a full reporter
			return render_markdown_table(results_dir, out).await;
		}
		_ => {
			return Err(anyhow::anyhow!(
				"Unknown format '{}'. Available formats: csv, whitepaper, json, markdown",
				format
			));
		}
	};

	// Collect all benchmark runs from JSON files
	let mut all_runs: Vec<bench::metrics::BenchmarkRun> = Vec::new();

	for entry in std::fs::read_dir(&results_dir)? {
		let entry = entry?;
		let path = entry.path();
		if path.extension().and_then(|e| e.to_str()) != Some("json") {
			continue;
		}

		let content = std::fs::read_to_string(&path)?;
		let parsed: serde_json::Value = serde_json::from_str(&content)?;

		// Extract runs array from the JSON
		if let Some(runs) = parsed.get("runs").and_then(|v| v.as_array()) {
			for run in runs {
				if let Ok(benchmark_run) =
					serde_json::from_value::<bench::metrics::BenchmarkRun>(run.clone())
				{
					all_runs.push(benchmark_run);
				}
			}
		}
	}

	if all_runs.is_empty() {
		return Err(anyhow::anyhow!(
			"No benchmark runs found in {}",
			results_dir.display()
		));
	}

	// Use reporter to render the output
	if let Some(out_path) = out {
		if let Some(parent) = out_path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		reporter.render(&all_runs, &out_path)?;
		println!("Wrote results table to {}", out_path.display());
	} else {
		// For stdout, create a temp file and print its contents
		let temp_path =
			std::env::temp_dir().join(format!("bench_results_{}.tmp", std::process::id()));
		reporter.render(&all_runs, &temp_path)?;
		let content = std::fs::read_to_string(&temp_path)?;
		println!("{}", content);
		std::fs::remove_file(temp_path).ok();
	}

	Ok(())
}

// Temporary function to handle markdown rendering until we create a proper markdown reporter
async fn render_markdown_table(results_dir: PathBuf, out: Option<PathBuf>) -> Result<()> {
	let mut all_runs: Vec<bench::metrics::BenchmarkRun> = Vec::new();

	for entry in std::fs::read_dir(&results_dir)? {
		let entry = entry?;
		let path = entry.path();
		if path.extension().and_then(|e| e.to_str()) != Some("json") {
			continue;
		}

		let content = std::fs::read_to_string(&path)?;
		let parsed: serde_json::Value = serde_json::from_str(&content)?;

		if let Some(runs) = parsed.get("runs").and_then(|v| v.as_array()) {
			for run in runs {
				if let Ok(benchmark_run) =
					serde_json::from_value::<bench::metrics::BenchmarkRun>(run.clone())
				{
					all_runs.push(benchmark_run);
				}
			}
		}
	}

	let mut rows = Vec::new();
	rows.push("| scenario | recipe | duration (s) | files | files/s | dirs | dirs/s | total GB | errors |".to_string());
	rows.push("|---|---|---:|---:|---:|---:|---:|---:|---:|".to_string());

	for run in all_runs {
		let (scenario, meta, files, files_per_s, dirs, dirs_per_s, total_gb, errors, durations) =
			match &run {
				bench::metrics::BenchmarkRun::IndexingDiscovery {
					meta,
					files,
					files_per_s,
					dirs,
					dirs_per_s,
					total_gb,
					errors,
					durations,
				} => (
					"indexing-discovery",
					meta,
					files,
					files_per_s,
					dirs,
					dirs_per_s,
					total_gb,
					errors,
					durations,
				),
				bench::metrics::BenchmarkRun::Aggregation {
					meta,
					files,
					files_per_s,
					dirs,
					dirs_per_s,
					total_gb,
					errors,
					durations,
				} => (
					"aggregation",
					meta,
					files,
					files_per_s,
					dirs,
					dirs_per_s,
					total_gb,
					errors,
					durations,
				),
				bench::metrics::BenchmarkRun::ContentIdentification {
					meta,
					files,
					files_per_s,
					dirs,
					dirs_per_s,
					total_gb,
					errors,
					durations,
				} => (
					"content-identification",
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

		let duration = durations.total_s.unwrap_or(0.0);
		rows.push(format!(
			"| {} | {} | {:.2} | {} | {:.1} | {} | {:.1} | {:.2} | {} |",
			scenario,
			&meta.recipe_name,
			duration,
			files,
			files_per_s,
			dirs,
			dirs_per_s,
			total_gb,
			errors
		));
	}

	let output = rows.join("\n") + "\n";

	if let Some(out_path) = out {
		if let Some(parent) = out_path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		std::fs::write(&out_path, &output)?;
		println!("Wrote results table to {}", out_path.display());
	} else {
		println!("\n{}", output);
	}

	Ok(())
}
