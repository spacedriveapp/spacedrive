//! Tests for search functionality

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::search::input::*;

	#[test]
	fn test_file_search_input_validation() {
		// Test valid input
		let valid_input = FileSearchInput::simple("test query".to_string());
		assert!(valid_input.validate().is_ok());

		// Test empty query
		let empty_input = FileSearchInput::simple("".to_string());
		assert!(empty_input.validate().is_err());

		// Test query too long
		let long_query = "a".repeat(1001);
		let long_input = FileSearchInput::simple(long_query);
		assert!(long_input.validate().is_err());

		// Test invalid pagination
		let mut invalid_pagination = FileSearchInput::simple("test".to_string());
		invalid_pagination.pagination.limit = 0;
		assert!(invalid_pagination.validate().is_err());
	}

	#[test]
	fn test_search_mode_creation() {
		let fast_search = FileSearchInput::fast("test".to_string());
		assert!(matches!(fast_search.mode, SearchMode::Fast));
		assert_eq!(fast_search.pagination.limit, 20);

		let normal_search = FileSearchInput::simple("test".to_string());
		assert!(matches!(normal_search.mode, SearchMode::Normal));
		assert_eq!(normal_search.pagination.limit, 50);

		let comprehensive_search = FileSearchInput::comprehensive("test".to_string());
		assert!(matches!(comprehensive_search.mode, SearchMode::Full));
		assert_eq!(comprehensive_search.pagination.limit, 100);
	}

	#[test]
	fn test_search_filters() {
		let mut filters = SearchFilters::default();

		// Test file type filter
		filters.file_types = Some(vec!["txt".to_string(), "pdf".to_string()]);

		// Test date range filter
		filters.date_range = Some(DateRangeFilter {
			field: DateField::ModifiedAt,
			start: Some(chrono::Utc::now() - chrono::Duration::days(7)),
			end: Some(chrono::Utc::now()),
		});

		// Test size range filter
		filters.size_range = Some(SizeRangeFilter {
			min: Some(1024),
			max: Some(1024 * 1024),
		});

		assert!(filters.file_types.is_some());
		assert!(filters.date_range.is_some());
		assert!(filters.size_range.is_some());
	}

	#[test]
	fn test_content_type_extensions() {
		use crate::domain::ContentKind;
		use crate::filetype::FileTypeRegistry;

		let registry = FileTypeRegistry::new();

		let image_exts = registry.get_extensions_for_category(ContentKind::Image);
		assert!(image_exts.contains(&"jpg"));
		assert!(image_exts.contains(&"png"));

		let code_exts = registry.get_extensions_for_category(ContentKind::Code);
		assert!(code_exts.contains(&"rs"));
		assert!(code_exts.contains(&"js"));

		let database_exts = registry.get_extensions_for_category(ContentKind::Database);
		assert!(database_exts.contains(&"db"));
		assert!(database_exts.contains(&"sqlite"));

		// Test that we get more extensions than hardcoded approach
		assert!(image_exts.len() > 5); // Should have more than basic hardcoded list
		assert!(code_exts.len() > 10); // Should have comprehensive code extensions
	}

	#[test]
	fn test_fts5_query_building() {
		use crate::ops::search::query::FileSearchQuery;

		let search_input = FileSearchInput::simple("test query".to_string());
		let query = FileSearchQuery::new(search_input);

		let fts_query = query.build_fts5_query();
		assert!(fts_query.contains("test"));
		assert!(fts_query.contains("query"));

		// Test escaping
		let search_input_special = FileSearchInput::simple("test*query".to_string());
		let query_special = FileSearchQuery::new(search_input_special);
		let fts_query_special = query_special.build_fts5_query();
		assert!(fts_query_special.contains("test\\*query"));
	}

	#[test]
	fn test_highlight_extraction() {
		use crate::ops::search::query::FileSearchQuery;

		let search_input = FileSearchInput::simple("test".to_string());
		let query = FileSearchQuery::new(search_input);

		let highlights =
			query.extract_highlights("test", "test_file.txt", &Some("test".to_string()));

		assert_eq!(highlights.len(), 2); // Should match in both name and extension
		assert_eq!(highlights[0].field, "name");
		assert_eq!(highlights[0].start, 0);
		assert_eq!(highlights[0].end, 4);

		assert_eq!(highlights[1].field, "extension");
		assert_eq!(highlights[1].start, 0);
		assert_eq!(highlights[1].end, 4); // "test" extension
	}
}
