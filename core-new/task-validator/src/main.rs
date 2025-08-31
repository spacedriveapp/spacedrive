// File: task-validator/src/main.rs
use serde::Deserialize;
use serde_json::Value;
use jsonschema::{JSONSchema, Draft};
use std::fs;
use std::process::{self, Command};

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
    // 1. Load the schema from the file you created earlier
    let schema_file = fs::File::open(".tasks/task.schema.json")
        .expect("Could not open task.schema.json");
    let schema: Value = serde_json::from_reader(schema_file)
        .expect("Could not parse task schema JSON");
    let compiled_schema = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(&schema)
        .expect("A valid schema");

    // 2. Get a list of staged markdown files in the .tasks/ directory
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM", "--", ".tasks/*.md"])
        .output()
        .expect("Failed to execute git command");

    let staged_files = String::from_utf8(output.stdout)
        .expect("git diff output was not valid UTF-8");

    let mut has_errors = false;

    // 3. Loop through each file and validate it
    for file_path in staged_files.lines() {
        if file_path.is_empty() {
            continue;
        }

        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue, // File might have been deleted, skip
        };

        if content.starts_with("---") {
            // Extract the YAML front matter part of the file
            if let Some(front_matter_str) = content.split("---").nth(1) {
                // Parse the YAML into a serde_json::Value for validation
                match serde_yaml::from_str::<Value>(front_matter_str) {
                    Ok(yaml_value) => {
                        // Validate the value against the schema
                        if let Err(errors) = compiled_schema.validate(&yaml_value) {
                            eprintln!("❌ ERROR in {}:");
                            for error in errors {
                                eprintln!("   - {}");
                            }
                            has_errors = true;
                        } else {
                            println!("✅ Validated: {}");
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ ERROR parsing YAML in {}:\n   {}", file_path, e);
                        has_errors = true;
                    }
                }
            }
        } else {
            println!("⚠️  Skipped (no front matter): {}");
        }
    }

    // 4. Exit with a non-zero status code to block the commit if errors were found
    if has_errors {
        eprintln!("\nCommit aborted due to validation errors in task files.");
        process::exit(1);
    }

    process::exit(0);
}
