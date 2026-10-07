//! Walk a drive into its volume index.
//!
//! The arena is the machine's filesystem map, and until something walks a drive
//! end to end that map only holds what a source walk or a directory listing
//! happened to visit. On a stock macOS install `/Applications` had never been
//! enumerated for exactly that reason. This is the pass that fills it in.
//!
//! It runs behind the source walk. What is in the library is what someone chose
//! to keep, so it earns the first pass and the file count people watch go up;
//! the rest of the drive follows at `LOW` for search and for the analyser's
//! totals.
//!
//! Mapping a drive is not registering one. The partition and its snapshot
//! belong to the drive, and neither needs a row in the sources list.

use crate::{
	context::CoreContext,
	device::get_current_device_slug,
	domain::{
		addressing::SdPath,
		volume::{MountType, Volume},
	},
	infra::{
		action::{context::ActionContext, error::ActionError},
		job::{prelude::JobId, types::JobPriority},
	},
	library::Library,
	ops::indexing::{
		job::{IndexScope, IndexerJob, IndexerJobConfig},
		rules::RuleToggles,
		summary::Retention,
	},
};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// How a drive is walked.
pub struct MapOptions {
	pub scope: IndexScope,
	pub priority: JobPriority,
	/// Drop what the previous pass left behind before walking. Re-indexing on
	/// request wants this; a background fill over a live arena does not, since
	/// the entries it would clear are the ones another job just wrote.
	pub reindex: bool,
	/// What the walk keeps. Summarising is the difference between a map that
	/// fits in memory and one that does not.
	pub retention: Retention,
	/// Whether the walk reports on the event bus. A walk someone requested
	/// does; the launch fill of every drive does not.
	pub announce: bool,
}

impl MapOptions {
	/// The background fill: everything under the mount point, behind whatever
	/// else is queued, kept at the fidelity the drive is worth.
	///
	/// `covered` is the roots another walk owns, which this one leaves alone.
	pub fn background(covered: Vec<std::path::PathBuf>) -> Self {
		Self {
			scope: IndexScope::Recursive,
			priority: JobPriority::LOW,
			reindex: false,
			retention: Retention::map(covered),
			announce: false,
		}
	}
}

/// Dispatch a walk of one drive and return the job that will do it.
pub async fn map_volume(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	volume: &Volume,
	options: MapOptions,
	action_context: Option<ActionContext>,
) -> Result<JobId, ActionError> {
	let sd_path = if let Some((service, identifier)) = volume.parse_cloud_identity() {
		SdPath::Cloud {
			service,
			identifier,
			path: String::new(),
		}
	} else {
		SdPath::Physical {
			device_slug: get_current_device_slug(),
			path: volume.mount_point.clone(),
		}
	};

	let cache = context.volume_index();
	let cloud = volume.parse_cloud_identity().is_some();
	if cloud {
		cache.track_volume(volume.id, volume.mount_point.clone());
	} else {
		cache.track_detected_volume(volume.id, volume.mount_point.clone(), volume.is_mounted);
	}
	// Detection can lag an unmount by a refresh interval, and the directory
	// left behind at the mount point is never walked as the drive.
	if !cloud && !crate::volume::utils::is_mount_point(&volume.mount_point) {
		return Err(ActionError::Internal(format!(
			"not walking {}: {} is not a mount point; the volume is not mounted",
			volume.name,
			volume.mount_point.display()
		)));
	}
	if let Some(reason) = cache.dispatch_refusal(&volume.mount_point) {
		return Err(ActionError::Internal(format!(
			"not walking {}: {reason}",
			volume.name
		)));
	}

	// Seed the partition from its snapshot before walking over it: duplicate
	// paths keep their identities, and a partition that skipped restore would
	// be barred from saving over the existing snapshot.
	cache.ensure_restored(&volume.mount_point).await;
	let index = cache.create_for_indexing(volume.mount_point.clone());

	let mut config = IndexerJobConfig::new(sd_path, options.scope, true);
	config.retention = options.retention;
	config.announce = options.announce;
	if volume.mount_type == MountType::External {
		// An archived drive's index must reflect the whole drive: rules are
		// view-time lenses, not walk-time exclusions, for removable media.
		config.rule_toggles = RuleToggles::none();
	}

	let mut job = IndexerJob::new(config);
	job.set_arena(index);
	if let Some(store) = cache.store_for(&volume.mount_point).await {
		job.set_source_store(store);
	}

	if options.reindex {
		let cleared = cache.clear_for_reindex(&volume.mount_point).await;
		if cleared > 0 {
			info!(
				"Cleared {} stale entries before re-indexing {}",
				cleared, volume.name
			);
		}
	}

	let handle = library
		.jobs()
		.dispatch_with_priority(job, options.priority, action_context)
		.await
		.map_err(|e| ActionError::Internal(format!("Failed to dispatch job: {e}")))?;

	Ok(handle.id())
}

/// Re-walk registered sources on this volume whose map coverage is missing.
///
/// The volume map summarises registered source subtrees, and only the
/// source's own walk fills them. An interrupted walk, a lost snapshot, or a
/// nested source whose volume restored around it all leave the same shape: a
/// store that holds records under a root whose node in the map has no
/// children. Store evidence contradicting arena emptiness is the trigger, so
/// a genuinely empty source is never rewalked.
async fn heal_uncovered_sources(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	volume: &crate::domain::Volume,
) {
	let cache = context.volume_index();
	for source in cache.sources() {
		if !source.root.starts_with(&volume.mount_point)
			|| !source.attached
			|| cache.is_indexing(&source.root)
		{
			continue;
		}
		if source.entry_count.unwrap_or(0) == 0 {
			continue;
		}

		let covered = match cache.get_for_search(&source.root) {
			Some(index) => index
				.read()
				.await
				.list_directory(&source.root)
				.is_some_and(|children| !children.is_empty()),
			None => false,
		};
		if covered {
			continue;
		}

		tracing::warn!(
			source = %source.id,
			root = %source.root.display(),
			records = source.entry_count.unwrap_or(0),
			"source has records but no map coverage; dispatching its walk"
		);
		crate::ops::sources::track::action::dispatch_source_walk(
			library,
			context,
			source.id,
			source.root.clone(),
			source.root == volume.mount_point,
			false,
		)
		.await;
	}
}

/// Map every drive attached to this machine.
///
/// Called once the library is open and its sources have been walked. Search and
/// the analyser both need the whole drive, and nothing else asks for it: a
/// source covers the part someone kept, and a directory listing covers the one
/// folder in front of them.
///
/// Without `whole_drives`, each drive is restored and its sources' coverage
/// healed, and nothing else on it is walked.
pub async fn map_attached_volumes(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	whole_drives: bool,
) {
	for volume in context.volume_manager.get_all_volumes().await {
		if !volume.is_mounted || volume.mount_type == MountType::Network {
			continue;
		}
		if volume.parse_cloud_identity().is_some() {
			continue;
		}
		// Volumes the platform hides from users either alias what a visible
		// volume already covers or hold no user files. On macOS the sealed
		// system volume mounts at `/`, and walking it would map the firmlinked
		// data-volume tree a second time under its short spelling.
		if !volume.is_user_visible {
			continue;
		}

		let cache = context.volume_index();
		cache.track_detected_volume(volume.id, volume.mount_point.clone(), volume.is_mounted);
		let restored = cache.ensure_restored(&volume.mount_point).await;

		// Runs whether or not the volume restored: a restored snapshot can
		// faithfully persist a coverage hole, which is exactly the state
		// this repairs.
		heal_uncovered_sources(library, context, &volume).await;

		if !whole_drives {
			continue;
		}
		if restored {
			debug!(
				"{} restored from its snapshot; not walking it again",
				volume.mount_point.display()
			);
			continue;
		}

		// A drive already being walked is either a source that spans it or a
		// map from earlier in this session, and either one covers this.
		if context.volume_index().is_indexing(&volume.mount_point) {
			debug!(
				"{} is already being walked; not mapping it again",
				volume.mount_point.display()
			);
			continue;
		}

		let covered = context
			.volume_index()
			.sources()
			.into_iter()
			.map(|source| source.root)
			.filter(|root| root.starts_with(&volume.mount_point))
			.collect();

		match map_volume(
			library,
			context,
			&volume,
			MapOptions::background(covered),
			None,
		)
		.await
		{
			Ok(job_id) => info!(
				"Mapping {} ({}) in the background as job {job_id}",
				volume.name,
				volume.mount_point.display()
			),
			Err(e) => warn!("Could not map {}: {e}", volume.mount_point.display()),
		}
	}
}
