//! Capture time and place, which only the image facet knows.
//!
//! The arena holds what a walk sees: names, sizes and filesystem times. When
//! a photo was taken and where sit in `facet_image`, written by the EXIF
//! pass after identification, so a listing or search served from an arena
//! reads them from the store before it sorts, filters or pages, the way a
//! tag filter resolves its paths from the stores first. A store-served row
//! carries its facet from the join in `sd_store::read`, so both backends
//! hand the same `File` to the one predicate and the one order below.
//!
//! A file with no facet has no capture time and no place: it fails a
//! capture-date range and `has_location: true`, and trails every dated file
//! in a capture order. A replica of another device's source has no store
//! here and reads the same way.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use super::input::{DateField, SearchFilters, SortField, SortOptions};
use super::output::FileSearchResult;
use crate::domain::{EntryKind, File, ImageMediaData};
use crate::ops::indexing::{SourceStore, VolumeIndex};

/// Whether the request sorts or filters by something only the facet holds,
/// which is what makes the decoration a step before the sort rather than a
/// finish on the page.
pub fn wanted(filters: &SearchFilters, sort: &SortOptions) -> bool {
	sort.field == SortField::CapturedAt || filters_capture(filters)
}

fn filters_capture(filters: &SearchFilters) -> bool {
	filters.has_location.is_some()
		|| filters
			.date_range
			.as_ref()
			.is_some_and(|range| range.field == DateField::CapturedAt)
}

/// The capture half of the filter set, judged from the `File` whichever
/// backend built it. The per-row predicates leave `CapturedAt` and
/// `has_location` to this one.
pub fn passes(file: &File, filters: &SearchFilters) -> bool {
	if let Some(wanted) = filters.has_location {
		if file.has_location() != wanted {
			return false;
		}
	}
	if let Some(range) = &filters.date_range {
		if range.field == DateField::CapturedAt {
			let Some(taken) = file.captured_at() else {
				return false;
			};
			if range.start.is_some_and(|start| taken < start) {
				return false;
			}
			if range.end.is_some_and(|end| taken > end) {
				return false;
			}
		}
	}
	true
}

/// Keep the results the capture filters admit.
pub fn retain_matching(results: &mut Vec<FileSearchResult>, filters: &SearchFilters) {
	if filters_capture(filters) {
		results.retain(|result| passes(&result.file, filters));
	}
}

/// Enrich arena-served results from their stores when the request needs
/// it, then apply the capture filters. Runs on a partition's full match set,
/// ahead of the page window, so the sort and the total see every row.
pub async fn apply_to_arena_results(
	cache: &VolumeIndex,
	results: &mut Vec<FileSearchResult>,
	filters: &SearchFilters,
	sort: &SortOptions,
) {
	if wanted(filters, sort) {
		decorate_results(cache, results).await;
	}
	retain_matching(results, filters);
}

pub async fn decorate_results(cache: &VolumeIndex, results: &mut [FileSearchResult]) {
	let mut files: Vec<File> = results.iter().map(|result| result.file.clone()).collect();
	decorate_files(cache, &mut files).await;
	for (result, file) in results.iter_mut().zip(files) {
		result.file.image_media_data = file.image_media_data;
	}
}

/// Give files listed from an arena the image facet their store rows carry.
/// A file that already has one, because its store built it, is left alone.
pub async fn decorate_files(cache: &VolumeIndex, files: &mut [File]) {
	let mut by_store: HashMap<Uuid, (Arc<SourceStore>, Vec<usize>)> = HashMap::new();
	for (position, file) in files.iter().enumerate() {
		if file.kind != EntryKind::File || file.image_media_data.is_some() {
			continue;
		}
		let Some(path) = file.sd_path.as_local_path() else {
			continue;
		};
		let Some(store) = cache.store_for(path).await else {
			continue;
		};
		by_store
			.entry(store.id())
			.or_insert_with(|| (store.clone(), Vec::new()))
			.1
			.push(position);
	}

	for (_, (store, positions)) in by_store {
		if let Err(error) = decorate_from_db(store.db(), files, &positions).await {
			tracing::warn!(source = %store.id(), %error, "image facets unavailable for listing");
		}
	}
}

/// Decorate the files at `positions` from one store, by record uuid.
pub async fn decorate_from_db(
	db: &sd_store::SourceDb,
	files: &mut [File],
	positions: &[usize],
) -> Result<(), sd_store::Error> {
	let ids: Vec<Uuid> = positions.iter().map(|&p| files[p].id).collect();
	let by_record: HashMap<Uuid, sd_store::ImageFacet> =
		sd_store::image_facets_for_records(db.pool(), &ids)
			.await?
			.into_iter()
			.collect();
	for &position in positions {
		let file = &mut files[position];
		if let Some(facet) = by_record.get(&file.id) {
			file.image_media_data = ImageMediaData::from_facet(file.id, facet);
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::domain::SdPath;
	use crate::ops::search::input::DateRangeFilter;
	use chrono::{DateTime, Utc};

	fn photo(taken: Option<&str>, place: Option<(f64, f64)>) -> File {
		let mut file = File::planned(SdPath::local("/vol/p.jpg"), 1, false);
		if taken.is_some() || place.is_some() {
			let facet = sd_store::ImageFacet {
				date_taken: taken.map(String::from),
				latitude: place.map(|(lat, _)| lat),
				longitude: place.map(|(_, lon)| lon),
				..Default::default()
			};
			file.image_media_data = ImageMediaData::from_facet(file.id, &facet);
		}
		file
	}

	fn at(rfc3339: &str) -> DateTime<Utc> {
		DateTime::parse_from_rfc3339(rfc3339)
			.unwrap()
			.with_timezone(&Utc)
	}

	#[test]
	fn a_file_without_a_facet_has_no_place_and_no_capture_time() {
		let placed = photo(Some("2024-03-12T10:00:00+00:00"), Some((35.68, 139.77)));
		let unplaced = photo(Some("2024-03-19T09:00:00+00:00"), None);
		let bare = photo(None, None);

		let placed_only = SearchFilters {
			has_location: Some(true),
			..Default::default()
		};
		assert!(passes(&placed, &placed_only));
		assert!(!passes(&unplaced, &placed_only));
		assert!(!passes(&bare, &placed_only));

		let unplaced_only = SearchFilters {
			has_location: Some(false),
			..Default::default()
		};
		assert!(!passes(&placed, &unplaced_only));
		assert!(passes(&unplaced, &unplaced_only));
		assert!(passes(&bare, &unplaced_only));
	}

	/// A capture range is inclusive at both ends and fails closed, like the
	/// filesystem date filters.
	#[test]
	fn a_capture_range_fails_closed_on_undated_files() {
		let march = SearchFilters {
			date_range: Some(DateRangeFilter {
				field: DateField::CapturedAt,
				start: Some(at("2024-03-12T10:00:00+00:00")),
				end: Some(at("2024-03-31T00:00:00+00:00")),
			}),
			..Default::default()
		};
		assert!(passes(
			&photo(Some("2024-03-12T10:00:00+00:00"), None),
			&march
		));
		assert!(passes(
			&photo(Some("2024-03-19T09:00:00+00:00"), None),
			&march
		));
		assert!(!passes(
			&photo(Some("2024-04-01T00:00:00+00:00"), None),
			&march
		));
		assert!(!passes(&photo(None, None), &march));

		// A range on another field is not this predicate's to judge.
		let modified = SearchFilters {
			date_range: Some(DateRangeFilter {
				field: DateField::ModifiedAt,
				start: Some(at("2030-01-01T00:00:00+00:00")),
				end: None,
			}),
			..Default::default()
		};
		assert!(passes(&photo(None, None), &modified));
	}

	/// Only a capture sort or filter pulls the facet ahead of the sort.
	#[test]
	fn the_facet_is_wanted_before_the_sort_only_when_the_request_reads_it() {
		let by_name = SortOptions {
			field: SortField::Name,
			direction: super::super::input::SortDirection::Asc,
		};
		assert!(!wanted(&SearchFilters::default(), &by_name));
		assert!(wanted(
			&SearchFilters {
				has_location: Some(false),
				..Default::default()
			},
			&by_name
		));
		assert!(wanted(
			&SearchFilters::default(),
			&SortOptions {
				field: SortField::CapturedAt,
				..by_name
			}
		));
	}
}
