//! Ordering and pagination shared by every search backend.
//!
//! One deterministic global sort runs before final pagination, whichever
//! representation produced the candidates. The candidate type is the result
//! itself: nothing here reaches back into an arena or a store, which is what
//! lets the source-runtime plan's R6 slice route store-read candidates
//! through this same stage.

use std::cmp::Ordering;

use super::input::{PaginationOptions, SortDirection, SortField, SortOptions};
use super::output::FileSearchResult;

/// Order candidates by the requested field, deterministically.
///
/// Equal keys tiebreak on case-folded name and then on record id, so a result
/// set cannot reshuffle between keystrokes or pages. The direction applies to
/// the primary key only; tiebreaks always run ascending, since their job is
/// stability rather than meaning.
pub fn order(results: &mut [FileSearchResult], sort: &SortOptions) {
	results.sort_by(|a, b| {
		let primary = match sort.field {
			// The volume index holds no indexed-at signal, so IndexedAt
			// orders by score until one exists.
			SortField::Relevance | SortField::IndexedAt => {
				a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal)
			}
			SortField::Name => a.file.name.to_lowercase().cmp(&b.file.name.to_lowercase()),
			SortField::Size => a.file.size.cmp(&b.file.size),
			SortField::ModifiedAt => a.file.modified_at.cmp(&b.file.modified_at),
			SortField::CreatedAt => a.file.created_at.cmp(&b.file.created_at),
		};
		let primary = match sort.direction {
			SortDirection::Asc => primary,
			SortDirection::Desc => primary.reverse(),
		};
		primary
			.then_with(|| a.file.name.to_lowercase().cmp(&b.file.name.to_lowercase()))
			.then_with(|| a.file.id.cmp(&b.file.id))
	});
}

/// The candidate window one partition must contribute for a page to be
/// complete: everything up to the end of the requested page.
pub fn window(pagination: &PaginationOptions) -> usize {
	(pagination.offset as usize).saturating_add(pagination.limit as usize)
}

/// Keep one partition's best `window` candidates, ordered by the requested
/// sort. A partition may only be truncated at or above the requested window;
/// cutting deeper loses candidates that belong on the page after the global
/// sort.
pub fn narrow(results: &mut Vec<FileSearchResult>, sort: &SortOptions, window: usize) {
	if results.len() > window {
		order(results, sort);
		results.truncate(window);
	}
}

/// One deterministic global sort, then the requested page. The true match
/// count is the caller's to report: it was known before any narrowing.
pub fn page(
	mut results: Vec<FileSearchResult>,
	sort: &SortOptions,
	pagination: &PaginationOptions,
) -> Vec<FileSearchResult> {
	order(&mut results, sort);
	let offset = pagination.offset as usize;
	if offset >= results.len() {
		return Vec::new();
	}
	results.drain(..offset);
	results.truncate(pagination.limit as usize);
	results
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::domain::{File, SdPath};
	use crate::ops::indexing::state::EntryKind;
	use crate::ops::search::output::ScoreBreakdown;
	use chrono::{TimeZone, Utc};
	use uuid::Uuid;

	fn candidate(name: &str, size: u64, modified_secs: i64, score: f32) -> FileSearchResult {
		let metadata = crate::ops::indexing::metadata::EntryMetadata {
			path: std::path::PathBuf::from(format!("/vol/{name}")),
			kind: EntryKind::File,
			size,
			modified: Some(
				std::time::UNIX_EPOCH + std::time::Duration::from_secs(modified_secs as u64),
			),
			accessed: None,
			created: Some(std::time::UNIX_EPOCH),
			inode: None,
			permissions: None,
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: name.starts_with('.'),
		};
		let file = File::from_ephemeral(
			Uuid::now_v7(),
			&metadata,
			SdPath::Physical {
				device_slug: "test".to_string(),
				path: metadata.path.clone(),
			},
		);
		FileSearchResult {
			file,
			score,
			score_breakdown: ScoreBreakdown::new(score, None, 0.0, 0.0, 0.0),
			highlights: Vec::new(),
			matched_content: None,
		}
	}

	fn names(results: &[FileSearchResult]) -> Vec<String> {
		results.iter().map(|r| r.file.name.clone()).collect()
	}

	fn sort(field: SortField, direction: SortDirection) -> SortOptions {
		SortOptions { field, direction }
	}

	#[test]
	fn every_sort_field_orders_and_reverses() {
		let mut results = vec![
			candidate("beta.txt", 30, 300, 0.5),
			candidate("Alpha.txt", 10, 100, 0.9),
			candidate("gamma.txt", 20, 200, 0.7),
		];

		order(&mut results, &sort(SortField::Name, SortDirection::Asc));
		assert_eq!(names(&results), ["Alpha", "beta", "gamma"]);

		order(&mut results, &sort(SortField::Name, SortDirection::Desc));
		assert_eq!(names(&results), ["gamma", "beta", "Alpha"]);

		order(&mut results, &sort(SortField::Size, SortDirection::Asc));
		assert_eq!(names(&results), ["Alpha", "gamma", "beta"]);

		order(
			&mut results,
			&sort(SortField::ModifiedAt, SortDirection::Desc),
		);
		assert_eq!(names(&results), ["beta", "gamma", "Alpha"]);

		order(
			&mut results,
			&sort(SortField::Relevance, SortDirection::Desc),
		);
		assert_eq!(names(&results), ["Alpha", "gamma", "beta"]);
	}

	/// Equal primary keys settle on name and then id, so two runs over the
	/// same candidates produce the same order.
	#[test]
	fn equal_scores_tiebreak_deterministically() {
		let mut a = vec![
			candidate("same.txt", 1, 1, 0.5),
			candidate("other.txt", 1, 1, 0.5),
			candidate("first.txt", 1, 1, 0.5),
		];
		let mut b = a.clone();
		b.reverse();

		let by_relevance = sort(SortField::Relevance, SortDirection::Desc);
		order(&mut a, &by_relevance);
		order(&mut b, &by_relevance);
		assert_eq!(names(&a), names(&b));
		assert_eq!(names(&a), ["first", "other", "same"]);
	}

	#[test]
	fn a_page_is_a_window_over_the_sorted_whole() {
		let results: Vec<FileSearchResult> = (0..10)
			.map(|i| candidate(&format!("file-{i:02}.txt"), i, i as i64, 0.5))
			.collect();

		let by_name = sort(SortField::Name, SortDirection::Asc);
		let pagination = PaginationOptions {
			limit: 3,
			offset: 4,
		};
		assert_eq!(window(&pagination), 7);

		let page = page(results.clone(), &by_name, &pagination);
		assert_eq!(names(&page), ["file-04", "file-05", "file-06"]);

		let past_the_end = PaginationOptions {
			limit: 3,
			offset: 40,
		};
		assert!(super::page(results, &by_name, &past_the_end).is_empty());
	}

	/// A partition narrowed below the window would lose candidates that
	/// belong on the page; at or above it, nothing on the page can be lost.
	#[test]
	fn narrowing_preserves_everything_the_page_can_reach() {
		let full: Vec<FileSearchResult> = (0..50)
			.map(|i| candidate(&format!("file-{i:02}.txt"), i, i as i64, 0.5))
			.collect();
		let by_size = sort(SortField::Size, SortDirection::Desc);
		let pagination = PaginationOptions {
			limit: 5,
			offset: 5,
		};

		// Split candidates across two partitions, narrow each to the window,
		// merge, and page: the result must equal paging the unsplit whole.
		let expected = names(&page(full.clone(), &by_size, &pagination));

		let (mut left, mut right): (Vec<_>, Vec<_>) = {
			let mut left = Vec::new();
			let mut right = Vec::new();
			for (i, result) in full.into_iter().enumerate() {
				if i % 2 == 0 {
					left.push(result);
				} else {
					right.push(result);
				}
			}
			(left, right)
		};

		let w = window(&pagination);
		narrow(&mut left, &by_size, w);
		narrow(&mut right, &by_size, w);
		left.extend(right);

		assert_eq!(names(&page(left, &by_size, &pagination)), expected);
	}
}
