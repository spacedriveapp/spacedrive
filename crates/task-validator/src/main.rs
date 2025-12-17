#![allow(warnings)]
// File: task-validator/src/main.rs
use clap::{Parser, Subcommand};
use comfy_table::{Cell, Table};
use glob::glob;
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::{self, Command};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// List and filter tasks
	List {
		#[arg(short, long)]
		status: Option<String>,
		#[arg(short, long)]
		assignee: Option<String>,
		#[arg(short, long)]
		priority: Option<String>,
		#[arg(long)]
		tag: Option<String>,
		#[arg(long, help = "Sort by field (id, title, status, priority, assignee)")]
		sort_by: Option<String>,
		#[arg(short, long, help = "Reverse sort order")]
		reverse: bool,
	},
	/// Validate staged task files (for git hook)
	Validate,
	/// Export tasks to JSON
	Export {
		#[arg(short, long, help = "Output file path")]
		output: String,
	},
}

/// A struct that matches the YAML Front Matter schema.
#[derive(Debug, Deserialize)]
struct TaskFrontMatter {
	id: String,
	title: String,
	status: String,
	assignee: String,
	parent: Option<String>,
	priority: String,
	tags: Option<Vec<String>>,
	whitepaper: Option<String>,
}

/// Exportable task with description for JSON output.
#[derive(Debug, Serialize)]
struct ExportableTask {
	id: String,
	title: String,
	status: String,
	assignee: String,
	priority: String,
	tags: Vec<String>,
	whitepaper: Option<String>,
	category: String,
	description: String,
	parent: Option<String>,
	file: String,
}

/// Root export structure.
#[derive(Debug, Serialize)]
struct TaskExport {
	tasks: Vec<ExportableTask>,
	categories: Vec<String>,
	generated_at: String,
}

fn main() {
	let cli = Cli::parse();

	match &cli.command {
		Commands::List {
			status,
			assignee,
			priority,
			tag,
			sort_by,
			reverse,
		} => {
			if let Err(e) = list_tasks(status, assignee, priority, tag, sort_by, *reverse) {
				eprintln!("Error listing tasks: {}", e);
				process::exit(1);
			}
		}
		Commands::Validate => {
			if let Err(e) = validate_tasks() {
				eprintln!("Error validating tasks: {}", e);
				process::exit(1);
			}
		}
		Commands::Export { output } => {
			if let Err(e) = export_tasks(output) {
				eprintln!("Error exporting tasks: {}", e);
				process::exit(1);
			}
		}
	}
}

fn list_tasks(
	status_filter: &Option<String>,
	assignee_filter: &Option<String>,
	priority_filter: &Option<String>,
	tag_filter: &Option<String>,
	sort_by: &Option<String>,
	reverse: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut tasks = Vec::new();

	// Support both old flat structure and new subdirectory structure
	for entry in glob(".tasks/**/*.md")? {
		let path = entry?;

		// Skip non-task files (like Claude.md, README.md, etc.)
		let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
		if !file_name.contains('-') || file_name == "Claude.md" {
			continue;
		}

		let content = fs::read_to_string(&path)?;

		if content.starts_with("---") {
			if let Some(front_matter_str) = content.split("---").nth(1) {
				match serde_yaml::from_str::<TaskFrontMatter>(front_matter_str) {
					Ok(front_matter) => tasks.push(front_matter),
					Err(e) => eprintln!("Error parsing YAML in {:?}: {}", path, e),
				}
			}
		}
	}

	let mut filtered_tasks = tasks
		.into_iter()
		.filter(|task| {
			let status_match = status_filter
				.as_ref()
				.map_or(true, |s| task.status.to_lowercase() == s.to_lowercase());
			let assignee_match = assignee_filter
				.as_ref()
				.map_or(true, |a| task.assignee.to_lowercase() == a.to_lowercase());
			let priority_match = priority_filter
				.as_ref()
				.map_or(true, |p| task.priority.to_lowercase() == p.to_lowercase());
			let tag_match = tag_filter.as_ref().map_or(true, |t| {
				task.tags.as_ref().map_or(false, |tags| {
					tags.iter()
						.any(|tag| tag.to_lowercase() == t.to_lowercase())
				})
			});
			status_match && assignee_match && priority_match && tag_match
		})
		.collect::<Vec<_>>();

	// Sort the tasks if sort_by is provided
	if let Some(sort_field) = sort_by {
		match sort_field.to_lowercase().as_str() {
			"id" => filtered_tasks.sort_by(|a, b| {
				let cmp = a.id.cmp(&b.id);
				if reverse {
					cmp.reverse()
				} else {
					cmp
				}
			}),
			"title" => filtered_tasks.sort_by(|a, b| {
				let cmp = a.title.to_lowercase().cmp(&b.title.to_lowercase());
				if reverse {
					cmp.reverse()
				} else {
					cmp
				}
			}),
			"status" => filtered_tasks.sort_by(|a, b| {
				let cmp = a.status.to_lowercase().cmp(&b.status.to_lowercase());
				if reverse {
					cmp.reverse()
				} else {
					cmp
				}
			}),
			"priority" => filtered_tasks.sort_by(|a, b| {
				// Sort priority: critical > high > medium > low
				let priority_value = |p: &str| match p.to_lowercase().as_str() {
					"critical" => 0,
					"high" => 1,
					"medium" => 2,
					"low" => 3,
					_ => 4,
				};
				let cmp = priority_value(&a.priority).cmp(&priority_value(&b.priority));
				if reverse {
					cmp.reverse()
				} else {
					cmp
				}
			}),
			"assignee" => filtered_tasks.sort_by(|a, b| {
				let cmp = a.assignee.to_lowercase().cmp(&b.assignee.to_lowercase());
				if reverse {
					cmp.reverse()
				} else {
					cmp
				}
			}),
			_ => eprintln!(
				"Invalid sort field: {}. Valid options: id, title, status, priority, assignee",
				sort_field
			),
		}
	}

	let mut table = Table::new();
	table.set_header(vec![
		"ID", "Title", "Status", "Assignee", "Priority", "Tags",
	]);

	for task in filtered_tasks {
		table.add_row(vec![
			Cell::new(&task.id),
			Cell::new(&task.title),
			Cell::new(&task.status),
			Cell::new(&task.assignee),
			Cell::new(&task.priority),
			Cell::new(task.tags.unwrap_or_default().join(", ")),
		]);
	}

	println!("{table}");

	Ok(())
}

fn validate_tasks() -> Result<(), Box<dyn std::error::Error>> {
	// 1. Load the schema
	let schema_file = fs::File::open(".tasks/task.schema.json")?;
	let schema: Value = serde_json::from_reader(schema_file)?;
	let compiled_schema = JSONSchema::options()
		.with_draft(Draft::Draft7)
		.compile(&schema)
		.expect("A valid schema");

	// 2. Get a list of staged markdown files (including subdirectories)
	let output = Command::new("git")
		.args([
			"diff",
			"--cached",
			"--name-only",
			"--diff-filter=ACM",
			"--",
			".tasks/",
		])
		.output()?;

	let staged_files = String::from_utf8(output.stdout)?;
	let mut has_errors = false;

	// 3. Loop through each file and validate it
	for file_path in staged_files.lines() {
		if file_path.is_empty() {
			continue;
		}

		// Only validate .md files in .tasks/ (skip schema.json, Claude.md, etc.)
		if !file_path.ends_with(".md") || !file_path.starts_with(".tasks/") {
			continue;
		}

		// Skip non-task files
		let file_name = std::path::Path::new(file_path)
			.file_name()
			.and_then(|n| n.to_str())
			.unwrap_or("");
		if !file_name.contains('-') || file_name == "Claude.md" {
			continue;
		}

		let content = match fs::read_to_string(file_path) {
			Ok(c) => c,
			Err(_) => continue, // File might have been deleted
		};

		if content.starts_with("---") {
			if let Some(front_matter_str) = content.split("---").nth(1) {
				match serde_yaml::from_str::<Value>(front_matter_str) {
					Ok(yaml_value) => {
						if let Err(errors) = compiled_schema.validate(&yaml_value) {
							eprintln!("ERROR in {}:", file_path);
							for error in errors {
								eprintln!("   - {}", error);
							}
							has_errors = true;
						} else {
							println!("Validated: {}", file_path);
						}
					}
					Err(e) => {
						eprintln!("ERROR parsing YAML in {}:\n   {}", file_path, e);
						has_errors = true;
					}
				}
			}
		}
	}

	if has_errors {
		eprintln!("\nCommit aborted due to validation errors in task files.");
		process::exit(1);
	}

	Ok(())
}

fn export_tasks(output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
	let mut tasks = Vec::new();
	let mut categories = HashSet::new();

	for entry in glob(".tasks/**/*.md")? {
		let path = entry?;

		// Skip non-task files
		let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
		if !file_name.contains('-') || file_name == "Claude.md" {
			continue;
		}

		let content = fs::read_to_string(&path)?;

		if content.starts_with("---") {
			let parts: Vec<&str> = content.splitn(3, "---").collect();
			if parts.len() < 3 {
				continue;
			}

			let front_matter_str = parts[1];
			let body = parts[2].trim();

			match serde_yaml::from_str::<TaskFrontMatter>(front_matter_str) {
				Ok(front_matter) => {
					// Extract category from path (.tasks/core/... -> "core")
					let category = path
						.parent()
						.and_then(|p| p.file_name())
						.and_then(|n| n.to_str())
						.unwrap_or("uncategorized")
						.to_string();

					categories.insert(category.clone());

					// Extract description (first section after front matter)
					let description = extract_description(body);

					// Get relative file path
					let file_path = path.to_str().unwrap_or("").to_string();

					tasks.push(ExportableTask {
						id: front_matter.id,
						title: front_matter.title,
						status: front_matter.status,
						assignee: front_matter.assignee,
						priority: front_matter.priority,
						tags: front_matter.tags.unwrap_or_default(),
						whitepaper: front_matter.whitepaper,
						category,
						description,
						parent: front_matter.parent,
						file: file_path,
					});
				}
				Err(e) => eprintln!("Error parsing YAML in {:?}: {}", path, e),
			}
		}
	}

	// Sort categories alphabetically
	let mut categories_vec: Vec<String> = categories.into_iter().collect();
	categories_vec.sort();

	// Sort tasks by ID
	tasks.sort_by(|a, b| a.id.cmp(&b.id));

	let export = TaskExport {
		tasks,
		categories: categories_vec,
		generated_at: chrono::Utc::now().to_rfc3339(),
	};

	// Create output directory if it doesn't exist
	if let Some(parent) = Path::new(output_path).parent() {
		fs::create_dir_all(parent)?;
	}

	// Write JSON to file
	let json = serde_json::to_string_pretty(&export)?;
	fs::write(output_path, json)?;

	println!("Exported {} tasks to {}", export.tasks.len(), output_path);

	Ok(())
}

fn extract_description(body: &str) -> String {
	// Find the Description section and extract its content
	let lines: Vec<&str> = body.lines().collect();
	let mut description = String::new();
	let mut in_description = false;

	for line in lines {
		if line.starts_with("## Description") {
			in_description = true;
			continue;
		}
		if in_description {
			if line.starts_with("##") {
				// Hit next section
				break;
			}
			if !description.is_empty() || !line.trim().is_empty() {
				if !description.is_empty() {
					description.push(' ');
				}
				description.push_str(line.trim());
			}
		}
	}

	description
}
