//! Integration tests for search functionality
//!
//! This test suite verifies search over the volume index.
//!
//! Tests cover:
//! - Name search over an indexed directory
//! - Filter application (file types, size, date, content types)
//! - Substring matching
//! - Result limits

mod helpers;

use helpers::*;
use sd_core::{
	domain::{addressing::SdPath, ContentKind},
	infra::{api::SessionContext, query::LibraryQuery},
	ops::{
		indexing::IndexScope,
		search::{
			input::{
				DateField, DateRangeFilter, FileSearchInput, PaginationOptions, SearchFilters,
				SearchMode, SearchScope, SizeRangeFilter, SortDirection, SortField, SortOptions,
			},
			query::FileSearchQuery,
		},
	},
};
use tokio::time::Duration;

// Helper function to execute search queries
async fn execute_search(
	harness: &IndexingHarness,
	input: FileSearchInput,
) -> anyhow::Result<sd_core::ops::search::output::FileSearchOutput> {
	let query = FileSearchQuery::new(input);
	let device_id = sd_core::device::get_current_device_id();
	let device_name = sd_core::device::get_current_device_slug();
	let mut session = SessionContext::device_session(device_id, device_name);
	session.current_library_id = Some(harness.library.id());

	let result = query.execute(harness.core.context.clone(), session).await?;
	Ok(result)
}

#[tokio::test]
async fn test_search_basic() -> anyhow::Result<()> {
	// Tests basic search in a browsed directory
	let harness = IndexingHarnessBuilder::new("search_basic")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("files");

	tokio::fs::create_dir_all(&search_dir).await?;
	tokio::fs::write(search_dir.join("document.txt"), "Important document").await?;
	tokio::fs::write(search_dir.join("notes.md"), "Meeting notes").await?;
	tokio::fs::write(search_dir.join("code.rs"), "fn main() {}").await?;

	// Browse the directory into the volume index
	harness
		.index_dir(&search_dir, IndexScope::Recursive)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search for "document"
	let search_input = FileSearchInput {
		query: "document".to_string(),
		scope: SearchScope::Path {
			path: SdPath::local(search_dir.clone()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters::default(),
		sort: SortOptions {
			field: SortField::Relevance,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: 50,
			offset: 0,
		},
	};

	let results = execute_search(&harness, search_input).await?;

	// Debug output
	eprintln!("Search results: {} found", results.total_found);
	eprintln!("Results count: {}", results.results.len());
	for (i, result) in results.results.iter().enumerate() {
		eprintln!(
			"  Result {}: {} (score: {})",
			i, result.file.name, result.score
		);
	}

	// Verify results
	assert!(!results.results.is_empty(), "Should find document.txt");
	let found_document = results.results.iter().any(|r| r.file.name == "document");
	assert!(found_document, "Should find document.txt by name");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_search_with_filters() -> anyhow::Result<()> {
	// Tests search with file type and size filters
	let harness = IndexingHarnessBuilder::new("search_filters")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("mixed");

	tokio::fs::create_dir_all(&search_dir).await?;

	// Create files with different types, sharing a name the query matches so
	// the filters are what narrow the results
	tokio::fs::write(search_dir.join("sample_notes.txt"), "Small text").await?;
	tokio::fs::write(search_dir.join("sample_readme.md"), "# Markdown").await?;
	tokio::fs::write(search_dir.join("sample_script.rs"), "fn main() {}").await?;
	tokio::fs::write(search_dir.join("sample_data.json"), r#"{"key": "value"}"#).await?;

	// Large file
	let large_content = "x".repeat(10000);
	tokio::fs::write(search_dir.join("sample_large.txt"), &large_content).await?;

	// Browse the directory into the volume index
	harness
		.index_dir(&search_dir, IndexScope::Recursive)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Test 1: Filter by file type (.txt files)
	let txt_search = FileSearchInput {
		query: "sample".to_string(), // Matches every fixture; the filter narrows
		scope: SearchScope::Path {
			path: SdPath::local(search_dir.clone()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters {
			file_types: Some(vec!["txt".to_string()]),
			..Default::default()
		},
		sort: SortOptions {
			field: SortField::Name,
			direction: SortDirection::Asc,
		},
		pagination: PaginationOptions {
			limit: 50,
			offset: 0,
		},
	};

	let txt_results = execute_search(&harness, txt_search).await?;

	let txt_count = txt_results
		.results
		.iter()
		.filter(|r| r.file.extension.as_deref() == Some("txt"))
		.count();
	assert_eq!(txt_count, 2, "Should find 2 .txt files");

	// Test 2: Filter by size range
	let size_search = FileSearchInput {
		query: "sample".to_string(), // Matches every fixture; the filter narrows
		scope: SearchScope::Path {
			path: SdPath::local(search_dir.clone()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters {
			size_range: Some(SizeRangeFilter {
				min: Some(5000),
				max: None,
			}),
			..Default::default()
		},
		sort: SortOptions {
			field: SortField::Size,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: 50,
			offset: 0,
		},
	};

	let size_results = execute_search(&harness, size_search).await?;
	assert!(
		size_results.results.len() >= 1,
		"Should find at least 1 large file"
	);

	// Test 3: Filter by content type (Code)
	let code_search = FileSearchInput {
		query: "sample".to_string(), // Matches every fixture; the filter narrows
		scope: SearchScope::Path {
			path: SdPath::local(search_dir.clone()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters {
			content_types: Some(vec![ContentKind::Code]),
			..Default::default()
		},
		sort: SortOptions {
			field: SortField::Name,
			direction: SortDirection::Asc,
		},
		pagination: PaginationOptions {
			limit: 50,
			offset: 0,
		},
	};

	let code_results = execute_search(&harness, code_search).await?;

	// Should find .rs files (identified as Code by FileTypeRegistry)
	let has_rust = code_results
		.results
		.iter()
		.any(|r| r.file.extension.as_deref() == Some("rs"));
	assert!(has_rust, "Should find Rust files via content type filter");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_search_date_filter() -> anyhow::Result<()> {
	// Tests search with date range filtering
	let harness = IndexingHarnessBuilder::new("search_dates")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("dated");

	tokio::fs::create_dir_all(&search_dir).await?;

	// Create some files (they'll all have recent timestamps)
	tokio::fs::write(search_dir.join("recent1.txt"), "Content 1").await?;
	tokio::fs::write(search_dir.join("recent2.txt"), "Content 2").await?;

	// Browse the directory into the volume index
	harness
		.index_dir(&search_dir, IndexScope::Recursive)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search with date filter (files from last hour)
	let one_hour_ago = chrono::Utc::now() - chrono::Duration::hours(1);

	let date_search = FileSearchInput {
		query: "recent".to_string(), // Matches both fixtures; the filter narrows
		scope: SearchScope::Path {
			path: SdPath::local(search_dir.clone()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters {
			date_range: Some(DateRangeFilter {
				field: DateField::ModifiedAt,
				start: Some(one_hour_ago),
				end: None,
			}),
			..Default::default()
		},
		sort: SortOptions {
			field: SortField::ModifiedAt,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: 50,
			offset: 0,
		},
	};

	let results = execute_search(&harness, date_search).await?;
	assert!(
		results.results.len() >= 2,
		"Should find recently created files"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_search_substring_matching() -> anyhow::Result<()> {
	// Tests search substring and prefix matching
	let harness = IndexingHarnessBuilder::new("search_substring")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("names");

	tokio::fs::create_dir_all(&search_dir).await?;

	// Create files with similar names
	tokio::fs::write(search_dir.join("test_file.txt"), "Content").await?;
	tokio::fs::write(search_dir.join("file_test.txt"), "Content").await?;
	tokio::fs::write(search_dir.join("testcase.txt"), "Content").await?;
	tokio::fs::write(search_dir.join("my_test_data.txt"), "Content").await?;

	// Browse the directory into the volume index
	harness
		.index_dir(&search_dir, IndexScope::Recursive)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search for "test" (should match all files with "test" in name)
	let search_input = FileSearchInput {
		query: "test".to_string(),
		scope: SearchScope::Path {
			path: SdPath::local(search_dir.clone()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters::default(),
		sort: SortOptions {
			field: SortField::Relevance,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: 50,
			offset: 0,
		},
	};

	let results = execute_search(&harness, search_input).await?;
	assert_eq!(
		results.results.len(),
		4,
		"Should find all 4 files with 'test' in name"
	);

	// Verify prefix matches score higher than substring matches
	let first_result = &results.results[0];
	assert!(
		first_result.file.name.starts_with("test"),
		"Prefix match should score highest"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_search_result_limit() -> anyhow::Result<()> {
	// Tests that search serves the requested window over the full match count
	let harness = IndexingHarnessBuilder::new("search_limit")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("many_files");

	tokio::fs::create_dir_all(&search_dir).await?;

	// More files than one page holds
	for i in 0..250 {
		tokio::fs::write(search_dir.join(format!("file_{:03}.txt", i)), "Content").await?;
	}

	// Browse the directory into the volume index
	harness
		.index_dir(&search_dir, IndexScope::Recursive)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Matches all 250 files and not the directory holding them
	let search_input = FileSearchInput {
		query: "file_".to_string(),
		scope: SearchScope::Path {
			path: SdPath::local(search_dir.clone()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters::default(),
		sort: SortOptions {
			field: SortField::Relevance,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: 100,
			offset: 0,
		},
	};

	let results = execute_search(&harness, search_input).await?;

	assert_eq!(results.results.len(), 100, "Should serve one page of 100");
	assert_eq!(results.total_found, 250, "Should count every match");

	harness.shutdown().await?;
	Ok(())
}
