//! Log analysis system for Spacedrive.
//!
//! This crate provides tools for parsing, analyzing, and querying large structured log files.
//! It identifies patterns, collapses repetitions, and enables queryable analysis of log data.
//!
//! # Overview
//!
//! The log analyzer works in several stages:
//!
//! 1. **Parse**: Extract structured components from log lines
//! 2. **Pattern**: Identify templates and variable positions
//! 3. **Collapse**: Group repetitions into summary statistics
//! 4. **Store**: Save to queryable database
//! 5. **Analyze**: Generate timelines, statistics, and reports
//!
//! # Example
//!
//! ```no_run
//! use log_analyzer::LogAnalyzer;
//!
//! let analyzer = LogAnalyzer::from_file("test.log")?;
//! let timeline = analyzer.generate_timeline()?;
//! println!("Analysis complete: {} unique patterns", analyzer.template_count());
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod analysis;
pub mod collapse;
pub mod database;
pub mod output;
pub mod parser;
pub mod pattern;
pub mod sequence;

mod types;

pub use sequence::{calculate_compression, detect_sequences, CompressionStats, SequencePattern};
pub use types::{LogGroup, LogLevel, ParsedLog, Template, Variable, VariableStat, VariableType};

use std::path::Path;

use anyhow::Result;

/// Main entry point for log analysis.
#[derive(Debug)]
pub struct LogAnalyzer {
	logs: Vec<ParsedLog>,
	templates: Vec<Template>,
	groups: Vec<LogGroup>,
	sequences: Vec<SequencePattern>,
	db_path: Option<String>,
}

impl LogAnalyzer {
	/// Create analyzer from a log file.
	pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
		let logs = parser::parse_file(path)?;
		let mut analyzer = Self {
			logs,
			templates: Vec::new(),
			groups: Vec::new(),
			sequences: Vec::new(),
			db_path: None,
		};
		analyzer.analyze()?;
		Ok(analyzer)
	}

	/// Create analyzer from log content string.
	pub fn from_string(content: &str) -> Result<Self> {
		let logs = parser::parse_string(content)?;
		let mut analyzer = Self {
			logs,
			templates: Vec::new(),
			groups: Vec::new(),
			sequences: Vec::new(),
			db_path: None,
		};
		analyzer.analyze()?;
		Ok(analyzer)
	}

	/// Perform analysis: detect patterns and collapse repetitions.
	fn analyze(&mut self) -> Result<()> {
		self.templates = pattern::detect_templates(&self.logs)?;
		self.groups = collapse::collapse_logs(&self.logs, &self.templates)?;
		self.sequences = sequence::detect_sequences(&self.groups);
		Ok(())
	}

	/// Get number of unique templates.
	pub fn template_count(&self) -> usize {
		self.templates.len()
	}

	/// Get number of collapsed groups.
	pub fn group_count(&self) -> usize {
		self.groups.len()
	}

	/// Get total log line count.
	pub fn log_count(&self) -> usize {
		self.logs.len()
	}

	/// Get compression ratio (accounting for sequences).
	pub fn compression_ratio(&self) -> f64 {
		let stats = self.compression_stats();
		stats.compression_ratio
	}

	/// Get detailed compression statistics.
	pub fn compression_stats(&self) -> CompressionStats {
		sequence::calculate_compression(self.logs.len(), self.groups.len(), &self.sequences)
	}

	/// Get detected sequences.
	pub fn sequences(&self) -> &[SequencePattern] {
		&self.sequences
	}

	/// Get all templates.
	pub fn templates(&self) -> &[Template] {
		&self.templates
	}

	/// Get all collapsed groups.
	pub fn groups(&self) -> &[LogGroup] {
		&self.groups
	}

	/// Generate timeline view.
	pub fn generate_timeline(&self) -> Result<analysis::Timeline> {
		analysis::generate_timeline(&self.logs, &self.groups)
	}

	/// Store analysis to database.
	pub fn store_to_db<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
		database::store_analysis(
			path.as_ref(),
			&self.templates,
			&self.logs,
			&self.groups,
			&self.sequences,
		)?;
		self.db_path = Some(path.as_ref().to_string_lossy().to_string());
		Ok(())
	}

	/// Generate markdown report.
	pub fn generate_markdown_report(&self) -> Result<String> {
		output::generate_markdown_report(&self.logs, &self.templates, &self.groups, &self.sequences)
	}

	/// Export to JSON.
	pub fn export_json(&self) -> Result<String> {
		output::export_json(&self.templates, &self.groups)
	}

	/// Generate phase-based summary (aggregates by time windows).
	pub fn generate_phase_summary(&self, phase_duration_secs: u64) -> Result<String> {
		output::generate_phase_summary(self, phase_duration_secs)
	}
}
