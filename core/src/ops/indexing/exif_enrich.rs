//! Reading EXIF into the image facet.
//!
//! Identification opens every file once and says what kind of bytes it
//! found. For an image that is enough to know a header read is worth it:
//! capture time, place, camera and geometry, which `records.exif`, the
//! rename resolver and the inspector all want answered from the store rather
//! than from the file. This pass runs behind identification, claims image
//! records whose facet row is missing or names other bytes, parses each on
//! the blocking pool, and writes one row per content hash.
//!
//! The work queue is the store (`sd_store::image`), so the pass resumes for
//! free, never reads a file twice for the same bytes, and picks up a changed
//! file as soon as identification has given it a new hash. A source added
//! without content identification never reaches this pass, since nothing
//! names its images.

use crate::infra::job::{generic_progress::GenericProgress, prelude::*, types::JobPriority};
use futures::StreamExt;
use sd_media_metadata::exif::{ExifMetadata, MediaDate, Orientation};
use sd_store::{ImageFacet, PendingImage};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

/// Records claimed from the store per pass. A header read is quick, so the
/// claim is sized to make the queue round trip disappear.
const BATCH_SIZE: usize = 256;

/// Files parsed at once. The same bound the thumbnail job submits under: a
/// header read is a seek and a few kilobytes, and the drive sets the limit.
const CONCURRENCY: usize = 32;

/// Reads EXIF for the image records of a source that have no current
/// facet row.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ExifEnrichJob {
	/// The source's root, which is how its store is found.
	root: PathBuf,
	/// Dispatched behind a background identification pass rather than a
	/// user action, and silent on the event bus like it.
	#[serde(default)]
	background: bool,
}

impl ExifEnrichJob {
	pub fn new(root: PathBuf, background: bool) -> Self {
		Self { root, background }
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExifEnrichOutput {
	/// Content hashes a facet row was written for, EXIF or not.
	pub enriched: u64,
	/// Of those, how many carried EXIF.
	pub with_exif: u64,
	/// Files that could not be opened. Left pending for the next pass.
	pub unreadable: u64,
}

impl From<ExifEnrichOutput> for JobOutput {
	fn from(output: ExifEnrichOutput) -> Self {
		JobOutput::custom(output)
	}
}

impl Job for ExifEnrichJob {
	const NAME: &'static str = "exif_enrich";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Read photo metadata");
}

impl crate::infra::job::traits::DynJob for ExifEnrichJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	/// One pass per source at a time; the work queue is shared.
	fn dedup_key(&self) -> Option<String> {
		Some(self.root.display().to_string())
	}

	fn should_emit_events(&self) -> bool {
		!self.background
	}
}

#[async_trait::async_trait]
impl JobHandler for ExifEnrichJob {
	type Output = ExifEnrichOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let volume_index = ctx.library().core_context().volume_index();
		if let Some(reason) = volume_index.dispatch_refusal(&self.root) {
			return Err(JobError::execution(reason));
		}
		let Some(store) = volume_index.store_for(&self.root).await else {
			ctx.log(format!(
				"No source store at {}; nothing to enrich",
				self.root.display()
			));
			return Ok(ExifEnrichOutput {
				enriched: 0,
				with_exif: 0,
				unreadable: 0,
			});
		};

		// Identities are queued behind the same writer; claim what landed.
		store.flush().await.map_err(|e| e.to_string())?;

		let outstanding = store
			.files_needing_image_facets_count()
			.await
			.map_err(|e| e.to_string())?;
		ctx.log(format!("{outstanding} images to read metadata for"));

		let label = super::content_identity::source_label(&self.root);
		if outstanding > 0 {
			ctx.progress(Progress::generic(GenericProgress::new(
				0.0,
				"Reading photo metadata",
				format!("{label} — 0 of {outstanding} images"),
			)));
		}

		let mut enriched = 0u64;
		let mut with_exif = 0u64;
		let mut unreadable = 0u64;
		let mut done = 0u64;
		let mut cursor = 0i64;

		loop {
			ctx.check_interrupt().await?;

			let batch = store
				.files_needing_image_facets(cursor, BATCH_SIZE)
				.await
				.map_err(|e| e.to_string())?;
			let Some(last) = batch.last() else {
				break;
			};
			cursor = last.0.rowid;
			done += batch.len() as u64;

			let (facets, failed) = read_batch(batch).await;
			unreadable += failed;
			enriched += facets.len() as u64;
			with_exif += facets
				.iter()
				.filter(|(_, facet)| *facet != ImageFacet::default())
				.count() as u64;
			store.image_facets(facets).await;

			ctx.progress(Progress::generic(GenericProgress::new(
				if outstanding > 0 {
					(done as f32 / outstanding as f32).min(1.0)
				} else {
					1.0
				},
				"Reading photo metadata",
				format!("{label} — {done} of {outstanding} images"),
			)));

			// The next claim reads the database; a claim that outraces the
			// writer would hand back rows this pass already wrote.
			store.flush().await.map_err(|e| e.to_string())?;
		}

		store.flush().await.map_err(|e| e.to_string())?;
		ctx.log(format!(
			"Read metadata for {enriched} distinct images, {with_exif} with EXIF, {unreadable} unreadable"
		));

		Ok(ExifEnrichOutput {
			enriched,
			with_exif,
			unreadable,
		})
	}
}

/// Parse one file per content hash in the batch, several at a time.
///
/// A file that cannot be opened is skipped and counted: the record keeps no
/// facet row, so the next pass claims it again, which is right for a drive
/// that hiccuped and harmless for a file the next walk removes. A file with
/// no EXIF, or EXIF the reader cannot make sense of, gets an empty row so
/// the bytes are never read again.
async fn read_batch(batch: Vec<(PendingImage, PathBuf)>) -> (Vec<(String, ImageFacet)>, u64) {
	let mut by_hash: HashMap<String, PathBuf> = HashMap::new();
	for (image, path) in batch {
		by_hash.entry(image.content_hash).or_insert(path);
	}

	let results: Vec<_> = futures::stream::iter(by_hash)
		.map(|(hash, path)| async move {
			match ExifMetadata::from_path(&path).await {
				Ok(Some(exif)) => Some((hash, facet_from_exif(&exif))),
				Ok(None) => Some((hash, ImageFacet::default())),
				Err(sd_media_metadata::Error::FileIO(error)) => {
					tracing::debug!(path = %path.display(), %error, "could not open for EXIF");
					None
				}
				Err(error) => {
					tracing::debug!(path = %path.display(), %error, "EXIF unreadable");
					Some((hash, ImageFacet::default()))
				}
			}
		})
		.buffer_unordered(CONCURRENCY)
		.collect()
		.await;

	let mut facets = Vec::with_capacity(results.len());
	let mut unreadable = 0;
	for result in results {
		match result {
			Some(facet) => facets.push(facet),
			None => unreadable += 1,
		}
	}
	(facets, unreadable)
}

/// The facet columns an EXIF read fills.
///
/// Dates are stored as RFC 3339; a naive capture time is taken as UTC, which
/// is what `records.exif` already assumed, so the row and the on-demand
/// parse agree. Geometry of zero means the tag was absent. The orientation
/// goes back to its EXIF number so a reader needs no enum of ours.
pub fn facet_from_exif(exif: &ExifMetadata) -> ImageFacet {
	let (latitude, longitude) = exif
		.location
		.as_ref()
		.map(|location| {
			let (lat, lon) = location.coordinates();
			(Some(lat), Some(lon))
		})
		.unwrap_or((None, None));
	let camera = &exif.camera_data;
	ImageFacet {
		width: (exif.resolution.width > 0).then_some(i64::from(exif.resolution.width)),
		height: (exif.resolution.height > 0).then_some(i64::from(exif.resolution.height)),
		date_taken: exif.date_taken.as_ref().map(|date| match date {
			MediaDate::Utc(t) => t.to_rfc3339(),
			MediaDate::Naive(t) => t.and_utc().to_rfc3339(),
		}),
		latitude,
		longitude,
		camera_make: camera.device_make.clone(),
		camera_model: camera.device_model.clone(),
		lens_model: camera.lens_model.clone(),
		focal_length: camera.focal_length.map(|f| f.to_string()),
		aperture: None,
		shutter_speed: camera.shutter_speed.map(|s| s.to_string()),
		iso: camera.iso.map(i64::from),
		orientation: Some(match camera.orientation {
			Orientation::Normal => 1,
			Orientation::MirroredHorizontal => 2,
			Orientation::CW180 => 3,
			Orientation::MirroredVertical => 4,
			Orientation::MirroredHorizontalAnd270CW => 5,
			Orientation::CW90 => 6,
			Orientation::MirroredHorizontalAnd90CW => 7,
			Orientation::CW270 => 8,
		}),
		color_space: camera.color_space.clone(),
		color_profile: camera
			.color_profile
			.as_ref()
			.map(|profile| format!("{profile:?}")),
		bit_depth: camera.bit_depth.map(|depth| depth.to_string()),
		artist: exif.artist.clone(),
		copyright: exif.copyright.clone(),
		description: exif.description.clone(),
	}
}

/// Queue a metadata pass behind an identification pass that just landed.
///
/// Called from the end of every identification job, so the pass follows
/// whichever path dispatched that one: a track, a watcher nudge, a returned
/// volume, or the launch sweep. The launch sweep is what backfills a store
/// identified before this pass existed. A source whose settings say not to
/// identify content gets nothing, and so does one with nothing pending,
/// which is most of them most of the time.
pub async fn enrich_after_identification(
	library: &Arc<crate::library::Library>,
	root: &Path,
	background: bool,
) {
	let volume_index = library.core_context().volume_index();
	if !volume_index
		.source_config_at(root)
		.is_some_and(|config| config.identify_content)
	{
		return;
	}
	let Some(store) = volume_index.store_for(root).await else {
		return;
	};
	match store.files_needing_image_facets_count().await {
		Ok(0) => return,
		Ok(_) => {}
		Err(error) => {
			tracing::warn!(root = %root.display(), %error, "could not count images needing metadata");
			return;
		}
	}
	if let Err(error) = library
		.jobs()
		.dispatch_with_priority(
			ExifEnrichJob::new(root.to_path_buf(), background),
			JobPriority::LOW,
			None,
		)
		.await
	{
		tracing::warn!(root = %root.display(), %error, "could not start photo metadata pass");
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sd_media_metadata::exif::{CameraData, ColorProfile, MediaLocation, Resolution};

	#[test]
	fn an_exif_read_maps_onto_the_facet() {
		let exif = ExifMetadata {
			resolution: Resolution::new(4032, 3024),
			date_taken: Some(MediaDate::Naive(
				chrono::NaiveDate::from_ymd_opt(2024, 3, 12)
					.unwrap()
					.and_hms_opt(10, 0, 0)
					.unwrap(),
			)),
			location: Some(MediaLocation::new(35.6812, 139.7671, None, None)),
			camera_data: CameraData {
				device_make: Some("Apple".into()),
				device_model: Some("iPhone 15 Pro".into()),
				color_space: Some("sRGB".into()),
				color_profile: Some(ColorProfile::Portrait),
				focal_length: Some(6.765),
				shutter_speed: Some(7.0),
				orientation: Orientation::CW90,
				lens_model: Some("iPhone 15 Pro back camera".into()),
				bit_depth: Some(8),
				iso: Some(80),
				..Default::default()
			},
			artist: Some("J".into()),
			description: Some("Tokyo".into()),
			copyright: None,
			exif_version: None,
		};

		let facet = facet_from_exif(&exif);
		assert_eq!(facet.width, Some(4032));
		assert_eq!(facet.height, Some(3024));
		assert_eq!(
			facet.date_taken.as_deref(),
			Some("2024-03-12T10:00:00+00:00")
		);
		assert_eq!(facet.latitude, Some(35.6812));
		assert_eq!(facet.longitude, Some(139.7671));
		assert_eq!(facet.camera_make.as_deref(), Some("Apple"));
		assert_eq!(facet.camera_model.as_deref(), Some("iPhone 15 Pro"));
		assert_eq!(
			facet.lens_model.as_deref(),
			Some("iPhone 15 Pro back camera")
		);
		assert_eq!(facet.focal_length.as_deref(), Some("6.765"));
		assert_eq!(facet.shutter_speed.as_deref(), Some("7"));
		assert_eq!(facet.iso, Some(80));
		assert_eq!(facet.orientation, Some(6));
		assert_eq!(facet.color_space.as_deref(), Some("sRGB"));
		assert_eq!(facet.color_profile.as_deref(), Some("Portrait"));
		assert_eq!(facet.bit_depth.as_deref(), Some("8"));
		assert_eq!(facet.artist.as_deref(), Some("J"));
		assert_eq!(facet.description.as_deref(), Some("Tokyo"));
		assert_eq!(facet.copyright, None);
		assert_eq!(facet.aperture, None);
	}

	#[test]
	fn an_empty_read_leaves_only_the_orientation() {
		// A reader that finds a segment but no tags reports the default
		// orientation; every other column is absent, which is what marks a
		// photo as read with nothing to show for it.
		let facet = facet_from_exif(&ExifMetadata::default());
		assert_eq!(facet.width, None);
		assert_eq!(facet.date_taken, None);
		assert_eq!(facet.latitude, None);
		assert_eq!(facet.orientation, Some(1));
	}

	#[tokio::test]
	async fn a_batch_reads_once_per_hash_and_skips_what_it_cannot_open() {
		let dir = tempfile::tempdir().unwrap();
		let plain = dir.path().join("plain.jpg");
		std::fs::write(&plain, vec![0xFFu8; 2048]).unwrap();
		let pending = |name: &str, hash: &str| {
			(
				PendingImage {
					uuid: uuid::Uuid::now_v7(),
					rowid: 1,
					external_id: name.to_string(),
					content_hash: hash.to_string(),
				},
				dir.path().join(name),
			)
		};

		let (facets, unreadable) = read_batch(vec![
			pending("plain.jpg", "a"),
			pending("plain.jpg", "a"),
			pending("gone.jpg", "b"),
		])
		.await;

		assert_eq!(unreadable, 1);
		assert_eq!(facets.len(), 1, "two records of one hash read once");
		assert_eq!(facets[0].0, "a");
		assert_eq!(facets[0].1, ImageFacet::default());
	}
}
