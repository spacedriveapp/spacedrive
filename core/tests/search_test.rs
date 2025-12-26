//! Integration tests for search functionality
//!
//! This test suite verifies search operations across both persistent (indexed)
//! and ephemeral (in-memory) indexes, ensuring correct routing and results.
//!
//! Tests cover:
//! - Persistent search (FTS5 database with full metadata)
//! - Ephemeral search (in-memory NameRegistry with limited metadata)
//! - Filter application (file types, size, date, content types)
//! - Index type detection and routing
//! - Result accuracy and relevance scoring

mod helpers;

use helpers::*;
use sd_core::{
	domain::{addressing::SdPath, ContentKind},
	infra::{api::SessionContext, query::LibraryQuery},
	location::IndexMode,
	ops::{
		indexing::{IndexScope, IndexerJob, IndexerJobConfig},
		search::{
			input::{
				DateField, DateRangeFilter, FileSearchInput, PaginationOptions, SearchFilters,
				SearchMode, SearchScope, SizeRangeFilter, SortDirection, SortField, SortOptions,
			},
			query::FileSearchQuery,
			IndexType,
		},
	},
};
use std::path::PathBuf;
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

// Helper function to index a directory in ephemeral mode
async fn index_ephemeral(
	harness: &IndexingHarness,
	path: PathBuf,
	scope: IndexScope,
) -> anyhow::Result<()> {
	let sd_path = SdPath::local(path.clone());
	let global_index = harness.core.context.ephemeral_cache().get_global_index();

	let indexer_config = IndexerJobConfig::ephemeral_browse(sd_path, scope);
	let mut indexer_job = IndexerJob::new(indexer_config);
	indexer_job.set_ephemeral_index(global_index);

	let index_handle = harness.library.jobs().dispatch(indexer_job).await?;
	index_handle.wait().await?;

	harness
		.core
		.context
		.ephemeral_cache()
		.mark_indexing_complete(&path);

	Ok(())
}

// ============================================================================
// PERSISTENT SEARCH TESTS (Indexed Locations)
// ============================================================================

#[tokio::test]
async fn test_persistent_search_basic() -> anyhow::Result<()> {
	// Tests basic search in a persistent indexed location
	let harness = IndexingHarnessBuilder::new("persistent_search_basic")
		.disable_watcher()
		.build()
		.await?;

	let test_location = harness.create_test_location("test_search").await?;

	// Create diverse file structure
	test_location.create_dir("documents").await?;
	test_location.create_dir("images").await?;
	test_location.create_dir("code").await?;

	test_location
		.write_file("documents/report.txt", "Annual report content")
		.await?;
	test_location
		.write_file("documents/notes.md", "Meeting notes")
		.await?;
	test_location
		.write_file("images/photo.jpg", "fake jpg data")
		.await?;
	test_location
		.write_file("images/screenshot.png", "fake png data")
		.await?;
	test_location
		.write_file("code/main.rs", "fn main() {}")
		.await?;
	test_location
		.write_file("code/lib.rs", "pub fn test() {}")
		.await?;

	// Index the location
	let location = test_location
		.index("Test Location", IndexMode::Shallow)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search for "report"
	let search_input = FileSearchInput {
		query: "report".to_string(),
		scope: SearchScope::Location {
			location_id: location.uuid,
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

	// Verify it used persistent index
	assert_eq!(
		results.index_type,
		IndexType::Persistent,
		"Should use persistent index for indexed location"
	);

	// Verify results
	assert!(!results.results.is_empty(), "Should find report.txt");
	let found_report = results.results.iter().any(|r| r.file.name == "report");
	assert!(found_report, "Should find report.txt by name");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_persistent_search_with_filters() -> anyhow::Result<()> {
	// Tests search with various filters in persistent index
	let harness = IndexingHarnessBuilder::new("persistent_search_filters")
		.disable_watcher()
		.build()
		.await?;

	let test_location = harness.create_test_location("test_search").await?;

	// Create files with different types and sizes
	test_location.create_dir("mixed").await?;

	// Small text files
	test_location
		.write_file("mixed/small.txt", "Small content")
		.await?;
	test_location
		.write_file("mixed/readme.md", "# Readme")
		.await?;

	// Larger files (simulate with repeated content)
	let large_content = "x".repeat(10000);
	test_location
		.write_file("mixed/large.txt", &large_content)
		.await?;

	// Different file types
	test_location
		.write_file("mixed/script.rs", "fn main() {}")
		.await?;
	test_location
		.write_file("mixed/data.json", r#"{"key": "value"}"#)
		.await?;

	let location = test_location
		.index("Test Location", IndexMode::Shallow)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Test 1: Filter by file type (text files)
	let text_search = FileSearchInput {
		query: "a".to_string(), // Broad query to test filtering
		scope: SearchScope::Location {
			location_id: location.uuid,
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

	let text_results = execute_search(&harness, text_search).await?;
	assert_eq!(text_results.index_type, IndexType::Persistent);

	let txt_count = text_results
		.results
		.iter()
		.filter(|r| r.file.extension.as_deref() == Some("txt"))
		.count();
	assert!(txt_count >= 2, "Should find at least 2 .txt files");

	// Test 2: Filter by size range (files > 5000 bytes)
	let size_search = FileSearchInput {
		query: "a".to_string(), // Broad query to test filtering
		scope: SearchScope::Location {
			location_id: location.uuid,
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
	assert_eq!(size_results.index_type, IndexType::Persistent);

	let large_files = size_results
		.results
		.iter()
		.filter(|r| r.file.size >= 5000)
		.count();
	assert!(large_files >= 1, "Should find at least 1 large file");

	// Test 3: Filter by content type (Code)
	let code_search = FileSearchInput {
		query: "a".to_string(), // Broad query to test filtering
		scope: SearchScope::Location {
			location_id: location.uuid,
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
	assert_eq!(code_results.index_type, IndexType::Persistent);

	// Should find .rs files (Code content type)
	let has_rust = code_results
		.results
		.iter()
		.any(|r| r.file.extension.as_deref() == Some("rs"));
	assert!(has_rust, "Should find Rust files with Code content type");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_persistent_search_by_path() -> anyhow::Result<()> {
	// Tests search scoped to a specific directory path
	let harness = IndexingHarnessBuilder::new("persistent_search_path")
		.disable_watcher()
		.build()
		.await?;

	let test_location = harness.create_test_location("test_search").await?;

	// Create nested structure
	test_location.create_dir("folder_a").await?;
	test_location.create_dir("folder_b").await?;

	test_location
		.write_file("folder_a/test.txt", "Content A")
		.await?;
	test_location
		.write_file("folder_b/test.txt", "Content B")
		.await?;
	test_location.write_file("root.txt", "Root content").await?;

	let location = test_location
		.index("Test Location", IndexMode::Shallow)
		.await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search within folder_a only
	let folder_a_path = test_location.path().join("folder_a");
	let device_slug = sd_core::device::get_current_device_slug();
	let folder_a_sd = SdPath::Physical {
		device_slug,
		path: folder_a_path.to_path_buf(),
	};

	let path_search = FileSearchInput {
		query: "test".to_string(),
		scope: SearchScope::Path { path: folder_a_sd },
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

	let results = execute_search(&harness, path_search).await?;
	assert_eq!(results.index_type, IndexType::Persistent);

	// Should only find test.txt from folder_a, not folder_b
	assert_eq!(
		results.results.len(),
		1,
		"Should only find file in folder_a"
	);
	assert!(results.results[0].file.name == "test");

	harness.shutdown().await?;
	Ok(())
}

// ============================================================================
// EPHEMERAL SEARCH TESTS (Non-Indexed Directories)
// ============================================================================

#[tokio::test]
async fn test_ephemeral_search_basic() -> anyhow::Result<()> {
	// Tests basic search in ephemeral (non-indexed) directory
	let harness = IndexingHarnessBuilder::new("ephemeral_search_basic")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("ephemeral_files");

	tokio::fs::create_dir_all(&search_dir).await?;
	tokio::fs::write(search_dir.join("document.txt"), "Important document").await?;
	tokio::fs::write(search_dir.join("notes.md"), "Meeting notes").await?;
	tokio::fs::write(search_dir.join("code.rs"), "fn main() {}").await?;

	// Index in ephemeral mode
	index_ephemeral(&harness, search_dir.clone(), IndexScope::Recursive).await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Debug: Check if files exist in ephemeral cache
	if let Some(index_arc) = harness
		.core
		.context
		.ephemeral_cache()
		.get_for_path(&search_dir)
	{
		let index = index_arc.read().await;
		let all_paths = index.list_directory(&search_dir).unwrap_or_default();
		eprintln!(
			"Ephemeral cache has {} entries for {:?}",
			all_paths.len(),
			search_dir
		);
		for path in all_paths.iter().take(10) {
			eprintln!("  - {:?}", path);
		}
	} else {
		eprintln!("No ephemeral cache found for {:?}", search_dir);
	}

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
	eprintln!(
		"Search results: {} found, index type: {:?}",
		results.total_found, results.index_type
	);
	eprintln!("Results count: {}", results.results.len());
	for (i, result) in results.results.iter().enumerate() {
		eprintln!(
			"  Result {}: {} (score: {})",
			i, result.file.name, result.score
		);
	}

	// Verify it used ephemeral index
	assert_eq!(
		results.index_type,
		IndexType::Ephemeral,
		"Should use ephemeral index for non-indexed directory"
	);

	// Verify results
	assert!(!results.results.is_empty(), "Should find document.txt");
	let found_document = results.results.iter().any(|r| r.file.name == "document");
	assert!(found_document, "Should find document.txt by name");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_ephemeral_search_with_filters() -> anyhow::Result<()> {
	// Tests ephemeral search with file type and size filters
	let harness = IndexingHarnessBuilder::new("ephemeral_search_filters")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("ephemeral_mixed");

	tokio::fs::create_dir_all(&search_dir).await?;

	// Create files with different types
	tokio::fs::write(search_dir.join("file1.txt"), "Small text").await?;
	tokio::fs::write(search_dir.join("file2.md"), "# Markdown").await?;
	tokio::fs::write(search_dir.join("script.rs"), "fn main() {}").await?;
	tokio::fs::write(search_dir.join("data.json"), r#"{"key": "value"}"#).await?;

	// Large file
	let large_content = "x".repeat(10000);
	tokio::fs::write(search_dir.join("large.txt"), &large_content).await?;

	// Ephemeral index
	index_ephemeral(&harness, search_dir.clone(), IndexScope::Recursive).await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Test 1: Filter by file type (.txt files)
	let txt_search = FileSearchInput {
		query: "a".to_string(), // Broad query to test filtering
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
	assert_eq!(txt_results.index_type, IndexType::Ephemeral);

	let txt_count = txt_results
		.results
		.iter()
		.filter(|r| r.file.extension.as_deref() == Some("txt"))
		.count();
	assert_eq!(txt_count, 2, "Should find 2 .txt files");

	// Test 2: Filter by size range
	let size_search = FileSearchInput {
		query: "a".to_string(), // Broad query to test filtering
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
	assert_eq!(size_results.index_type, IndexType::Ephemeral);
	assert!(
		size_results.results.len() >= 1,
		"Should find at least 1 large file"
	);

	// Test 3: Filter by content type (Code)
	let code_search = FileSearchInput {
		query: "a".to_string(), // Broad query to test filtering
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
	assert_eq!(code_results.index_type, IndexType::Ephemeral);

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
async fn test_ephemeral_search_date_filter() -> anyhow::Result<()> {
	// Tests ephemeral search with date range filtering
	let harness = IndexingHarnessBuilder::new("ephemeral_search_dates")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("ephemeral_dated");

	tokio::fs::create_dir_all(&search_dir).await?;

	// Create some files (they'll all have recent timestamps)
	tokio::fs::write(search_dir.join("recent1.txt"), "Content 1").await?;
	tokio::fs::write(search_dir.join("recent2.txt"), "Content 2").await?;

	// Ephemeral index
	index_ephemeral(&harness, search_dir.clone(), IndexScope::Recursive).await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search with date filter (files from last hour)
	let one_hour_ago = chrono::Utc::now() - chrono::Duration::hours(1);

	let date_search = FileSearchInput {
		query: "a".to_string(), // Broad query to test filtering
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
	assert_eq!(results.index_type, IndexType::Ephemeral);
	assert!(
		results.results.len() >= 2,
		"Should find recently created files"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_ephemeral_search_substring_matching() -> anyhow::Result<()> {
	// Tests ephemeral search substring and prefix matching
	let harness = IndexingHarnessBuilder::new("ephemeral_search_substring")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("ephemeral_names");

	tokio::fs::create_dir_all(&search_dir).await?;

	// Create files with similar names
	tokio::fs::write(search_dir.join("test_file.txt"), "Content").await?;
	tokio::fs::write(search_dir.join("file_test.txt"), "Content").await?;
	tokio::fs::write(search_dir.join("testcase.txt"), "Content").await?;
	tokio::fs::write(search_dir.join("my_test_data.txt"), "Content").await?;

	// Ephemeral index
	index_ephemeral(&harness, search_dir.clone(), IndexScope::Recursive).await?;

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
	assert_eq!(results.index_type, IndexType::Ephemeral);
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

// ============================================================================
// INDEX ROUTING TESTS
// ============================================================================

#[tokio::test]
async fn test_search_routing_indexed_vs_ephemeral() -> anyhow::Result<()> {
	// Tests that search correctly routes between persistent and ephemeral indexes
	let harness = IndexingHarnessBuilder::new("search_routing")
		.disable_watcher()
		.build()
		.await?;

	// Create an indexed location
	let indexed_location = harness.create_test_location("indexed").await?;
	indexed_location
		.write_file("indexed_file.txt", "Content")
		.await?;
	let location = indexed_location
		.index("Indexed Location", IndexMode::Shallow)
		.await?;

	// Create an ephemeral directory
	let test_root = harness.temp_path();
	let ephemeral_dir = test_root.join("ephemeral");
	tokio::fs::create_dir_all(&ephemeral_dir).await?;
	tokio::fs::write(ephemeral_dir.join("ephemeral_file.txt"), "Content").await?;

	// Index ephemeral directory
	index_ephemeral(&harness, ephemeral_dir.clone(), IndexScope::Recursive).await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search in indexed location - should use Persistent
	let indexed_search = FileSearchInput {
		query: "indexed".to_string(),
		scope: SearchScope::Location {
			location_id: location.uuid,
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

	let indexed_results = execute_search(&harness, indexed_search).await?;
	assert_eq!(
		indexed_results.index_type,
		IndexType::Persistent,
		"Indexed location should use Persistent index"
	);

	// Search in ephemeral directory - should use Ephemeral
	let ephemeral_search = FileSearchInput {
		query: "ephemeral".to_string(),
		scope: SearchScope::Path {
			path: SdPath::local(ephemeral_dir.clone()),
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

	let ephemeral_results = execute_search(&harness, ephemeral_search).await?;
	assert_eq!(
		ephemeral_results.index_type,
		IndexType::Ephemeral,
		"Ephemeral directory should use Ephemeral index"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_ephemeral_search_result_limit() -> anyhow::Result<()> {
	// Tests that ephemeral search respects the 200 result limit
	let harness = IndexingHarnessBuilder::new("ephemeral_search_limit")
		.disable_watcher()
		.build()
		.await?;

	let test_root = harness.temp_path();
	let search_dir = test_root.join("many_files");

	tokio::fs::create_dir_all(&search_dir).await?;

	// Create 250 files (exceeds ephemeral limit of 200)
	for i in 0..250 {
		tokio::fs::write(search_dir.join(format!("file_{:03}.txt", i)), "Content").await?;
	}

	// Ephemeral index
	index_ephemeral(&harness, search_dir.clone(), IndexScope::Recursive).await?;

	tokio::time::sleep(Duration::from_millis(500)).await;

	// Search for "file" (should match all 250 files)
	let search_input = FileSearchInput {
		query: "file".to_string(),
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
			limit: 300, // Request more than ephemeral limit
			offset: 0,
		},
	};

	let results = execute_search(&harness, search_input).await?;
	assert_eq!(results.index_type, IndexType::Ephemeral);

	// Should be capped at 200 (ephemeral limit)
	assert!(
		results.results.len() <= 200,
		"Ephemeral search should cap at 200 results, got {}",
		results.results.len()
	);

	harness.shutdown().await?;
	Ok(())
}
