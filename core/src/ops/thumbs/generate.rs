//! # Thumbnail generation
//!
//! A path selects known files from the volume map. Only cache artifacts are
//! written; source assertions and indexing coverage keep their existing owners.

use std::{
	path::{Component, PathBuf},
	sync::Arc,
	time::Duration,
};

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::{handle::JobReceipt, prelude::*, traits::DynJob},
	},
	library::Library,
	service::thumbs::{GenerationOutcome, ThumbnailGenerationMode},
};

const BATCH_SIZE: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbnailGenerateInput {
	pub scope: SdPath,
	/// Include indexed subdirectories. False selects only immediate files.
	#[serde(default)]
	pub recursive: bool,
	#[serde(default)]
	pub mode: ThumbnailGenerationMode,
}

pub struct ThumbnailGenerateAction {
	input: ThumbnailGenerateInput,
}

impl LibraryAction for ThumbnailGenerateAction {
	type Input = ThumbnailGenerateInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		let path = input
			.scope
			.as_local_path()
			.ok_or("Thumbnail generation requires a path on this device")?;
		if !path.is_absolute()
			|| path
				.components()
				.any(|part| matches!(part, Component::ParentDir))
		{
			return Err("Thumbnail scope must be an absolute path without parent traversal".into());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<JobReceipt, ActionError> {
		let path = self.input.scope.as_local_path().unwrap().to_path_buf();
		let path = context
			.volume_manager
			.locate_path(&path)
			.await
			.map(|(_, path)| path)
			.unwrap_or(path);
		let metadata = tokio::fs::symlink_metadata(&path)
			.await
			.map_err(|error| ActionError::InvalidInput(format!("{}: {error}", path.display())))?;
		if !metadata.is_file() && !metadata.is_dir() {
			return Err(ActionError::InvalidInput(
				"Select a regular file or directory".into(),
			));
		}
		let cache = context.volume_index();
		cache.ensure_restored(&path).await;
		let slot = cache.resolve(&path);
		let volume_id = slot.id().ok_or_else(|| {
			ActionError::InvalidInput("The path is not on a mapped volume".into())
		})?;
		let root = slot
			.root()
			.ok_or_else(|| ActionError::InvalidInput("The volume is unavailable".into()))?;
		if slot.is_detached() || !slot.index().read().await.has_entry(&path) {
			return Err(ActionError::InvalidInput(
				"The path is not in the available index. Browse or index it first.".into(),
			));
		}
		let relative = path
			.strip_prefix(root)
			.map_err(|error| ActionError::InvalidInput(error.to_string()))?
			.to_path_buf();
		let job = ThumbnailGenerateJob {
			volume_id,
			relative,
			recursive: self.input.recursive,
			mode: self.input.mode,
			cursor: None,
			generated: 0,
			skipped: 0,
			failed: 0,
		};
		library
			.jobs()
			.dispatch(job)
			.await
			.map(Into::into)
			.map_err(ActionError::Job)
	}

	fn action_kind(&self) -> &'static str {
		"thumbs.generate"
	}
}

crate::register_library_action!(ThumbnailGenerateAction, "thumbs.generate");

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ThumbnailGenerateJob {
	volume_id: Uuid,
	relative: PathBuf,
	recursive: bool,
	mode: ThumbnailGenerationMode,
	/// Last settled path relative to the volume, so remounts retain progress.
	cursor: Option<PathBuf>,
	generated: u64,
	skipped: u64,
	failed: u64,
}

impl Job for ThumbnailGenerateJob {
	const NAME: &'static str = "thumbnail_generate";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> =
		Some("Generate thumbnails for a file or indexed folder");
}

impl DynJob for ThumbnailGenerateJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
	fn dedup_key(&self) -> Option<String> {
		Some(format!(
			"{}:{:?}:{}:{:?}",
			self.volume_id, self.relative, self.recursive, self.mode
		))
	}
}

#[async_trait::async_trait]
impl JobHandler for ThumbnailGenerateJob {
	type Output = JobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<JobOutput> {
		// Checkpoints can be newer than the job row after an abrupt restart.
		if let Some(saved) = ctx.load_state::<Self>().await? {
			*self = saved;
		}
		let core = ctx.library().core_context();
		let cache = core.volume_index();
		let root = cache
			.volume_index_root(self.volume_id)
			.ok_or_else(|| JobError::execution("Thumbnail volume is unavailable"))?;
		let scope = root.join(&self.relative);
		cache.ensure_restored(&scope).await;
		let slot = cache.resolve(&scope);
		if slot.id() != Some(self.volume_id) || slot.is_detached() {
			return Err(JobError::execution(
				"Thumbnail scope no longer resolves to its original volume",
			));
		}
		let metadata = tokio::fs::symlink_metadata(&scope)
			.await
			.map_err(|error| JobError::execution(error.to_string()))?;
		if !metadata.is_file() && !metadata.is_dir() {
			return Err(JobError::execution(
				"Thumbnail scope is no longer a regular file or directory",
			));
		}
		let (files, summarised) = {
			let index = slot.index();
			let index = index.read().await;
			let files = index
				.files_in_scope(&scope, self.recursive)
				.ok_or_else(|| {
					JobError::execution(
						"Thumbnail scope is absent from the index; browse or index it first",
					)
				})?;
			let summarised = index
				.summarised_paths()
				.iter()
				.filter(|path| path.starts_with(&scope) && (self.recursive || **path == scope))
				.count();
			(files, summarised)
		};
		ctx.log(format!(
			"Generating thumbnails for {} known files at {}",
			files.len(),
			scope.display()
		));
		if summarised > 0 {
			ctx.add_warning(format!("{summarised} summarised directories have no file listing. Only indexed files will be processed."));
		}
		let files: Vec<_> = files
			.into_iter()
			.filter(|path| {
				self.cursor.as_ref().is_none_or(|cursor| {
					path.strip_prefix(&root)
						.is_ok_and(|relative| relative > cursor.as_path())
				})
			})
			.collect();
		let total = self.generated + self.skipped + self.failed + files.len() as u64;
		for batch in files.chunks(BATCH_SIZE) {
			ctx.check_interrupt().await?;
			// Refuse a swapped or detached drive before submitting another batch.
			if cache.resolve(&scope).id() != Some(self.volume_id) || cache.is_detached(&scope) {
				return Err(JobError::execution("Thumbnail volume became unavailable"));
			}
			let mode = self.mode;
			let thumbs = core.thumbs.clone();
			let mut outcomes = stream::iter(batch.to_vec())
				.map(move |path| {
					let thumbs = thumbs.clone();
					async move { thumbs.generate_one(&path, mode).await }
				})
				.buffer_unordered(BATCH_SIZE);
			let (mut generated, mut skipped, mut failed) = (0, 0, 0);
			loop {
				tokio::select! {
					outcome = outcomes.next() => match outcome {
						Some(GenerationOutcome::Generated) => generated += 1,
						Some(GenerationOutcome::Skipped) => skipped += 1,
						Some(GenerationOutcome::Failed) => failed += 1,
						None => break,
					},
					_ = tokio::time::sleep(Duration::from_millis(100)) => ctx.check_interrupt().await?,
				}
			}
			core.thumbs
				.flush_cache(self.volume_id)
				.await
				.map_err(JobError::execution)?;
			self.generated += generated;
			self.skipped += skipped;
			self.failed += failed;
			self.cursor = batch
				.last()
				.and_then(|path| path.strip_prefix(&root).ok())
				.map(PathBuf::from);
			ctx.checkpoint_with_state(self).await?;
			let completed = self.generated + self.skipped + self.failed;
			ctx.progress(Progress::generic(GenericProgress::new(
				completed as f32 / total.max(1) as f32,
				"Generating thumbnails",
				format!(
					"{completed}/{total}: {} generated, {} skipped, {} failed",
					self.generated, self.skipped, self.failed
				),
			)));
		}
		ctx.log(format!(
			"Thumbnails: {} generated, {} skipped, {} failed",
			self.generated, self.skipped, self.failed
		));
		Ok(JobOutput::ThumbnailGeneration {
			generated_count: self.generated,
			skipped_count: self.skipped,
			error_count: self.failed,
			total_size_bytes: 0,
		})
	}

	fn is_resuming(&self) -> bool {
		self.cursor.is_some()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::indexing::{metadata::EntryMetadata, state::EntryKind, Arena};

	#[test]
	fn thumbnail_scope_selects_file_children_or_descendants_only() {
		let mut index = Arena::new().unwrap();
		for name in [
			"/photos/a.png",
			"/photos/child/b.png",
			"/photos-other/c.png",
		] {
			let path = PathBuf::from(name);
			index
				.add_entry(
					path.clone(),
					Uuid::new_v4(),
					EntryMetadata {
						path,
						kind: EntryKind::File,
						size: 1,
						modified: None,
						accessed: None,
						created: None,
						inode: None,
						permissions: None,
						uid: None,
						gid: None,
						link_target: None,
						is_hidden: false,
					},
				)
				.unwrap();
		}
		assert_eq!(
			index
				.files_in_scope(std::path::Path::new("/photos/a.png"), true)
				.unwrap(),
			vec![PathBuf::from("/photos/a.png")]
		);
		assert_eq!(
			index
				.files_in_scope(std::path::Path::new("/photos"), false)
				.unwrap(),
			vec![PathBuf::from("/photos/a.png")]
		);
		assert_eq!(
			index
				.files_in_scope(std::path::Path::new("/photos"), true)
				.unwrap(),
			vec![
				PathBuf::from("/photos/a.png"),
				PathBuf::from("/photos/child/b.png")
			]
		);
		assert!(index
			.files_in_scope(std::path::Path::new("/missing"), true)
			.is_none());
	}

	#[test]
	fn thumbnail_scope_rejects_remote_and_relative_paths() {
		for scope in [
			SdPath::Physical {
				device_slug: "another-machine".into(),
				path: "/photos".into(),
			},
			SdPath::Physical {
				device_slug: "local".into(),
				path: "photos".into(),
			},
			SdPath::Physical {
				device_slug: "local".into(),
				path: "/photos/../private".into(),
			},
		] {
			assert!(ThumbnailGenerateAction::from_input(ThumbnailGenerateInput {
				scope,
				recursive: true,
				mode: ThumbnailGenerationMode::Force
			})
			.is_err());
		}
	}

	#[test]
	fn thumbnail_checkpoint_retains_cursor_and_modes() {
		let job = ThumbnailGenerateJob {
			volume_id: Uuid::new_v4(),
			relative: "photos".into(),
			recursive: true,
			mode: ThumbnailGenerationMode::Force,
			cursor: Some("photos/child/b.png".into()),
			generated: 10,
			skipped: 2,
			failed: 1,
		};
		let restored: ThumbnailGenerateJob =
			rmp_serde::from_slice(&rmp_serde::to_vec(&job).unwrap()).unwrap();
		assert_eq!(restored.cursor, job.cursor);
		assert_eq!(restored.dedup_key(), job.dedup_key());
		assert_eq!(
			(restored.generated, restored.skipped, restored.failed),
			(10, 2, 1)
		);
	}

	#[tokio::test]
	async fn thumbnail_job_resumes_after_completed_path_and_waits_for_cache() {
		let data = tempfile::tempdir().unwrap();
		let media = tempfile::tempdir().unwrap();
		let root = media.path().canonicalize().unwrap();
		let first = root.join("a.png");
		let second = root.join("b.png");
		for path in [&first, &second] {
			image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
				.save(path)
				.unwrap();
		}
		let core = crate::Core::new(data.path().to_path_buf()).await.unwrap();
		let library = core
			.libraries
			.create_library("Thumbnail test", None, core.context.clone())
			.await
			.unwrap();
		let cache = core.context.volume_index();
		cache.track_volume(Uuid::new_v4(), root.clone());
		for path in [&first, &second] {
			let metadata = crate::ops::indexing::metadata::extract_metadata(path, None)
				.await
				.unwrap();
			cache
				.resolve_index(path)
				.write()
				.await
				.add_entry(path.clone(), Uuid::new_v4(), metadata)
				.unwrap();
		}
		let slot = cache.resolve(&root);
		let volume_id = slot.id().unwrap();
		let job = ThumbnailGenerateJob {
			volume_id,
			relative: PathBuf::new(),
			recursive: true,
			mode: ThumbnailGenerationMode::Force,
			cursor: Some("a.png".into()),
			generated: 1,
			skipped: 0,
			failed: 0,
		};
		let handle = library.jobs().dispatch(job).await.unwrap();
		let output = tokio::time::timeout(Duration::from_secs(20), handle.wait())
			.await
			.unwrap()
			.unwrap();
		assert!(matches!(
			output,
			JobOutput::ThumbnailGeneration {
				generated_count: 2,
				error_count: 0,
				..
			}
		));
		let index = slot.index();
		let index = index.read().await;
		let first_id = index.get_entry_uuid(&first).unwrap();
		let second_id = index.get_entry_uuid(&second).unwrap();
		let mut reader =
			sd_pvcache::PvcacheReader::open(&core.context.thumbs.cache_path(volume_id).unwrap())
				.unwrap();
		assert_eq!(
			reader.lookup(first_id, 0).unwrap(),
			sd_pvcache::TileState::Absent
		);
		assert!(reader.lookup(second_id, 0).unwrap().frame().is_some());
	}
}
