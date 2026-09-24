//! Writing an archive, and extracting one, entry by entry.
//!
//! Both do their reading and writing on a blocking thread that reports
//! each entry over a channel, so the job's side keeps progress, the
//! journal, checkpoints and interruption. An archive is written to a
//! temporary name beside its destination and renamed into place when
//! complete, so an interrupted job leaves nothing half-written. An extract
//! checkpoints the entry it reached and resumes there; a file it replaces
//! goes to the trash first, so the replacement can be undone.

use std::{
	collections::HashSet,
	io,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Instant,
};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::mpsc;

use super::{
	input::{ArchiveFormat, FileArchiveInput, FileExtractInput},
	plan::decide,
	preflight::read_directory,
};
use crate::{
	infra::job::{generic_progress::GenericProgress, journal::Effect, prelude::*},
	ops::files::{merge::MergeConflictPolicy, plan::ChangeKind, trash},
};

const CHECKPOINT_EVERY: usize = 64;

/// The suffix an archive carries until it is complete.
const PARTIAL: &str = "sd-partial";

/// What the blocking side reports.
enum Report {
	/// One entry written, with its bytes.
	Entry {
		index: usize,
		bytes: u64,
	},
	Failed {
		index: usize,
		error: String,
	},
	Done(Result<(), String>),
}

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ArchiveJob {
	pub input: FileArchiveInput,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl ArchiveJob {
	pub fn new(input: FileArchiveInput) -> Self {
		Self {
			input,
			started_at: Instant::now(),
		}
	}
}

impl Job for ArchiveJob {
	const NAME: &'static str = "archive_files";
	const RESUMABLE: bool = false;
	const DESCRIPTION: Option<&'static str> = Some("Write an archive");
}

impl crate::infra::job::traits::DynJob for ArchiveJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn dedup_key(&self) -> Option<String> {
		Some(self.input.destination.to_string())
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ArchiveOutput {
	pub archive: PathBuf,
	pub files: u64,
	pub bytes: u64,
	pub left: Vec<ArchiveProblem>,
	pub sources_removed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ArchiveProblem {
	pub path: PathBuf,
	pub reason: String,
}

impl From<ArchiveOutput> for JobOutput {
	fn from(output: ArchiveOutput) -> Self {
		JobOutput::custom(output)
	}
}

/// One file or folder to put in the archive, under its name there.
struct Member {
	path: PathBuf,
	name: String,
	is_dir: bool,
	size: u64,
}

#[async_trait::async_trait]
impl JobHandler for ArchiveJob {
	type Output = ArchiveOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let destination = self
			.input
			.destination
			.as_local_path()
			.ok_or_else(|| JobError::execution("the archive would be written on another device"))?
			.to_path_buf();
		if tokio::fs::symlink_metadata(&destination).await.is_ok() {
			return Err(JobError::execution(format!(
				"{} is already there",
				destination.display()
			)));
		}
		let partial = partial_name(&destination);
		let _ = tokio::fs::remove_file(&partial).await;

		ctx.progress(Progress::Indeterminate("Listing the sources".to_string()));
		let mut members = Vec::new();
		let mut left = Vec::new();
		for source in &self.input.sources {
			let Some(path) = source.as_local_path() else {
				left.push(ArchiveProblem {
					path: PathBuf::new(),
					reason: format!("{source} is on another device"),
				});
				continue;
			};
			gather(path, &mut members, &mut left, self.input.format).await;
		}
		let total_bytes: u64 = members.iter().map(|member| member.size).sum();
		let total_files = members.iter().filter(|member| !member.is_dir).count() as u64;
		ctx.log(format!(
			"{total_files} files ({total_bytes} bytes) into {}",
			destination.display()
		));

		let (report_tx, mut report_rx) = mpsc::unbounded_channel();
		let cancel = Arc::new(AtomicBool::new(false));
		let worker = {
			let partial = partial.clone();
			let format = self.input.format;
			let cancel = cancel.clone();
			let members: Vec<(PathBuf, String, bool)> = members
				.iter()
				.map(|member| (member.path.clone(), member.name.clone(), member.is_dir))
				.collect();
			tokio::task::spawn_blocking(move || {
				let result = write_archive(&partial, format, &members, &cancel, &report_tx);
				let _ = report_tx.send(Report::Done(result.map_err(|error| error.to_string())));
			})
		};

		let mut bytes = 0u64;
		let mut files = 0u64;
		let mut outcome: Option<Result<(), String>> = None;
		while let Some(report) = report_rx.recv().await {
			if ctx.check_interrupt().await.is_err() {
				cancel.store(true, Ordering::Relaxed);
			}
			match report {
				Report::Entry {
					index,
					bytes: written,
				} => {
					bytes += written;
					if !members[index].is_dir {
						files += 1;
					}
					ctx.progress(Progress::generic(
						GenericProgress::new(
							(bytes as f32 / total_bytes.max(1) as f32).min(1.0),
							"Archiving",
							format!("{files} of {total_files} files"),
						)
						.with_completion(files, total_files)
						.with_bytes(bytes, total_bytes),
					));
				}
				Report::Failed { index, error } => left.push(ArchiveProblem {
					path: members[index].path.clone(),
					reason: error,
				}),
				Report::Done(result) => {
					outcome = Some(result);
					break;
				}
			}
		}
		let _ = worker.await;
		if cancel.load(Ordering::Relaxed) {
			let _ = tokio::fs::remove_file(&partial).await;
			return Err(JobError::Interrupted);
		}
		if let Some(Err(error)) = outcome {
			let _ = tokio::fs::remove_file(&partial).await;
			return Err(JobError::execution(format!(
				"could not write the archive: {error}"
			)));
		}

		tokio::fs::rename(&partial, &destination)
			.await
			.map_err(|error| {
				JobError::execution(format!("could not finish the archive: {error}"))
			})?;
		let subject = tokio::fs::symlink_metadata(&destination).await.ok();
		ctx.record(vec![Effect::created(destination.clone(), subject.as_ref())])
			.await;

		let mut sources_removed = 0;
		if self.input.remove_sources && left.is_empty() {
			for source in &self.input.sources {
				let Some(path) = source.as_local_path() else {
					continue;
				};
				match trash::trash(path, ctx.volume_manager().as_deref(), ctx.id()).await {
					Ok(location) => {
						let subject = match &location {
							Some(location) => tokio::fs::symlink_metadata(location).await.ok(),
							None => None,
						};
						ctx.record(vec![Effect::trashed(
							path.to_path_buf(),
							location,
							subject.as_ref(),
						)])
						.await;
						sources_removed += 1;
					}
					Err(error) => ctx.log(format!("Could not remove {}: {error}", path.display())),
				}
			}
		} else if self.input.remove_sources {
			ctx.log("Sources kept: not every file made it into the archive");
		}

		ctx.progress(Progress::generic(
			GenericProgress::new(1.0, "Complete", format!("{files} files archived"))
				.with_completion(total_files, total_files)
				.with_bytes(bytes, bytes)
				.with_performance(0.0, None, Some(self.started_at.elapsed()))
				.with_errors(left.len() as u64, 0),
		));
		ctx.log(format!(
			"Archive completed: {files} files, {bytes} bytes, {} left out, {sources_removed} sources removed",
			left.len()
		));
		Ok(ArchiveOutput {
			archive: destination,
			files,
			bytes,
			left,
			sources_removed,
		})
	}
}

fn partial_name(destination: &Path) -> PathBuf {
	let name = destination
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	destination.with_file_name(format!(".{name}.{PARTIAL}"))
}

/// Every file and folder beneath a source, named from the source's own
/// name down, in sorted order.
async fn gather(
	source: &Path,
	members: &mut Vec<Member>,
	left: &mut Vec<ArchiveProblem>,
	format: ArchiveFormat,
) {
	let base = source
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let mut stack = vec![(source.to_path_buf(), base)];
	while let Some((path, name)) = stack.pop() {
		let Ok(meta) = tokio::fs::symlink_metadata(&path).await else {
			left.push(ArchiveProblem {
				path,
				reason: "not there".to_string(),
			});
			continue;
		};
		if meta.file_type().is_symlink() {
			if format == ArchiveFormat::Zip {
				left.push(ArchiveProblem {
					path,
					reason: "a zip does not carry a symlink".to_string(),
				});
				continue;
			}
			members.push(Member {
				path,
				name,
				is_dir: false,
				size: 0,
			});
			continue;
		}
		if meta.is_dir() {
			members.push(Member {
				path: path.clone(),
				name: name.clone(),
				is_dir: true,
				size: 0,
			});
			let mut children = Vec::new();
			if let Ok(mut entries) = tokio::fs::read_dir(&path).await {
				while let Ok(Some(entry)) = entries.next_entry().await {
					children.push(entry.file_name());
				}
			}
			children.sort();
			for child in children.into_iter().rev() {
				let child_name = child.to_string_lossy().into_owned();
				stack.push((path.join(&child), format!("{name}/{child_name}")));
			}
			continue;
		}
		members.push(Member {
			path,
			name,
			is_dir: false,
			size: meta.len(),
		});
	}
}

/// Write every member, reporting each.
fn write_archive(
	partial: &Path,
	format: ArchiveFormat,
	members: &[(PathBuf, String, bool)],
	cancel: &AtomicBool,
	report: &mpsc::UnboundedSender<Report>,
) -> io::Result<()> {
	let file = std::fs::File::create(partial)?;
	match format {
		ArchiveFormat::Zip => {
			let mut writer = zip::ZipWriter::new(file);
			for (index, (path, name, is_dir)) in members.iter().enumerate() {
				if cancel.load(Ordering::Relaxed) {
					return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
				}
				let written = if *is_dir {
					writer
						.add_directory(name, zip_options(path))
						.map(|()| 0)
						.map_err(io::Error::other)
				} else {
					writer
						.start_file(name, zip_options(path))
						.map_err(io::Error::other)
						.and_then(|()| {
							let mut source = std::fs::File::open(path)?;
							io::copy(&mut source, &mut writer)
						})
				};
				match written {
					Ok(bytes) => {
						let _ = report.send(Report::Entry { index, bytes });
					}
					Err(error) => {
						let _ = report.send(Report::Failed {
							index,
							error: error.to_string(),
						});
					}
				}
			}
			writer.finish().map_err(io::Error::other)?;
		}
		ArchiveFormat::TarZstd => {
			let encoder = zstd::stream::write::Encoder::new(file, 3)?;
			let mut builder = tar::Builder::new(encoder);
			builder.follow_symlinks(false);
			for (index, (path, name, is_dir)) in members.iter().enumerate() {
				if cancel.load(Ordering::Relaxed) {
					return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
				}
				let written = if *is_dir {
					builder.append_dir(name, path).map(|()| 0)
				} else {
					builder.append_path_with_name(path, name).and_then(|()| {
						std::fs::symlink_metadata(path).map(|meta| {
							if meta.file_type().is_symlink() {
								0
							} else {
								meta.len()
							}
						})
					})
				};
				match written {
					Ok(bytes) => {
						let _ = report.send(Report::Entry { index, bytes });
					}
					Err(error) => {
						let _ = report.send(Report::Failed {
							index,
							error: error.to_string(),
						});
					}
				}
			}
			let encoder = builder.into_inner()?;
			encoder.finish()?;
		}
	}
	Ok(())
}

/// Deflate, with the file's modification time and unix mode where the
/// platform has them.
fn zip_options(path: &Path) -> zip::write::SimpleFileOptions {
	let mut options = zip::write::SimpleFileOptions::default()
		.compression_method(zip::CompressionMethod::Deflated)
		.large_file(true);
	if let Ok(meta) = std::fs::symlink_metadata(path) {
		if let Ok(modified) = meta.modified() {
			let when: chrono::DateTime<chrono::Local> = modified.into();
			use chrono::{Datelike, Timelike};
			if let Ok(time) = zip::DateTime::from_date_and_time(
				when.year() as u16,
				when.month() as u8,
				when.day() as u8,
				when.hour() as u8,
				when.minute() as u8,
				when.second() as u8,
			) {
				options = options.last_modified_time(time);
			}
		}
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			options = options.unix_permissions(meta.permissions().mode());
		}
	}
	options
}

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ExtractJob {
	pub input: FileExtractInput,
	/// The entry index the job reached, which a resumed job continues from.
	pub next: usize,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl ExtractJob {
	pub fn new(input: FileExtractInput) -> Self {
		Self {
			input,
			next: 0,
			started_at: Instant::now(),
		}
	}
}

impl Job for ExtractJob {
	const NAME: &'static str = "extract_archive";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Extract an archive");
}

impl crate::infra::job::traits::DynJob for ExtractJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn dedup_key(&self) -> Option<String> {
		Some(format!(
			"{} -> {}",
			self.input.archive, self.input.destination
		))
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ExtractOutput {
	pub extracted: u64,
	pub replaced: u64,
	pub skipped: u64,
	pub conflicts: u64,
	pub bytes: u64,
	pub failed: Vec<ArchiveProblem>,
}

impl From<ExtractOutput> for JobOutput {
	fn from(output: ExtractOutput) -> Self {
		JobOutput::custom(output)
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct Resume {
	next: usize,
}

/// One entry the worker writes, at its final path.
struct Write {
	index: usize,
	path: PathBuf,
	is_dir: bool,
}

#[async_trait::async_trait]
impl JobHandler for ExtractJob {
	type Output = ExtractOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if let Some(resume) = ctx.load_state::<Resume>().await? {
			self.next = resume.next;
			ctx.log(format!("Resuming at entry {}", self.next));
		}
		let (Some(archive), Some(destination)) = (
			self.input.archive.as_local_path().map(Path::to_path_buf),
			self.input
				.destination
				.as_local_path()
				.map(Path::to_path_buf),
		) else {
			return Err(JobError::execution(
				"the archive and the destination must be on this device",
			));
		};
		ctx.progress(Progress::Indeterminate("Reading the archive".to_string()));
		let (format, entries) = read_directory(&archive)
			.await
			.map_err(JobError::execution)?;
		let (decisions, escapes) = decide(&self.input, &destination, entries).await;
		if escapes > 0 {
			return Err(JobError::execution(format!(
				"{escapes} entries would land outside the destination"
			)));
		}
		let total_bytes: u64 = decisions
			.iter()
			.filter_map(|decision| match decision.change {
				ChangeKind::Create { size } => Some(size),
				ChangeKind::Replace { incoming_size, .. } => Some(incoming_size),
				_ => None,
			})
			.sum();
		let mut output = ExtractOutput::default();

		// The places, settled before anything is written: numbered names
		// for what is kept beside an existing file, and the previous bytes
		// of what is replaced set aside in the trash.
		let mut writes: Vec<Write> = Vec::new();
		let mut previous: std::collections::HashMap<usize, Option<PathBuf>> =
			std::collections::HashMap::new();
		let mut claimed: HashSet<PathBuf> = HashSet::new();
		for decision in &decisions {
			let Some(path) = &decision.path else {
				continue;
			};
			match &decision.change {
				ChangeKind::Skip { .. } => output.skipped += 1,
				ChangeKind::Conflict { .. } => output.conflicts += 1,
				ChangeKind::MergeInto => {}
				ChangeKind::CreateDirectory => writes.push(Write {
					index: decision.entry.index,
					path: path.clone(),
					is_dir: true,
				}),
				ChangeKind::Create { .. } => {
					let final_path = if tokio::fs::symlink_metadata(path).await.is_ok()
						&& self.input.on_conflict == MergeConflictPolicy::KeepBoth
					{
						let numbered = crate::ops::files::merge::job::unique_name(path).await;
						claimed.insert(numbered.clone());
						numbered
					} else {
						path.clone()
					};
					writes.push(Write {
						index: decision.entry.index,
						path: final_path,
						is_dir: false,
					});
				}
				ChangeKind::Replace { .. } => {
					if decision.entry.index >= self.next {
						match trash::trash(path, ctx.volume_manager().as_deref(), ctx.id()).await {
							Ok(location) => {
								previous.insert(decision.entry.index, location);
							}
							Err(error) => {
								output.failed.push(ArchiveProblem {
									path: path.clone(),
									reason: format!(
										"could not set the previous file aside: {error}"
									),
								});
								continue;
							}
						}
					}
					writes.push(Write {
						index: decision.entry.index,
						path: path.clone(),
						is_dir: false,
					});
				}
				_ => {}
			}
		}
		let pending: Vec<Write> = writes
			.into_iter()
			.filter(|write| write.index >= self.next)
			.collect();
		let total_files = pending.iter().filter(|write| !write.is_dir).count() as u64;
		ctx.log(format!(
			"{total_files} entries to write from entry {}",
			self.next
		));
		let by_index: std::collections::HashMap<usize, (PathBuf, bool)> = pending
			.iter()
			.map(|write| (write.index, (write.path.clone(), write.is_dir)))
			.collect();

		let (report_tx, mut report_rx) = mpsc::unbounded_channel();
		let cancel = Arc::new(AtomicBool::new(false));
		let worker = {
			let archive = archive.clone();
			let cancel = cancel.clone();
			let plan: Vec<(usize, PathBuf, bool)> = pending
				.iter()
				.map(|write| (write.index, write.path.clone(), write.is_dir))
				.collect();
			tokio::task::spawn_blocking(move || {
				let result = extract_entries(&archive, format, &plan, &cancel, &report_tx);
				let _ = report_tx.send(Report::Done(result.map_err(|error| error.to_string())));
			})
		};

		let mut since_checkpoint = 0;
		let mut outcome = None;
		while let Some(report) = report_rx.recv().await {
			if ctx.check_interrupt().await.is_err() {
				cancel.store(true, Ordering::Relaxed);
			}
			match report {
				Report::Entry { index, bytes } => {
					let Some((path, is_dir)) = by_index.get(&index) else {
						continue;
					};
					let subject = tokio::fs::symlink_metadata(path).await.ok();
					let effect = match previous.remove(&index) {
						Some(location) => {
							output.replaced += 1;
							Effect::replaced(path.clone(), location, subject.as_ref())
						}
						None => {
							if !is_dir {
								output.extracted += 1;
							}
							Effect::created(path.clone(), subject.as_ref())
						}
					};
					ctx.record(vec![effect]).await;
					output.bytes += bytes;
					self.next = index + 1;
					since_checkpoint += 1;
					if since_checkpoint >= CHECKPOINT_EVERY {
						since_checkpoint = 0;
						ctx.checkpoint_with_state(&Resume { next: self.next })
							.await?;
					}
					ctx.progress(Progress::generic(
						GenericProgress::new(
							(output.bytes as f32 / total_bytes.max(1) as f32).min(1.0),
							"Extracting",
							format!(
								"{} of {total_files} files",
								output.extracted + output.replaced
							),
						)
						.with_completion(output.extracted + output.replaced, total_files)
						.with_bytes(output.bytes, total_bytes),
					));
				}
				Report::Failed { index, error } => {
					let path = by_index
						.get(&index)
						.map(|(path, _)| path.clone())
						.unwrap_or_default();
					if let Some(Some(location)) = previous.remove(&index) {
						if let Err(restore_error) = trash::restore(&location, &path).await {
							ctx.log(format!(
								"Could not put {} back: {restore_error}",
								path.display()
							));
						}
					}
					output.failed.push(ArchiveProblem {
						path,
						reason: error,
					});
				}
				Report::Done(result) => {
					outcome = Some(result);
					break;
				}
			}
		}
		let _ = worker.await;
		if cancel.load(Ordering::Relaxed) {
			ctx.checkpoint_with_state(&Resume { next: self.next })
				.await?;
			return Err(JobError::Interrupted);
		}
		if let Some(Err(error)) = outcome {
			return Err(JobError::execution(format!(
				"could not read the archive: {error}"
			)));
		}

		ctx.progress(Progress::generic(
			GenericProgress::new(
				1.0,
				"Complete",
				format!(
					"{} extracted, {} replaced",
					output.extracted, output.replaced
				),
			)
			.with_completion(total_files, total_files)
			.with_bytes(output.bytes, output.bytes)
			.with_performance(0.0, None, Some(self.started_at.elapsed()))
			.with_errors(output.failed.len() as u64, output.conflicts),
		));
		ctx.log(format!(
			"Extract completed: {} extracted, {} replaced, {} skipped, {} conflicts, {} failed",
			output.extracted,
			output.replaced,
			output.skipped,
			output.conflicts,
			output.failed.len()
		));
		Ok(output)
	}
}

/// Write the planned entries, reporting each.
fn extract_entries(
	archive: &Path,
	format: ArchiveFormat,
	plan: &[(usize, PathBuf, bool)],
	cancel: &AtomicBool,
	report: &mpsc::UnboundedSender<Report>,
) -> io::Result<()> {
	let wanted: std::collections::HashMap<usize, (&PathBuf, bool)> = plan
		.iter()
		.map(|(index, path, is_dir)| (*index, (path, *is_dir)))
		.collect();
	match format {
		ArchiveFormat::Zip => {
			let file = std::fs::File::open(archive)?;
			let mut zip = zip::ZipArchive::new(file).map_err(io::Error::other)?;
			for (index, path, is_dir) in plan {
				if cancel.load(Ordering::Relaxed) {
					return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
				}
				let written = if *is_dir {
					std::fs::create_dir_all(path).map(|()| 0)
				} else {
					zip.by_index(*index)
						.map_err(io::Error::other)
						.and_then(|mut entry| {
							if let Some(parent) = path.parent() {
								std::fs::create_dir_all(parent)?;
							}
							let mut out = std::fs::File::create(path)?;
							let bytes = io::copy(&mut entry, &mut out)?;
							if let Some(time) = entry.last_modified().and_then(zip_system_time) {
								let _ = out.set_modified(time);
							}
							Ok(bytes)
						})
				};
				let _ = report.send(match written {
					Ok(bytes) => Report::Entry {
						index: *index,
						bytes,
					},
					Err(error) => Report::Failed {
						index: *index,
						error: error.to_string(),
					},
				});
			}
		}
		ArchiveFormat::TarZstd => {
			let file = std::fs::File::open(archive)?;
			let decoder = zstd::stream::read::Decoder::new(file)?;
			let mut tar = tar::Archive::new(decoder);
			for (index, entry) in tar.entries()?.enumerate() {
				if cancel.load(Ordering::Relaxed) {
					return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
				}
				let mut entry = entry?;
				let Some((path, is_dir)) = wanted.get(&index) else {
					continue;
				};
				let written = if *is_dir {
					std::fs::create_dir_all(path).map(|()| 0)
				} else {
					(|| {
						if let Some(parent) = path.parent() {
							std::fs::create_dir_all(parent)?;
						}
						entry.unpack(path)?;
						Ok::<u64, io::Error>(entry.header().size().unwrap_or(0))
					})()
				};
				let _ = report.send(match written {
					Ok(bytes) => Report::Entry { index, bytes },
					Err(error) => Report::Failed {
						index,
						error: error.to_string(),
					},
				});
			}
		}
	}
	Ok(())
}

fn zip_system_time(time: zip::DateTime) -> Option<std::time::SystemTime> {
	let date = chrono::NaiveDate::from_ymd_opt(
		i32::from(time.year()),
		u32::from(time.month()),
		u32::from(time.day()),
	)?;
	let when = date.and_hms_opt(
		u32::from(time.hour()),
		u32::from(time.minute()),
		u32::from(time.second()),
	)?;
	let local = when.and_local_timezone(chrono::Local).single()?;
	Some(local.into())
}
