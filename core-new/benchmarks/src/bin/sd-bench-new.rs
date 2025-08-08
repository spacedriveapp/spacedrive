use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sd_bench::generator::DatasetGenerator;
use sd_bench::mod_new as bench;

#[derive(Parser, Debug)]
#[command(name = "sd-bench-new")]
#[command(about = "Spacedrive benchmarking harness (modular)", long_about = None)]
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
	},
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
		.with_max_level(tracing::Level::INFO)
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.try_init();
}

async fn mkdata(recipe_path: PathBuf) -> Result<()> {
	let recipe_str = std::fs::read_to_string(&recipe_path)
		.with_context(|| format!("reading recipe {recipe_path:?}"))?;
	let recipe: bench::recipe::Recipe =
		serde_yaml::from_str(&recipe_str).context("parsing recipe yaml")?;

	let gen = bench::generator::FileSystemGenerator::default();
	gen.generate(&recipe).await?;
	tracing::info!(name = %recipe.name, "generated dataset (modular)");
	Ok(())
}

async fn run_scenario(
	scenario: String,
	recipe_path: PathBuf,
	out_json: Option<PathBuf>,
) -> Result<()> {
	tracing::info!(%scenario, recipe = %recipe_path.display(), "run scenario (modular)");

	// Boot isolated core for benchmark
	let boot = bench::core_boot::boot_isolated_with_core(&scenario, None).await?;

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
	let results = bench::runner::run_scenario(
		&boot,
		&bench::generator::FileSystemGenerator::default(),
		&mut *scenario_impl,
		&reporters,
		&recipe,
		out_json.as_deref(),
	)
	.await?;

	// Always print a brief summary to stdout; if writing to file, also show where
	if let Some(path) = &out_json {
		println!("\nWrote summary to {}", path.display());
	}
	for r in &results {
		println!("\nBenchmark summary (job {}):", r.id);
		if r.duration_s > 0.0 {
			println!("- Duration: {:.2}s", r.duration_s);
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

async fn run_all(scenario: String, recipes_dir: PathBuf, out_dir: PathBuf) -> Result<()> {
	std::fs::create_dir_all(&out_dir)?;
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
		mkdata(recipe.clone()).await?;
		let stem = recipe
			.file_stem()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string();
		let out_json = out_dir.join(format!("{}-{}.json", stem, scenario));
		run_scenario(scenario.clone(), recipe.clone(), Some(out_json)).await?;
	}

	println!("\nCompleted RunAll for {} recipes.", scenario);
	Ok(())
}
