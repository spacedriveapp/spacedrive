//! The transcription job: every audio and video file under a path whose
//! transcript is missing or was made from other bytes.
//!
//! The work queue is the content text index's transcript table, the same way
//! the text pass uses its own rows: a file is pending until a transcript row
//! stamped with its current size and mtime exists. That makes the job
//! resumable for free, and an edited recording is transcribed again on the
//! next run while a renamed one is not.

use std::{
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
	audio::{open_speech_audio, Opened},
	engine::{Engine, TranscribeError},
	transcript::{read_transcript, Transcript, MEDIA_EXTENSIONS},
};
use crate::{
	domain::sidecar::{content_uuid_of_file, ContentSidecar, SidecarFormat},
	infra::job::{generic_progress::GenericProgress, prelude::*},
	ops::models::whisper::{WhisperModel, WhisperModelManager},
};

/// Files claimed per round.
const BATCH_SIZE: usize = 16;

/// Transcribes the audio and video files at or under one path of a source.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct TranscribeJob {
	/// The source's root, which is how its store is found.
	root: PathBuf,
	/// The file or directory to transcribe, relative to `root`. Empty means
	/// the whole source.
	under: String,
	/// A Whisper model name, such as `base`.
	model: String,
	/// Pin the language, such as `en`. None detects it per file.
	language: Option<String>,
	/// Make transcripts again even when one exists for the bytes.
	force: bool,
	/// The forced pass has dropped the old rows, so a resumed run does not
	/// drop the ones it already wrote.
	#[serde(default)]
	forgotten: bool,
	/// The record row the pass has reached. A file that failed is not tried
	/// again in the same run.
	#[serde(default)]
	cursor: i64,
}

impl TranscribeJob {
	pub fn new(
		root: PathBuf,
		under: String,
		model: WhisperModel,
		language: Option<String>,
		force: bool,
	) -> Self {
		Self {
			root,
			under,
			model: model_name(model).to_string(),
			language,
			force,
			forgotten: false,
			cursor: 0,
		}
	}
}

/// The name `WhisperModel::from_str` reads back.
pub fn model_name(model: WhisperModel) -> &'static str {
	model.id().trim_start_matches("whisper-")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscribeOutput {
	/// Files the model transcribed.
	pub transcribed: u64,
	/// Files whose transcript sidecar already existed and was indexed again.
	pub reused: u64,
	/// Files with no audio track.
	pub no_audio: u64,
	/// Files that could not be read or decoded. Left pending.
	pub failed: u64,
	/// Rows removed because their file is gone.
	pub pruned: u64,
}

impl From<TranscribeOutput> for JobOutput {
	fn from(output: TranscribeOutput) -> Self {
		JobOutput::custom(output)
	}
}

impl Job for TranscribeJob {
	const NAME: &'static str = "transcribe";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Transcribe speech");
}

impl crate::infra::job::traits::DynJob for TranscribeJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn dedup_key(&self) -> Option<String> {
		Some(format!("transcribe:{}:{}", self.root.display(), self.under))
	}
}

#[async_trait::async_trait]
impl JobHandler for TranscribeJob {
	type Output = TranscribeOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let volume_index = ctx.library().core_context().volume_index();
		if let Some(reason) = volume_index.dispatch_refusal(&self.root) {
			return Err(JobError::execution(reason));
		}
		let Some(store) = volume_index.store_for(&self.root).await else {
			return Err(JobError::execution(format!(
				"no source store at {}",
				self.root.display()
			)));
		};
		store.flush().await.map_err(|e| e.to_string())?;
		let index = store.text_index().await.map_err(|e| e.to_string())?;

		let model = WhisperModel::from_str(&self.model)
			.ok_or_else(|| JobError::execution(format!("unknown model {}", self.model)))?;
		let data_dir = crate::config::default_data_dir().map_err(|e| e.to_string())?;
		let models = WhisperModelManager::new(&data_dir);
		if !models.is_downloaded(&model).await {
			return Err(JobError::execution(format!(
				"the {} model is not downloaded; run models.whisper.download first",
				self.model
			)));
		}
		let model_path = models.get_model_path(&model);

		let mut output = TranscribeOutput {
			pruned: index.prune_transcripts().await.map_err(|e| e.to_string())?,
			..Default::default()
		};
		if self.force && !self.forgotten {
			let dropped = index
				.forget_transcripts(&self.under)
				.await
				.map_err(|e| e.to_string())?;
			self.forgotten = true;
			ctx.checkpoint().await?;
			ctx.log(format!("Dropped {dropped} transcripts to make them again"));
		}

		let library_path = ctx.library().path().to_path_buf();
		let mut engine: Option<Arc<Engine>> = None;
		let mut done = 0u64;
		let mut outstanding = index
			.pending_transcripts(MEDIA_EXTENSIONS, &self.under, self.cursor, usize::MAX >> 1)
			.await
			.map_err(|e| e.to_string())?
			.len() as u64;
		ctx.log(format!(
			"{outstanding} recordings to transcribe with {}",
			model.id()
		));

		loop {
			ctx.check_interrupt().await?;
			let claimed = index
				.pending_transcripts(MEDIA_EXTENSIONS, &self.under, self.cursor, BATCH_SIZE)
				.await
				.map_err(|e| e.to_string())?;
			if claimed.is_empty() {
				break;
			}
			for file in claimed {
				ctx.check_interrupt().await?;
				self.cursor = file.rowid;
				outstanding = outstanding.max(done + 1);
				let path = self.root.join(&file.external_id);
				let (position, total, name, job) = (done, outstanding, &file.external_id, &ctx);
				let report = move |fraction: f32| {
					job.progress(Progress::generic(GenericProgress::new(
						((position as f32 + fraction) / total as f32).min(1.0),
						"Transcribing",
						format!("{name} — {} of {total}", position + 1),
					)));
				};
				report(0.0);

				let content_uuid = match file.content_uuid {
					Some(uuid) => uuid,
					None => match content_uuid_of_file(&path).await {
						Ok(uuid) => uuid,
						Err(error) => {
							ctx.log(format!("Skipped {}: {error}", file.external_id));
							output.failed += 1;
							done += 1;
							continue;
						}
					},
				};

				let reusable = if self.force {
					None
				} else {
					read_transcript(&library_path, content_uuid)
						.await
						.ok()
						.flatten()
				};
				let from_model = reusable.is_none();
				let transcript = match reusable {
					Some(transcript) => Some(transcript),
					None => {
						if engine.is_none() {
							let (path, id) = (model_path.clone(), model.id().to_string());
							let loaded =
								tokio::task::spawn_blocking(move || Engine::load(&path, &id))
									.await
									.map_err(|e| e.to_string())??;
							engine = Some(Arc::new(loaded));
						}
						let engine = engine.clone().expect("loaded above");
						match self
							.transcribe_one(
								&ctx,
								engine,
								&library_path,
								content_uuid,
								&path,
								&report,
							)
							.await?
						{
							Ok(transcript) => transcript,
							Err(error) => {
								ctx.log(format!(
									"Could not transcribe {}: {error}",
									file.external_id
								));
								output.failed += 1;
								done += 1;
								continue;
							}
						}
					}
				};

				let row = match &transcript {
					Some(transcript) => {
						if from_model {
							if let Err(error) =
								write_sidecars(&library_path, content_uuid, transcript).await
							{
								ctx.log(format!(
									"Could not write the transcript of {}: {error}",
									file.external_id
								));
								output.failed += 1;
								done += 1;
								continue;
							}
							output.transcribed += 1;
						} else {
							output.reused += 1;
						}
						sd_store::TranscriptDoc {
							record_uuid: file.record_uuid,
							size: file.size,
							mtime: file.mtime,
							extension: file.extension.clone(),
							language: transcript.language.clone(),
							model: transcript.model.clone(),
							duration_ms: Some(transcript.duration_ms as i64),
							spans: transcript
								.segments
								.iter()
								.map(|segment| sd_store::TranscriptSpan {
									start_ms: segment.start_ms as i64,
									end_ms: segment.end_ms as i64,
									text: segment.text.clone(),
								})
								.collect(),
							skipped: None,
						}
					}
					None => {
						output.no_audio += 1;
						sd_store::TranscriptDoc {
							record_uuid: file.record_uuid,
							size: file.size,
							mtime: file.mtime,
							extension: file.extension.clone(),
							language: None,
							model: model.id().to_string(),
							duration_ms: None,
							spans: Vec::new(),
							skipped: Some("no audio track".into()),
						}
					}
				};
				index
					.write_transcript(&row)
					.await
					.map_err(|e| e.to_string())?;
				done += 1;
			}
			ctx.checkpoint().await?;
		}

		ctx.progress(Progress::generic(GenericProgress::new(
			1.0,
			"Transcribing",
			format!("{done} of {outstanding}"),
		)));
		ctx.log(format!(
			"Transcribed {}, reused {}, {} without audio, {} failed",
			output.transcribed, output.reused, output.no_audio, output.failed
		));
		Ok(output)
	}
}

impl TranscribeJob {
	/// Run the model over one file on a blocking thread, reporting how far
	/// through it is and stopping when the job is interrupted. The outer
	/// result is the job's; the inner one is this file's.
	async fn transcribe_one(
		&self,
		ctx: &JobContext<'_>,
		engine: Arc<Engine>,
		library_path: &Path,
		content_uuid: Uuid,
		path: &Path,
		report: &(dyn Fn(f32) + Sync),
	) -> JobResult<Result<Option<Transcript>, String>> {
		let cancel = Arc::new(AtomicBool::new(false));
		let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<f32>();
		let task = {
			let (cancel, language) = (cancel.clone(), self.language.clone());
			let (library_path, path) = (library_path.to_path_buf(), path.to_path_buf());
			tokio::task::spawn_blocking(move || {
				let mut audio = match open_speech_audio(&library_path, content_uuid, &path)? {
					Opened::NoAudio => return Ok(None),
					Opened::Audio(audio) => audio,
				};
				let total = audio.duration_ms().filter(|ms| *ms > 0);
				let mut progress = |read: u64| {
					if let Some(total) = total {
						let _ = sender.send((read as f32 / total as f32).min(1.0));
					}
				};
				engine
					.transcribe(audio.as_mut(), language.as_deref(), &mut progress, &cancel)
					.map(Some)
			})
		};
		tokio::pin!(task);
		let mut interrupted = None;
		let result = loop {
			tokio::select! {
				result = &mut task => break result,
				Some(fraction) = receiver.recv() => report(fraction),
				_ = tokio::time::sleep(Duration::from_millis(500)), if interrupted.is_none() => {
					if let Err(error) = ctx.check_interrupt().await {
						cancel.store(true, Ordering::Relaxed);
						interrupted = Some(error);
					}
				}
			}
		};
		if let Some(error) = interrupted {
			return Err(error);
		}
		Ok(match result {
			Ok(Ok(transcript)) => Ok(transcript),
			Ok(Err(TranscribeError::Failed(error))) => Err(error),
			Ok(Err(TranscribeError::Cancelled)) => Err("cancelled".into()),
			Err(join) => Err(format!("transcription panicked: {join}")),
		})
	}
}

/// Write the JSON, SRT and VTT sidecars. Each goes through
/// `.<variant>.partial.<ext>` and a rename, so a reader never sees half a
/// file. The JSON is renamed last: its presence means the set is complete.
async fn write_sidecars(
	library_path: &Path,
	content_uuid: Uuid,
	transcript: &Transcript,
) -> std::io::Result<()> {
	let json = serde_json::to_vec_pretty(transcript)?;
	for (format, bytes) in [
		(SidecarFormat::Srt, transcript.to_srt().into_bytes()),
		(SidecarFormat::Vtt, transcript.to_vtt().into_bytes()),
		(SidecarFormat::Json, json),
	] {
		let sidecar = ContentSidecar::transcript(content_uuid, format);
		let path = sidecar.path_in(library_path);
		let dir = path.parent().expect("a sidecar path has a directory");
		tokio::fs::create_dir_all(dir).await?;
		let partial = dir.join(format!(
			".{}.partial.{}",
			sidecar.variant.as_str(),
			sidecar.format.extension()
		));
		tokio::fs::write(&partial, &bytes).await?;
		tokio::fs::rename(&partial, &path).await?;
	}
	Ok(())
}
