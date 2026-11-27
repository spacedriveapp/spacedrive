//! Reading EXIF into the image facet and container headers into the video
//! facet.
//!
//! Identification opens every file once and says what kind of bytes it
//! found. For an image or a video that is enough to know a header read is
//! worth it: capture time, place, camera and geometry for a photo, and
//! duration, frame size and codec for a video, which listings, columns, the
//! rename resolver and the inspector all want answered from the store rather
//! than from the file. This pass runs behind identification, claims records
//! whose facet row is missing or names other bytes, parses each on the
//! blocking pool, and writes one row per content hash. A video's header is
//! read without decoding a frame, so a long recording costs a few seeks.
//!
//! The work queue is the store (`sd_store::image`, `sd_store::video`), so the
//! pass resumes for free, never reads a file twice for the same bytes, and
//! picks up a changed file as soon as identification has given it a new
//! hash. Identification runs only inside a source's deep scope, so this pass
//! does too: a record outside it has no content hash to be claimed by.

use crate::infra::job::{generic_progress::GenericProgress, prelude::*, types::JobPriority};
use futures::StreamExt;
use sd_media_metadata::exif::{ExifMetadata, MediaDate, Orientation};
use sd_media_metadata::video::read_header;
use sd_store::{ImageFacet, PendingFacet, VideoFacet};
use serde::{Deserialize, Serialize};
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::Arc,
};

/// Records claimed from the store per pass. A header read is quick, so the
/// claim is sized to make the queue round trip disappear.
const BATCH_SIZE: usize = 256;

/// Files parsed at once. A JPEG read is a seek and a few kilobytes, but the
/// EXIF reader loads a TIFF-shaped container (TIFF, DNG, CR2, NEF, ARW and
/// the other raw kinds) whole, so the bound is memory for a raw folder, not
/// the drive's appetite for seeks. Four keeps a shoot of 50 MB raws under a
/// few hundred megabytes in flight.
const CONCURRENCY: usize = 4;

/// Reads EXIF for the image records and container headers for the video
/// records of a source that have no current facet row.
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
	/// Content hashes a video facet row was written for.
	#[serde(default)]
	pub videos: u64,
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
	const DESCRIPTION: Option<&'static str> = Some("Read photo and video metadata");
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

	/// Unreadable files stay pending, so a pass that only met those did
	/// nothing.
	fn keep_record(&self, output: &Self::Output) -> bool {
		!self.background || output.enriched > 0 || output.videos > 0
	}

	fn finished_rows_kept(&self) -> Option<u64> {
		self.background
			.then_some(super::content_identity::BACKGROUND_HISTORY)
	}

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
				videos: 0,
				unreadable: 0,
			});
		};

		// Identities are queued behind the same writer; claim what landed.
		store.flush().await.map_err(|e| e.to_string())?;

		let label = super::content_identity::source_label(&self.root);
		let mut output = ExifEnrichOutput {
			enriched: 0,
			with_exif: 0,
			videos: 0,
			unreadable: 0,
		};
		// Rows already attempted under the same hash, per kind, for the whole
		// job: a file that could not be opened is neither opened nor counted
		// twice, while one whose bytes changed presents a new hash and is read
		// again.
		let mut attempted: [HashSet<(i64, String)>; 2] = Default::default();
		// A record identified while this pass runs is pending at a row id
		// already passed, or in a kind already swept, and a second dispatch
		// deduplicates onto this job rather than queueing behind it. So the
		// pass sweeps every kind again until a round reads nothing new.
		loop {
			let mut read_any = false;
			for (media, attempted) in [Media::Image, Media::Video].into_iter().zip(&mut attempted) {
				let outstanding = store
					.files_needing_facets_count(media.kind())
					.await
					.map_err(|e| e.to_string())?;
				if outstanding == 0 {
					continue;
				}
				ctx.log(format!(
					"{outstanding} {} to read metadata for",
					media.noun()
				));
				let progress = |done: u64| {
					Progress::generic(GenericProgress::new(
						(done as f32 / outstanding as f32).min(1.0),
						media.title(),
						format!("{label} — {done} of {outstanding} {}", media.noun()),
					))
				};
				ctx.progress(progress(0));

				let mut done = 0u64;
				let mut cursor = 0i64;
				loop {
					ctx.check_interrupt().await?;

					let claimed = store
						.files_needing_facets(media.kind(), cursor, BATCH_SIZE)
						.await
						.map_err(|e| e.to_string())?;
					let Some(last) = claimed.last() else {
						break;
					};
					cursor = last.0.rowid;
					let batch: Vec<_> = claimed
						.into_iter()
						.filter(|(pending, _)| {
							attempted.insert((pending.rowid, pending.content_hash.clone()))
						})
						.collect();
					if batch.is_empty() {
						continue;
					}
					read_any = true;
					done += batch.len() as u64;

					match media {
						Media::Image => {
							let (facets, failed) = read_batch(batch, read_image).await;
							output.unreadable += failed;
							output.enriched += facets.len() as u64;
							output.with_exif += facets
								.iter()
								.filter(|(_, facet)| *facet != ImageFacet::default())
								.count() as u64;
							store.image_facets(facets).await;
						}
						Media::Video => {
							let (facets, failed) = read_batch(batch, read_video).await;
							output.unreadable += failed;
							output.videos += facets.len() as u64;
							store.video_facets(facets).await;
						}
					}
					ctx.progress(progress(done));

					// The next claim reads the database; a claim that outraces
					// the writer would hand back rows this pass already wrote.
					store.flush().await.map_err(|e| e.to_string())?;
				}
			}
			if !read_any {
				break;
			}
		}

		store.flush().await.map_err(|e| e.to_string())?;
		ctx.log(format!(
			"Read metadata for {} distinct images, {} with EXIF, {} distinct videos, {} unreadable",
			output.enriched, output.with_exif, output.videos, output.unreadable
		));
		Ok(output)
	}
}

/// The two kinds of bytes this pass reads a header from.
#[derive(Clone, Copy)]
enum Media {
	Image,
	Video,
}

impl Media {
	fn kind(self) -> crate::domain::ContentKind {
		match self {
			Self::Image => crate::domain::ContentKind::Image,
			Self::Video => crate::domain::ContentKind::Video,
		}
	}

	fn noun(self) -> &'static str {
		match self {
			Self::Image => "images",
			Self::Video => "videos",
		}
	}

	fn title(self) -> &'static str {
		match self {
			Self::Image => "Reading photo metadata",
			Self::Video => "Reading video metadata",
		}
	}
}

/// Parse one file per content hash in the batch, several at a time.
///
/// A file that cannot be opened is skipped and counted: the record keeps no
/// facet row, so the next pass claims it again, which is right for a drive
/// that hiccuped and harmless for a file the next walk removes. A file with
/// no EXIF, or EXIF the reader cannot make sense of, gets an empty row so
/// the bytes are never read again.
async fn read_batch<T, F, Fut>(
	batch: Vec<(PendingFacet, PathBuf)>,
	read: F,
) -> (Vec<(String, T)>, u64)
where
	F: Fn(PathBuf) -> Fut,
	Fut: std::future::Future<Output = Option<T>>,
{
	let mut by_hash: HashMap<String, PathBuf> = HashMap::new();
	for (pending, path) in batch {
		by_hash.entry(pending.content_hash).or_insert(path);
	}

	let read = &read;
	let results: Vec<_> = futures::stream::iter(by_hash)
		.map(|(hash, path)| async move {
			// Unread and unrecorded, so the pass claims it again once the
			// file is downloaded.
			if crate::ops::indexing::metadata::refuse_dataless(&path)
				.await
				.is_err()
			{
				return None;
			}
			Some((hash, read(path).await?))
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

/// An image's facet, or `None` when the file could not be opened.
async fn read_image(path: PathBuf) -> Option<ImageFacet> {
	match ExifMetadata::from_path(&path).await {
		Ok(Some(exif)) => Some(facet_from_exif(&exif)),
		Ok(None) => Some(ImageFacet::default()),
		Err(sd_media_metadata::Error::FileIO(error)) => {
			tracing::debug!(path = %path.display(), %error, "could not open for EXIF");
			None
		}
		Err(error) => {
			tracing::debug!(path = %path.display(), %error, "EXIF unreadable");
			Some(ImageFacet::default())
		}
	}
}

/// A video's facet from its container header, or `None` when the file
/// could not be opened. A header this reader does not know, or one cut
/// short, records an empty row so the bytes are not read again.
async fn read_video(path: PathBuf) -> Option<VideoFacet> {
	let read = tokio::task::spawn_blocking({
		let path = path.clone();
		move || read_header(&path)
	})
	.await
	.ok()?;
	match read {
		Ok(header) => Some(
			header.map_or_else(VideoFacet::default, |header| VideoFacet {
				duration_seconds: header.duration_seconds,
				width: header.width.map(i64::from),
				height: header.height.map(i64::from),
				codec: header.codec,
			}),
		),
		Err(error)
			if matches!(
				error.kind(),
				std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::InvalidData
			) =>
		{
			tracing::debug!(path = %path.display(), %error, "video header unreadable");
			Some(VideoFacet::default())
		}
		Err(error) => {
			tracing::debug!(path = %path.display(), %error, "could not open for a video header");
			None
		}
	}
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
	let mut outstanding = 0;
	for kind in [
		crate::domain::ContentKind::Image,
		crate::domain::ContentKind::Video,
	] {
		match store.files_needing_facets_count(kind).await {
			Ok(count) => outstanding += count,
			Err(error) => {
				tracing::warn!(root = %root.display(), %error, "could not count media needing metadata");
				return;
			}
		}
	}
	if outstanding == 0 {
		return;
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
		tracing::warn!(root = %root.display(), %error, "could not start the media metadata pass");
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
				PendingFacet {
					uuid: uuid::Uuid::now_v7(),
					rowid: 1,
					external_id: name.to_string(),
					content_hash: hash.to_string(),
				},
				dir.path().join(name),
			)
		};

		let (facets, unreadable) = read_batch(
			vec![
				pending("plain.jpg", "a"),
				pending("plain.jpg", "a"),
				pending("gone.jpg", "b"),
			],
			read_image,
		)
		.await;

		assert_eq!(unreadable, 1);
		assert_eq!(facets.len(), 1, "two records of one hash read once");
		assert_eq!(facets[0].0, "a");
		assert_eq!(facets[0].1, ImageFacet::default());
	}
}
