//! Demonstration of action metadata for jobs
//!
//! This example shows how the universal action metadata system enhances
//! job progress events with rich contextual information.

use sd_core::{
    infra::action::context::{ActionContext, ActionContextProvider, sanitize_action_input},
    ops::indexing::{IndexMode, IndexPersistence, IndexScope, action::IndexingAction, IndexInput},
};
use serde_json::json;
use std::path::PathBuf;

fn main() {
    println!("=== Action Metadata System Demo ===\n");

    // Example 1: Manual indexing action
    demo_indexing_action();

    // Example 2: Location add action (conceptual - would trigger indexing)
    demo_location_add_context();

    // Example 3: Show enhanced progress event structure
    demo_enhanced_progress_event();
}

fn demo_indexing_action() {
    println!("1. INDEXING ACTION CONTEXT");
    println!("==========================");

    let indexing_action = IndexingAction::new(IndexInput {
        paths: vec![
            PathBuf::from("/Users/james/Documents"),
            PathBuf::from("/Users/james/Photos"),
        ],
        mode: IndexMode::Deep,
        scope: IndexScope::Recursive,
        persistence: IndexPersistence::Persistent,
        include_hidden: false,
    });

    let context = indexing_action.create_action_context();

    println!("Action Type: {}", context.action_type);
    println!("Initiated At: {}", context.initiated_at);
    println!("Action Input: {}", serde_json::to_string_pretty(&context.action_input).unwrap());
    println!("Context: {}", serde_json::to_string_pretty(&context.context).unwrap());
    println!();
}

fn demo_location_add_context() {
    println!("2. LOCATION ADD CONTEXT (Conceptual)");
    println!("====================================");

    // This shows what the context would look like for a location add action
    let location_context = ActionContext::new(
        "locations.add",
        json!({
            "path": "/Users/james/Documents",
            "name": "Documents",
            "mode": "deep"
        }),
        json!({
            "operation": "add_location",
            "trigger": "user_action",
            "path": "/Users/james/Documents",
            "name": "Documents",
            "mode": "deep"
        }),
    ).with_initiated_by("cli:james");

    println!("Action Type: {}", location_context.action_type);
    println!("Initiated By: {:?}", location_context.initiated_by);
    println!("Action Input: {}", serde_json::to_string_pretty(&location_context.action_input).unwrap());
    println!("Context: {}", serde_json::to_string_pretty(&location_context.context).unwrap());
    println!();
}

fn demo_enhanced_progress_event() {
    println!("3. ENHANCED PROGRESS EVENT");
    println!("==========================");

    // This shows what a job progress event would look like with action context
    let enhanced_event = json!({
        "Event": {
            "JobProgress": {
                "job_id": "f845b0ac-0886-4816-8e21-d9abba95aa0a",
                "job_type": "indexer",
                "progress": 0.99754,
                "message": "Finalizing Documents scan (3846/3877)",
                "generic_progress": {
                    "percentage": 0.99754,
                    "phase": "Finalizing",
                    "current_path": null,
                    "message": "Finalizing (3846/3877)",
                    "completion": {
                        "completed": 3846,
                        "total": 3877,
                        "bytes_completed": 7646481634_u64,
                        "total_bytes": 7646481634_u64
                    },
                    "performance": {
                        "rate": 0.0,
                        "estimated_remaining": null,
                        "elapsed": null,
                        "error_count": 0,
                        "warning_count": 0
                    },
                    "metadata": {
                        "action_context": {
                            "action_type": "locations.add",
                            "initiated_at": "2024-12-19T10:30:00Z",
                            "initiated_by": "ui:file_manager",
                            "action_input": {
                                "path": "/Users/james/Documents",
                                "name": "Documents",
                                "mode": "deep"
                            },
                            "context": {
                                "operation": "add_location",
                                "location_id": "550e8400-e29b-41d4-a716-446655440000",
                                "device_id": "dev-james-macbook",
                                "trigger": "drag_and_drop"
                            }
                        },
                        "phase": "Finalizing",
                        "scope": "Recursive",
                        "persistence": "Persistent",
                        "is_ephemeral": false,
                        "current_path": "Aggregating directory 3846/3877: Documents",
                        "processing_rate": 0.0,
                        "total_found": {
                            "bytes": 7646481634_u64,
                            "dirs": 3876,
                            "errors": 0,
                            "files": 18563,
                            "skipped": 0,
                            "symlinks": 1
                        }
                    }
                }
            }
        }
    });

    println!("Enhanced Job Progress Event:");
    println!("{}", serde_json::to_string_pretty(&enhanced_event).unwrap());
    println!();

    println!("Key Benefits:");
    println!("Know the action that triggered the job: 'locations.add'");
    println!("Understand the context: 'Adding Documents location via drag_and_drop'");
    println!("See original user input: path, name, mode");
    println!("Rich debugging info: location_id, device_id, trigger method");
    println!("Full audit trail: user action → job → progress → completion");
}

