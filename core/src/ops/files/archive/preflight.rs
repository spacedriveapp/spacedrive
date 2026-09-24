//! Whether an archive can be written or extracted, and what would exist
//! after.
//!
//! An archive's plan is one `Create` with the sources' byte estimate, and
//! the sources as `Delete` rows when they are to be removed. An extract's
//! plan is read from the archive's own directory and decided against the
//! live destination: a `Create`, `Replace` or `Skip` per entry, a conflict
//! where an entry meets a folder or escapes the destination, which
//! validation refuses.

use std::path::Path;

use super::{
	action::{FileArchiveAction, FileExtractAction},
	directory::{self, ArchiveEntry},
	input::{ArchiveFormat, FileArchiveInput, FileExtractInput},
	plan::decide,
};
use crate::{
	domain::SdPath,
	infra::action::{
		error::ActionError,
		preflight::{
			ExecutionFacts, Finding, PreviewContext, PreviewableAction, ValidatedAction, Validation,
		},
	},
	ops::files::{
		copy::database::CopyDatabaseQuery,
		delete::last_copies,
		plan::{
			ChangeKind, ConflictKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlannedChange,
		},
	},
};

pub const REMOTE_ROOT: &str = "archive.remote_root";
pub const SOURCE_MISSING: &str = "archive.source_missing";
pub const EXISTS: &str = "archive.exists";
pub const DESTINATION_MISSING: &str = "archive.destination_missing";
pub const UNSUPPORTED: &str = "archive.unsupported";
pub const SPACE: &str = "archive.space";
pub const LAST_COPY: &str = "archive.last_copy";
pub const UNREADABLE: &str = "extract.unreadable";
pub const ESCAPE: &str = "extract.escape";
pub const EXTRACT_DESTINATION: &str = "extract.destination";
pub const EXTRACT_SPACE: &str = "extract.space";
pub const CONFLICTS: &str = "extract.conflicts";

impl ValidatedAction for FileArchiveAction {
	async fn validate(
		input: &FileArchiveInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		let mut findings = Vec::new();
		let mut facts = ExecutionFacts {
			executes_on: ctx.executes_on(),
			strategy: Some(input.format.extension().to_string()),
			..Default::default()
		};
		let Some(destination) = input.destination.as_local_path() else {
			findings.push(
				Finding::error(
					REMOTE_ROOT,
					"the archive would be written on another device",
				)
				.at(input.destination.clone()),
			);
			return Ok(Validation { findings, facts });
		};
		if tokio::fs::symlink_metadata(destination).await.is_ok() {
			findings.push(
				Finding::error(EXISTS, "a file is already at the archive's name")
					.at(input.destination.clone()),
			);
		}
		if !destination.parent().is_some_and(|parent| parent.is_dir()) {
			findings.push(
				Finding::error(
					DESTINATION_MISSING,
					"the folder the archive would be written in does not exist",
				)
				.at(input.destination.clone()),
			);
		}
		let mut local = Vec::new();
		for source in &input.sources {
			let Some(path) = source.as_local_path() else {
				findings.push(
					Finding::error(REMOTE_ROOT, "a source is on another device").at(source.clone()),
				);
				continue;
			};
			match tokio::fs::symlink_metadata(path).await {
				Ok(meta) => {
					if meta.file_type().is_symlink() && input.format == ArchiveFormat::Zip {
						findings.push(
							Finding::warning(
								UNSUPPORTED,
								"a zip does not carry a symlink; it is left out",
							)
							.at(source.clone()),
						);
					}
					local.push(source.clone());
				}
				Err(_) => findings.push(
					Finding::error(SOURCE_MISSING, "a source is not there").at(source.clone()),
				),
			}
		}

		let estimate = estimate(ctx, &local).await;
		facts.estimated_files = Some(estimate.0);
		facts.estimated_bytes = Some(estimate.1);
		if let Some(volume) = ctx.volumes().volume_for_path(destination).await {
			facts.free_space_after = Some(volume.available_space as i64 - estimate.1 as i64);
			if estimate.1 > volume.available_space {
				findings.push(Finding::warning(
					SPACE,
					format!(
						"{} bytes to archive against {} free on the destination volume; compression may bring that down",
						estimate.1, volume.available_space
					),
				));
			}
		}
		if input.remove_sources {
			let stranded = last_copies_among(ctx, &local).await;
			if stranded > 0 {
				findings.push(Finding::warning(
					LAST_COPY,
					format!(
						"{stranded} of the sources are the last copy of their content anywhere in your library; the archive becomes the only copy"
					),
				));
			}
		}
		Ok(Validation { findings, facts })
	}
}

impl PreviewableAction for FileArchiveAction {
	type Plan = FsPlan;

	async fn preview(input: FileArchiveInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		let device = crate::device::get_current_device_slug();
		let mut summary = FsPlanSummary::default();
		let mut changes = PlanChanges::default();
		let (_, bytes) = estimate(ctx, &input.sources).await;
		let create = ChangeKind::Create { size: bytes };
		summary.count(&create, bytes);
		changes.push(PlannedChange {
			path: input.destination.clone(),
			change: create,
		});
		if input.remove_sources {
			for source in &input.sources {
				let Some(path) = source.as_local_path() else {
					continue;
				};
				let size = tokio::fs::symlink_metadata(path)
					.await
					.map(|meta| if meta.is_dir() { 0 } else { meta.len() })
					.unwrap_or(0);
				let change = ChangeKind::Delete { last_copy: false };
				summary.count(&change, size);
				changes.push(PlannedChange {
					path: SdPath::Physical {
						device_slug: device.clone(),
						path: path.to_path_buf(),
					},
					change,
				});
			}
		}
		let (changes, truncated) = changes.finish();
		Ok(ctx.plans().retain(FsPlan {
			handle: None,
			basis: PlanBasis::Index {
				revisions: Vec::new(),
			},
			roots: Vec::new(),
			summary,
			changes,
			truncated,
		}))
	}
}

impl ValidatedAction for FileExtractAction {
	async fn validate(
		input: &FileExtractInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		let mut findings = Vec::new();
		let mut facts = ExecutionFacts {
			executes_on: ctx.executes_on(),
			..Default::default()
		};
		let (Some(archive), Some(destination)) = (
			input.archive.as_local_path(),
			input.destination.as_local_path(),
		) else {
			findings.push(Finding::error(
				REMOTE_ROOT,
				"the archive or the destination is on another device",
			));
			return Ok(Validation { findings, facts });
		};
		if !destination.is_dir() {
			findings.push(
				Finding::error(
					EXTRACT_DESTINATION,
					"the destination does not exist; an extract writes into an existing folder",
				)
				.at(input.destination.clone()),
			);
		}
		let entries = match read_directory(archive).await {
			Ok((format, entries)) => {
				facts.strategy = Some(format.extension().to_string());
				entries
			}
			Err(error) => {
				findings.push(
					Finding::error(UNREADABLE, format!("the archive cannot be read: {error}"))
						.at(input.archive.clone()),
				);
				return Ok(Validation { findings, facts });
			}
		};
		let (decisions, escapes) = decide(input, destination, entries).await;
		if escapes > 0 {
			findings.push(
				Finding::error(
					ESCAPE,
					format!(
						"{escapes} entries would land outside the destination, or strip to nothing"
					),
				)
				.at(input.archive.clone()),
			);
		}
		let conflicts = decisions
			.iter()
			.filter(|decision| {
				matches!(
					decision.change,
					ChangeKind::Conflict {
						kind: ConflictKind::FileVsDirectory
					}
				)
			})
			.count();
		if conflicts > 0 {
			findings.push(Finding::warning(
				CONFLICTS,
				format!("{conflicts} entries meet a folder where a file goes, or a file where a folder goes; they are left"),
			));
		}
		let bytes: u64 = decisions
			.iter()
			.filter_map(|decision| match decision.change {
				ChangeKind::Create { size } => Some(size),
				ChangeKind::Replace { incoming_size, .. } => Some(incoming_size),
				_ => None,
			})
			.sum();
		facts.estimated_files = Some(
			decisions
				.iter()
				.filter(|decision| {
					matches!(
						decision.change,
						ChangeKind::Create { .. } | ChangeKind::Replace { .. }
					)
				})
				.count() as u64,
		);
		facts.estimated_bytes = Some(bytes);
		if let Some(volume) = ctx.volumes().volume_for_path(destination).await {
			facts.free_space_after = Some(volume.available_space as i64 - bytes as i64);
			if bytes > volume.available_space {
				findings.push(Finding::warning(
					EXTRACT_SPACE,
					format!(
						"{bytes} bytes to extract against {} free on the destination volume",
						volume.available_space
					),
				));
			}
		}
		Ok(Validation { findings, facts })
	}
}

impl PreviewableAction for FileExtractAction {
	type Plan = FsPlan;

	async fn preview(input: FileExtractInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		let (Some(archive), Some(destination)) = (
			input.archive.as_local_path(),
			input.destination.as_local_path(),
		) else {
			return Err(ActionError::InvalidInput(
				"the archive and the destination must be on this device".into(),
			));
		};
		let (_, entries) = read_directory(archive).await.map_err(|error| {
			ActionError::InvalidInput(format!("the archive cannot be read: {error}"))
		})?;
		let count = entries.len() as u64;
		let (decisions, _) = decide(&input, destination, entries).await;
		let device = crate::device::get_current_device_slug();
		let mut summary = FsPlanSummary::default();
		let mut changes = PlanChanges::default();
		for decision in decisions {
			let path = decision
				.path
				.unwrap_or_else(|| destination.join(&decision.entry.name));
			let bytes = match decision.change {
				ChangeKind::Create { size } => size,
				ChangeKind::Replace { incoming_size, .. } => incoming_size,
				ChangeKind::Skip { .. } => decision.entry.size,
				_ => 0,
			};
			summary.count(&decision.change, bytes);
			changes.push(PlannedChange {
				path: SdPath::Physical {
					device_slug: device.clone(),
					path,
				},
				change: decision.change,
			});
		}
		let (changes, truncated) = changes.finish();
		Ok(ctx.plans().retain(FsPlan {
			handle: None,
			basis: PlanBasis::Archive {
				path: input.archive.clone(),
				entries: count,
			},
			roots: Vec::new(),
			summary,
			changes,
			truncated,
		}))
	}
}

crate::register_validate!(FileArchiveAction, "files.archive");
crate::register_preview!(FileArchiveAction, "files.archive");
crate::register_validate!(FileExtractAction, "files.extract");
crate::register_preview!(FileExtractAction, "files.extract");

/// The archive's format from its name, and its directory.
pub(super) async fn read_directory(
	archive: &Path,
) -> Result<(ArchiveFormat, Vec<ArchiveEntry>), String> {
	let name = archive
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let format = ArchiveFormat::of_name(&name)
		.ok_or_else(|| "only .zip and .tar.zst are read".to_string())?;
	let path = archive.to_path_buf();
	let entries = tokio::task::spawn_blocking(move || directory::read(&path, format))
		.await
		.map_err(|error| error.to_string())?
		.map_err(|error| error.to_string())?;
	Ok((format, entries))
}

/// Files and bytes beneath the sources, from the index where it has them,
/// else from the disk.
async fn estimate(ctx: &PreviewContext, sources: &[SdPath]) -> (u64, u64) {
	let mut spelled = Vec::with_capacity(sources.len());
	for source in sources {
		let Some(path) = source.as_local_path() else {
			continue;
		};
		spelled.push(match ctx.volumes().locate_path(path).await {
			Some((_, path)) => SdPath::local(path),
			None => source.clone(),
		});
	}
	if let Ok(estimates) = CopyDatabaseQuery::new(ctx.index())
		.get_estimates_for_paths(&spelled)
		.await
	{
		if estimates.is_complete() {
			return (estimates.file_count, estimates.total_size);
		}
	}
	let mut files = 0;
	let mut bytes = 0;
	for source in sources {
		let Some(path) = source.as_local_path() else {
			continue;
		};
		walk(path, &mut files, &mut bytes).await;
	}
	(files, bytes)
}

async fn walk(path: &Path, files: &mut u64, bytes: &mut u64) {
	let mut stack = vec![path.to_path_buf()];
	while let Some(current) = stack.pop() {
		let Ok(meta) = tokio::fs::symlink_metadata(&current).await else {
			continue;
		};
		if meta.is_dir() {
			if let Ok(mut entries) = tokio::fs::read_dir(&current).await {
				while let Ok(Some(entry)) = entries.next_entry().await {
					stack.push(entry.path());
				}
			}
		} else if meta.is_file() {
			*files += 1;
			*bytes += meta.len();
		}
	}
}

/// How many of the sources' files are the last copy of their content.
async fn last_copies_among(ctx: &PreviewContext, sources: &[SdPath]) -> u64 {
	let mut hashes = Vec::new();
	for source in sources {
		for reach in ctx.reach(source).await {
			let Some(db) = ctx.index().read_store(reach.source.id).await else {
				continue;
			};
			if reach.prefix.is_empty() {
				if let Ok(Some(entry)) =
					sd_store::read::entry_by_path(db.pool(), &reach.scope).await
				{
					if entry.kind == sd_store::FileKind::File {
						hashes.extend(entry.sampled_hash);
						continue;
					}
				}
			}
			let mut cursor: Option<(String, String)> = None;
			loop {
				let batch = {
					let start = match &cursor {
						Some((directory, name)) => sd_store::read::Start::After { directory, name },
						None => sd_store::read::Start::First,
					};
					match sd_store::read::files_beneath(
						db.pool(),
						&reach.scope,
						start,
						None,
						true,
						1000,
					)
					.await
					{
						Ok(batch) => batch,
						Err(_) => break,
					}
				};
				let full = batch.len() == 1000;
				for entry in &batch {
					hashes.extend(entry.sampled_hash.clone());
				}
				cursor = batch
					.last()
					.map(|entry| (entry.directory().to_string(), entry.name.clone()));
				if !full {
					break;
				}
			}
		}
	}
	let stranded = last_copies(ctx, hashes.clone()).await;
	hashes
		.iter()
		.filter(|hash| stranded.contains(*hash))
		.count() as u64
}
