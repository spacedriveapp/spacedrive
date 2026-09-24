//! Whether and how a merge would run.
//!
//! Cheap by design: the roots' shapes come from one stat each, the estimate
//! from the arena's rollups, free space from the volume registry, and the
//! stranded-assertion count from one indexed count per source. Errors refuse
//! the execution; a detached source is one, while its preview still answers
//! from the store, so a merge can be planned against a drive in a drawer.

use super::input::FileMergeInput;
use crate::{
	domain::SdPath,
	infra::action::{
		error::ActionError,
		preflight::{ExecutionFacts, Finding, PreviewContext, Validation},
	},
	ops::files::copy::{
		database::CopyDatabaseQuery, input::CopyMethod, routing::CopyStrategyRouter,
	},
};

pub const REMOTE_ROOT: &str = "merge.remote_root";
pub const DESTINATION_MISSING: &str = "merge.destination_missing";
pub const DESTINATION_NOT_DIRECTORY: &str = "merge.destination_not_directory";
pub const NESTED: &str = "merge.nested";
pub const SOURCE_DETACHED: &str = "merge.source_detached";
pub const SOURCE_NOT_DIRECTORY: &str = "merge.source_not_directory";
pub const UNTRACKED: &str = "merge.untracked";
pub const SPACE: &str = "merge.space";
pub const ASSERTIONS_STRANDED: &str = "merge.assertions_stranded";

pub(super) async fn validate(
	input: &FileMergeInput,
	ctx: &PreviewContext,
) -> Result<Validation, ActionError> {
	let mut findings = Vec::new();
	let mut facts = ExecutionFacts {
		executes_on: ctx.executes_on(),
		..Default::default()
	};

	let Some(destination) = input.destination.as_local_path() else {
		findings.push(
			Finding::error(
				REMOTE_ROOT,
				"the destination is on another device; run the merge there with --device",
			)
			.at(input.destination.clone()),
		);
		return Ok(Validation { findings, facts });
	};
	match tokio::fs::metadata(destination).await {
		Ok(meta) if meta.is_dir() => {}
		Ok(_) => findings.push(
			Finding::error(
				DESTINATION_NOT_DIRECTORY,
				"the destination is a file; a merge writes into an existing directory",
			)
			.at(input.destination.clone()),
		),
		Err(_) => findings.push(
			Finding::error(
				DESTINATION_MISSING,
				"the destination does not exist; a merge writes into an existing directory",
			)
			.at(input.destination.clone()),
		),
	}

	let mut sources = Vec::new();
	for source in &input.sources.paths {
		let Some(path) = source.as_local_path() else {
			findings.push(
				Finding::error(
					REMOTE_ROOT,
					"a source is on another device; run the merge there with --device",
				)
				.at(source.clone()),
			);
			continue;
		};
		if path == destination || path.starts_with(destination) || destination.starts_with(path) {
			findings.push(
				Finding::error(NESTED, "a source and the destination contain one another")
					.at(source.clone()),
			);
		}
		match tokio::fs::metadata(path).await {
			Ok(meta) if meta.is_dir() => {}
			Ok(_) => findings.push(
				Finding::error(SOURCE_NOT_DIRECTORY, "a source is a file, not a folder")
					.at(source.clone()),
			),
			Err(_) => findings.push(
				Finding::error(
					SOURCE_DETACHED,
					"a source is not on this device right now; its preview still answers from its store",
				)
				.at(source.clone()),
			),
		}
		if ctx.reach(source).await.is_empty() {
			findings.push(
				Finding::warning(
					UNTRACKED,
					"a source is outside every tracked source, so the plan cannot be read from the index",
				)
				.at(source.clone()),
			);
		}
		sources.push((source, path));
	}

	// The arena keys its rollups by the volume's spelling of a path.
	let mut spelled = Vec::with_capacity(sources.len());
	for (source, path) in &sources {
		spelled.push(match ctx.volumes().locate_path(path).await {
			Some((_, path)) => SdPath::local(path),
			None => (*source).clone(),
		});
	}
	let estimates = CopyDatabaseQuery::new(ctx.index())
		.get_estimates_for_paths(&spelled)
		.await
		.ok();
	facts.estimated_files = estimates.as_ref().map(|estimate| estimate.file_count);
	facts.estimated_bytes = estimates.as_ref().map(|estimate| estimate.total_size);

	let volume = ctx.volumes().volume_for_path(destination).await;
	if let (Some(volume), Some(estimate)) = (&volume, &estimates) {
		facts.free_space_after = Some(volume.available_space as i64 - estimate.total_size as i64);
		findings.extend(space_finding(estimate.total_size, volume.available_space));
	}

	if input.consume_sources {
		if let Some(destination_volume) = &volume {
			for (source, path) in &sources {
				let same_volume = ctx
					.volumes()
					.volume_for_path(path)
					.await
					.is_some_and(|volume| volume.id == destination_volume.id);
				if same_volume {
					continue;
				}
				let stranded = assertions_beneath(ctx, source).await;
				if stranded > 0 {
					findings.push(
						Finding::warning(
							ASSERTIONS_STRANDED,
							format!(
								"{stranded} tag assertions on records under this source stay behind: across volumes a consuming merge writes new records at the destination"
							),
						)
						.at((*source).clone()),
					);
				}
			}
		}
	}

	if let Some((source, _)) = sources.first() {
		let (_, metadata) = CopyStrategyRouter::select_strategy_with_metadata(
			source,
			&input.destination,
			false,
			&CopyMethod::Auto,
			Some(ctx.volumes()),
		)
		.await;
		facts.strategy = Some(metadata.strategy_name);
	}

	Ok(Validation { findings, facts })
}

/// A warning when the whole of the sources would not fit. The estimate is an
/// upper bound, since the plan's duplicate skips bring it down.
pub(super) fn space_finding(needed: u64, free: u64) -> Option<Finding> {
	(needed > free).then(|| {
		Finding::warning(
			SPACE,
			format!(
				"{needed} bytes to copy against {free} free on the destination volume; the plan's duplicate skips may bring that down"
			),
		)
	})
}

/// How many tag assertions stand on records under a source root.
async fn assertions_beneath(ctx: &PreviewContext, source: &SdPath) -> i64 {
	let mut count = 0;
	for reach in ctx.reach(source).await {
		let Some(db) = ctx.index().read_store(reach.source.id).await else {
			continue;
		};
		count += sd_store::read::assertions_beneath(db.pool(), &reach.scope)
			.await
			.unwrap_or(0);
	}
	count
}
