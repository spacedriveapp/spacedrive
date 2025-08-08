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
	},
	/// Generate benchmark datasets for all recipes in a directory
	MkdataAll {
		/// Directory containing recipe YAML files
		#[arg(long, default_value = "benchmarks/recipes")]
		recipes_dir: PathBuf,
	},
	/// Run a benchmark scenario
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
		/// Skip dataset generation (assume recipes already generated)
		#[arg(long, default_value_t = false)]
		skip_generate: bool,
	},
}

pub async fn run(cli: Cli) -> Result<()> {
	match cli.command {
		Commands::Mkdata { recipe } => mkdata(recipe).await?,
		Commands::MkdataAll { recipes_dir } => mkdata_all(recipes_dir).await?,
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
			skip_generate,
		} => run_all(scenario, recipes_dir, out_dir, skip_generate).await?,
	}
	Ok(())
}

async fn mkdata(recipe_path: PathBuf) -> Result<()> {
	println!("Generating dataset for {}...", recipe_path.display());
	use std::io::Write as _;
	let _ = std::io::stdout().flush();
	let recipe_str = std::fs::read_to_string(&recipe_path)
		.with_context(|| format!("reading recipe {recipe_path:?}"))?;
	let recipe: bench::recipe::Recipe =
		serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;

	let gen = bench::generator::FileSystemGenerator::default();
	gen.generate(&recipe).await?;
	println!("Generated dataset: {}", recipe.name);
	let _ = std::io::stdout().flush();
	Ok(())
}

async fn mkdata_all(recipes_dir: PathBuf) -> Result<()> {
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
	for recipe in entries {
		mkdata(recipe).await?;
	}
	println!("\nCompleted MkdataAll for {}", recipes_dir.display());
	Ok(())
}

async fn run_scenario(
	scenario: String,
	recipe_path: PathBuf,
	out_json: Option<PathBuf>,
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

	for recipe in entries {
		println!("\n---");
		println!("Preparing {}", recipe.display());
		if !skip_generate {
			mkdata(recipe.clone()).await?;
		} else {
			println!("Skipping dataset generation (--skip-generate)");
		}
		// Preflight: ensure recipe location paths exist before running scenario
		let recipe_str = std::fs::read_to_string(&recipe)
			.with_context(|| format!("reading recipe {recipe:?}"))?;
		let parsed: bench::recipe::Recipe =
			serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;
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
		let out_json = out_dir.join(format!("{}-{}.json", stem, scenario));
		println!("Executing scenario -> {}", out_json.display());
		run_scenario(scenario.clone(), recipe.clone(), Some(out_json)).await?;
	}

	println!("\nCompleted RunAll for {} recipes.", scenario);
	Ok(())
}
