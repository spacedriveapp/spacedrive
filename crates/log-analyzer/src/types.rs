//! Core data structures for log analysis.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Parsed log line with extracted components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedLog {
	pub timestamp: DateTime<Utc>,
	pub level: LogLevel,
	pub thread_id: Option<String>,
	pub module: String,
	pub message: String,
	pub raw: String,
	#[serde(skip)]
	pub template_id: Option<u64>,
}

/// Log level enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogLevel {
	Trace,
	Debug,
	Info,
	Warn,
	Error,
}

impl std::str::FromStr for LogLevel {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_uppercase().as_str() {
			"TRACE" => Ok(Self::Trace),
			"DEBUG" => Ok(Self::Debug),
			"INFO" => Ok(Self::Info),
			"WARN" | "WARNING" => Ok(Self::Warn),
			"ERROR" => Ok(Self::Error),
			_ => Err(format!("Unknown log level: {}", s)),
		}
	}
}

impl LogLevel {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Trace => "TRACE",
			Self::Debug => "DEBUG",
			Self::Info => "INFO",
			Self::Warn => "WARN",
			Self::Error => "ERROR",
		}
	}
}

/// Template representing a log pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
	pub id: u64,
	pub module: String,
	pub level: LogLevel,
	pub static_parts: Vec<String>,
	pub variables: Vec<Variable>,
	#[serde(skip)]
	pub regex_pattern: String,
	pub example: String,
	pub total_count: usize,
	pub first_seen: Option<DateTime<Utc>>,
	pub last_seen: Option<DateTime<Utc>>,
}

/// Variable in a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
	pub name: String,
	pub position: usize,
	pub var_type: VariableType,
}

/// Variable type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableType {
	Uuid,
	Number,
	Timestamp,
	HLC,
	String,
	Path,
	Duration,
}

impl VariableType {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Uuid => "uuid",
			Self::Number => "number",
			Self::Timestamp => "timestamp",
			Self::HLC => "hlc",
			Self::String => "string",
			Self::Path => "path",
			Self::Duration => "duration",
		}
	}
}

/// Collapsed group of log instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogGroup {
	pub template_id: u64,
	pub count: usize,
	pub start_time: DateTime<Utc>,
	pub end_time: DateTime<Utc>,
	pub duration_ms: i64,
	pub variable_stats: HashMap<String, VariableStat>,
	pub sample_indices: Vec<usize>,
}

/// Statistics for a variable across instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableStat {
	Constant(String),
	Range { min: String, max: String },
	Unique(HashSet<String>),
	Distribution { total: usize, unique: usize },
}

impl VariableStat {
	pub fn format(&self) -> String {
		match self {
			Self::Constant(val) => format!("{} (constant)", val),
			Self::Range { min, max } => format!("{} - {} (range)", min, max),
			Self::Unique(vals) => format!("{} unique values", vals.len()),
			Self::Distribution { total, unique } => {
				format!("{} unique values out of {} total", unique, total)
			}
		}
	}
}

/// Token for message parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
	Word(String),
	Punctuation(char),
}

impl Token {
	pub fn as_str(&self) -> &str {
		match self {
			Self::Word(s) => s.as_str(),
			Self::Punctuation(c) => match c {
				'(' => "(",
				')' => ")",
				'{' => "{",
				'}' => "}",
				'[' => "[",
				']' => "]",
				',' => ",",
				':' => ":",
				'=' => "=",
				_ => "",
			},
		}
	}
}
