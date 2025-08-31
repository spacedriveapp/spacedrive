// File: task-validator/src/main.rs
use clap::{Parser, Subcommand};
use comfy_table::{Cell, Table};
use glob::glob;
use jsonschema::{Draft, JSONSchema};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
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
    },
    /// Validate staged task files (for git hook)
    Validate,
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
    whitepaper: String,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::List { status, assignee, priority, tag } => {
            if let Err(e) = list_tasks(status, assignee, priority, tag) {
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
    }
}

fn list_tasks(
    status_filter: &Option<String>,
    assignee_filter: &Option<String>,
    priority_filter: &Option<String>,
    tag_filter: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tasks = Vec::new();

    for entry in glob(".tasks/*.md")? {
        let path = entry?;
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

    let filtered_tasks = tasks.into_iter().filter(|task| {
        let status_match = status_filter.as_ref().map_or(true, |s| task.status.to_lowercase() == s.to_lowercase());
        let assignee_match = assignee_filter.as_ref().map_or(true, |a| task.assignee.to_lowercase() == a.to_lowercase());
        let priority_match = priority_filter.as_ref().map_or(true, |p| task.priority.to_lowercase() == p.to_lowercase());
        let tag_match = tag_filter.as_ref().map_or(true, |t| {
            task.tags.as_ref().map_or(false, |tags| tags.iter().any(|tag| tag.to_lowercase() == t.to_lowercase()))
        });
        status_match && assignee_match && priority_match && tag_match
    }).collect::<Vec<_>>();

    let mut table = Table::new();
    table.set_header(vec!["ID", "Title", "Status", "Assignee", "Priority", "Tags"]);

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

    // 2. Get a list of staged markdown files
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM", "--", ".tasks/*.md"])
        .output()?;

    let staged_files = String::from_utf8(output.stdout)?;
    let mut has_errors = false;

    // 3. Loop through each file and validate it
    for file_path in staged_files.lines() {
        if file_path.is_empty() {
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
                            eprintln!("❌ ERROR in {}:", file_path);
                            for error in errors {
                                eprintln!("   - {}", error);
                            }
                            has_errors = true;
                        } else {
                            println!("✅ Validated: {}", file_path);
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ ERROR parsing YAML in {}:\n   {}", file_path, e);
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