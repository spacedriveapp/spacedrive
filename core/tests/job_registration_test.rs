//! Tests for job registration system

use sd_core::{
	domain::addressing::SdPath,
	infra::job::{prelude::*, registry::REGISTRY},
	ops::files::copy::job::FileCopyJob,
};

#[tokio::test]
async fn test_job_registration() {
	// Test that FileCopyJob is registered
	let job_names = REGISTRY.job_names();
	assert!(
		job_names.contains(&"file_copy"),
		"FileCopyJob should be registered"
	);

	// Test getting schema
	let schema = REGISTRY.get_schema("file_copy");
	assert!(schema.is_some(), "Should be able to get FileCopyJob schema");

	let schema = schema.unwrap();
	assert_eq!(schema.name, "file_copy");
	assert_eq!(schema.resumable, true);
	assert_eq!(
		schema.description,
		Some("Copy or move files to a destination")
	);
}

#[tokio::test]
async fn test_job_creation_from_json() {
	// Create a FileCopyJob using the registry
	let sources = vec![SdPath::new("test-device".to_string(), "/test/source")];
	let destination = SdPath::new("test-device".to_string(), "/test/dest");
	let job = FileCopyJob::from_paths(sources, destination);

	// Serialize to JSON
	let json_data = serde_json::to_value(&job).expect("Should serialize to JSON");

	// Create job from registry
	let created_job = REGISTRY
		.create_job("file_copy", json_data)
		.expect("Should create job from JSON");

	// Verify it's the right type by serializing state
	let state = created_job
		.serialize_state()
		.expect("Should serialize state");
	assert!(!state.is_empty(), "State should not be empty");
}

#[tokio::test]
async fn test_job_deserialization() {
	// Create a FileCopyJob
	let sources = vec![SdPath::new("test-device".to_string(), "/test/source")];
	let destination = SdPath::new("test-device".to_string(), "/test/dest");
	let job = FileCopyJob::from_paths(sources, destination);

	// Serialize as MessagePack (how jobs are stored)
	let state = rmp_serde::to_vec(&job).expect("Should serialize as MessagePack");

	// Deserialize using registry
	let deserialized_job = REGISTRY
		.deserialize_job("file_copy", &state)
		.expect("Should deserialize from MessagePack");

	// Verify by re-serializing
	let new_state = deserialized_job
		.serialize_state()
		.expect("Should re-serialize");
	assert_eq!(state, new_state, "States should match after round-trip");
}

#[tokio::test]
async fn test_unregistered_job_error() {
	// Try to create a job that doesn't exist
	let result = REGISTRY.create_job("nonexistent_job", serde_json::json!({}));
	assert!(result.is_err(), "Should fail for unregistered job");

	let error = result.unwrap_err();
	if let JobError::NotFound(msg) = error {
		assert!(
			msg.contains("nonexistent_job"),
			"Error should mention the missing job type"
		);
	} else {
		panic!("Expected NotFound error, got: {:?}", error);
	}
}

#[tokio::test]
async fn test_job_schema_information() {
	let schema = REGISTRY
		.get_schema("file_copy")
		.expect("Should have schema");

	// Verify schema properties
	assert_eq!(schema.name, "file_copy");
	assert!(schema.resumable, "FileCopyJob should be resumable");
	assert_eq!(schema.version, 1, "Should have version 1");
	assert!(schema.description.is_some(), "Should have description");
}

#[test]
fn test_has_job() {
	assert!(REGISTRY.has_job("file_copy"), "Should have file_copy job");
	assert!(
		!REGISTRY.has_job("nonexistent"),
		"Should not have nonexistent job"
	);
}
