//! CLI tool for analyzing Spacedrive log files.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use log_analyzer::LogAnalyzer;

#[derive(Parser)]
#[command(name = "log-analyzer")]
#[command(about = "Analyze Spacedrive log files", long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Analyze a log file and generate report
	Analyze {
		/// Path to log file
		#[arg(value_name = "FILE")]
		file: PathBuf,

		/// Output format
		#[arg(short, long, default_value = "markdown")]
		format: OutputFormat,

		/// Output file (defaults to stdout)
		#[arg(short, long)]
		output: Option<PathBuf>,

		/// Store analysis to database
		#[arg(short, long)]
		database: Option<PathBuf>,
	},

	/// Generate timeline view
	Timeline {
		/// Path to log file
		#[arg(value_name = "FILE")]
		file: PathBuf,

		/// Output file (defaults to stdout)
		#[arg(short, long)]
		output: Option<PathBuf>,
	},

	/// Show statistics
	Stats {
		/// Path to log file
		#[arg(value_name = "FILE")]
		file: PathBuf,
	},

	/// Generate condensed timeline (collapsed sequences as readable log)
	Condense {
		/// Path to log file
		#[arg(value_name = "FILE")]
		file: PathBuf,

		/// Output file (defaults to input.condensed.log)
		#[arg(short, long)]
		output: Option<PathBuf>,

		/// Minimum repetitions to collapse (default: 10)
		#[arg(short, long, default_value = "10")]
		min_reps: usize,
	},

	/// Generate phase-based summary (aggregates by time windows)
	Phases {
		/// Path to log file
		#[arg(value_name = "FILE")]
		file: PathBuf,

		/// Phase duration in seconds (default: 5)
		#[arg(short, long, default_value = "5")]
		duration: u64,

		/// Output file (defaults to stdout)
		#[arg(short, long)]
		output: Option<PathBuf>,
	},
}

#[derive(Clone, Copy)]
enum OutputFormat {
	Markdown,
	Json,
}

impl std::str::FromStr for OutputFormat {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"markdown" | "md" => Ok(Self::Markdown),
			"json" => Ok(Self::Json),
			_ => Err(format!("Unknown format: {}", s)),
		}
	}
}

fn main() -> Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
		)
		.init();

	let cli = Cli::parse();

	match cli.command {
		Commands::Analyze {
			file,
			format,
			output,
			database,
		} => {
			println!("Analyzing log file: {:?}", file);

			let mut analyzer = LogAnalyzer::from_file(&file)?;

			let stats = analyzer.compression_stats();
			println!(
				"Parsed {} logs into {} templates and {} groups",
				analyzer.log_count(),
				analyzer.template_count(),
				analyzer.group_count()
			);
			println!("Detected {} sequences", analyzer.sequences().len());
			println!(
				"Final compressed count: {} (compression: {:.1}%)",
				stats.final_count,
				stats.compression_ratio * 100.0
			);

			// Store to database if requested
			if let Some(db_path) = database {
				println!("Storing analysis to database: {:?}", db_path);
				analyzer.store_to_db(&db_path)?;
			}

			// Generate output
			let content = match format {
				OutputFormat::Markdown => analyzer.generate_markdown_report()?,
				OutputFormat::Json => analyzer.export_json()?,
			};

			// Write output
			if let Some(output_path) = output {
				std::fs::write(output_path, content)?;
			} else {
				println!("\n{}", content);
			}
		}

		Commands::Timeline { file, output } => {
			println!("Generating timeline for: {:?}", file);

			let analyzer = LogAnalyzer::from_file(&file)?;
			let timeline = analyzer.generate_timeline()?;

			let content = serde_json::to_string_pretty(&timeline)?;

			if let Some(output_path) = output {
				std::fs::write(output_path, content)?;
			} else {
				println!("\n{}", content);
			}
		}

		Commands::Stats { file } => {
			println!("Analyzing statistics for: {:?}", file);

			let analyzer = LogAnalyzer::from_file(&file)?;

			let stats = analyzer.compression_stats();

			println!("\n┌─ Statistics ─────────────────────────────────────┐");
			println!("│");
			println!("│  Total log lines:     {}", analyzer.log_count());
			println!("│  Unique templates:    {}", analyzer.template_count());
			println!("│  Collapsed groups:    {}", analyzer.group_count());
			println!("│  Detected sequences:  {}", analyzer.sequences().len());
			println!("│  Final count:         {}", stats.final_count);
			println!(
				"│  Compression ratio:   {:.1}%",
				stats.compression_ratio * 100.0
			);
			println!("│");
			println!("└───────────────────────────────────────────────────┘");

			// Show top templates
			let mut sorted_templates: Vec<_> = analyzer.templates().iter().collect();
			sorted_templates.sort_by(|a, b| b.total_count.cmp(&a.total_count));

			println!("\n┌─ Top Templates by Frequency ─────────────────────┐");
			for (i, template) in sorted_templates.iter().take(5).enumerate() {
				println!("│");
				println!(
					"│  {}. [{}×] {}",
					i + 1,
					template.total_count,
					template.module
				);
				println!("│     {}", template.example);
			}
			println!("│");
			println!("└───────────────────────────────────────────────────┘");
		}

		Commands::Condense {
			file,
			output,
			min_reps,
		} => {
			println!("Generating condensed timeline for: {:?}", file);

			let analyzer = LogAnalyzer::from_file(&file)?;

			let condensed = log_analyzer::output::generate_condensed_timeline(&analyzer, min_reps)?;

			let output_path = output.unwrap_or_else(|| {
				let mut path = file.clone();
				path.set_extension("condensed.log");
				path
			});

			std::fs::write(&output_path, condensed)?;
			println!("Condensed timeline written to: {:?}", output_path);

			let stats = analyzer.compression_stats();
			println!("Original: {} lines", analyzer.log_count());
			println!(
				"Condensed: {} lines ({:.1}% compression)",
				stats.final_count,
				stats.compression_ratio * 100.0
			);
		}

		Commands::Phases {
			file,
			duration,
			output,
		} => {
			println!("Generating phase summary for: {:?}", file);
			println!("Phase duration: {}s windows", duration);

			let analyzer = LogAnalyzer::from_file(&file)?;
			let summary = analyzer.generate_phase_summary(duration)?;

			if let Some(output_path) = output {
				std::fs::write(output_path, &summary)?;
				println!("Phase summary written to output file");
			} else {
				println!("\n{}", summary);
			}
		}
	}

	Ok(())
}