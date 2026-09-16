//! Track a root as a filesystem source, and start indexing it.
//!
//! Until now a filesystem source appeared as a side effect: browsing a
//! directory dispatched a walk, and the walk registered whatever partition it
//! landed in. This is the intent made explicit, which is what "track this
//! drive" and "track this folder" both come to.
//!
//! Indexing starts here rather than waiting to be asked. Registration is
//! instant and the walk is not, so the action returns as soon as the row
//! exists and the job reports progress against it.
//!
//! ## Against `volumes.track`
//!
//! Different axes, and the names collide on the word rather than the meaning.
//! `volumes.track` marks a *medium* as belonging to a library: it flips
//! `is_tracked` on the volume row and indexes nothing. This makes an *index* of
//! what is on one. A drive can be tracked with nothing indexed, and a root can
//! be indexed on a volume nobody tracked, which is the unanchored case.
//!
//! `volumes.index` is the older overlap: it takes a fingerprint rather than a
//! path and can only ever mean the whole drive. It retires once callers move to
//! this, since a mount point is just a path.

use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::indexing::{
		ephemeral::VolumeAnchor,
		job::{IndexScope, IndexerJob},
		rules::RuleToggles,
	},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TrackSourceInput {
	/// The root to track. A volume's mount point tracks the whole drive; any
	/// path under one tracks that subtree.
	pub path: PathBuf,
	/// Display name, or the directory's own name.
	pub name: Option<String>,
	/// Record everything readable, rather than applying the default rules that
	/// hide system files, `.git` and dev directories. Archival drives want
	/// this; a working directory usually does not.
	#[serde(default)]
	pub unfiltered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TrackSourceOutput {
	pub id: Uuid,
	pub root: PathBuf,
	/// The medium underneath, when Spacedrive tracks one. A source with no
	/// volume still works; it just cannot follow a remount.
	pub volume_uuid: Option<Uuid>,
	/// Whether this root is the whole volume rather than a subtree of one.
	pub whole_volume: bool,
	pub job_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSourceAction {
	input: TrackSourceInput,
}

impl LibraryAction for TrackSourceAction {
	type Input = TrackSourceInput;
	type Output = TrackSourceOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if !input.path.is_absolute() {
			return Err("Source root must be an absolute path".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		track_and_index(&library, &context, self.input.path, self.input.unfiltered).await
	}

	fn action_kind(&self) -> &'static str {
		"sources.track"
	}
}

crate::register_library_action!(TrackSourceAction, "sources.track");

/// Register a root as a source and start walking it.
///
/// Shared by `sources.track` and `volumes.track`, because tracking a drive and
/// tracking a folder on one differ only in how the caller arrived at the path.
/// Keeping one body is what stops the two from drifting on the steps that are
/// easy to forget: seeding from the snapshot, and clearing what the new pass
/// will not revisit.
pub async fn track_and_index(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	root: PathBuf,
	unfiltered: bool,
) -> Result<TrackSourceOutput, ActionError> {
	if !root.is_dir() {
		return Err(ActionError::Internal(format!(
			"{} is not a directory",
			root.display()
		)));
	}

	// Anchoring to the volume is what lets the source follow a remount, so it
	// is worth resolving even though a source without one still works.
	//
	// The volume decides how the path is written. On macOS a home directory
	// reached as `/Users/me` is the same directory as
	// `/System/Volumes/Data/Users/me`, and only the second is under the mount
	// point, so taking the caller's spelling would drop the anchor and start a
	// second map of files the drive already holds.
	let located = context.volume_manager.locate_path(&root).await;
	let root = located
		.as_ref()
		.map(|(_, path)| path.clone())
		.unwrap_or(root);
	let volume = located.map(|(volume, _)| volume);
	let whole_volume = volume
		.as_ref()
		.is_some_and(|volume| volume.mount_point == root);
	let anchor = volume.as_ref().map(|volume| VolumeAnchor {
		uuid: volume.id,
		mount_point: volume.mount_point.clone(),
	});

	// The source's anchor is only as durable as the volume row it points at.
	// Persisting the volume here is what lets the registry resolve this
	// source's absolute root on every later boot; without the row, the next
	// process knows the source only by its relative path.
	if let Some(volume) = &volume {
		if let Err(e) = context
			.volume_manager
			.ensure_volume_in_db(volume, library)
			.await
		{
			tracing::warn!(%e, volume = %volume.name, "could not persist the source's volume");
		}
	}

	let id = context
		.ephemeral_cache()
		.register_source(&root, anchor)
		.await
		.map_err(|e| ActionError::Internal(format!("Failed to register source: {e}")))?;

	// The capture policy is the source's, not the caller's moment: the
	// watcher reads it for every later event, so it has to survive with the
	// registration. Tracking may widen it and never narrows it, so a plain
	// re-track cannot silently demote an archival source; narrowing is
	// `sources.update`'s explicit job.
	let unfiltered = unfiltered
		|| context
			.ephemeral_cache()
			.source_config(id)
			.is_some_and(|config| config.unfiltered);
	context
		.ephemeral_cache()
		.set_source_config(
			id,
			crate::ops::indexing::ephemeral::sources::SourceConfig { unfiltered },
		)
		.await;

	let job_id =
		dispatch_source_walk(&library, &context, id, root.clone(), whole_volume, true).await;

	Ok(TrackSourceOutput {
		id,
		root,
		volume_uuid: volume.map(|volume| volume.id),
		whole_volume,
		job_id,
	})
}

/// Dispatch the walk and the follow-up hashing pass for a registered source.
///
/// Shared by explicit tracking and by the discovery pass's coverage heal, so
/// both honor the persisted capture policy, source retention, and the store
/// wiring the same way. `announce` distinguishes a person tracking a source
/// from a background repair.
pub(crate) async fn dispatch_source_walk(
	library: &Arc<crate::library::Library>,
	context: &Arc<CoreContext>,
	id: Uuid,
	root: PathBuf,
	whole_volume: bool,
	announce: bool,
) -> Option<uuid::Uuid> {
	let unfiltered = context
		.ephemeral_cache()
		.source_config(id)
		.is_some_and(|config| config.unfiltered);

	// Seed the partition from its snapshot before walking over it. A partition
	// that skipped restore is barred from saving over an existing snapshot, so
	// tracking a root that already has one would index and then fail to persist.
	context.ephemeral_cache().ensure_restored(&root).await;

	let index = context.ephemeral_cache().create_for_indexing(root.clone());

	// Entries from a previous pass that this one will not revisit would
	// otherwise linger in the arena as files that no longer exist.
	let cleared = context.ephemeral_cache().clear_for_reindex(&root).await;
	if cleared > 0 {
		tracing::debug!(source = %id, cleared, "cleared stale entries before re-indexing");
	}

	let sd_path = SdPath::Physical {
		device_slug: crate::device::get_current_device_slug(),
		path: root.clone(),
	};

	let mut config = crate::ops::indexing::job::IndexerJobConfig::ephemeral_browse(
		sd_path,
		IndexScope::Recursive,
		whole_volume,
	);
	config.announce = announce;
	if unfiltered {
		config.rule_toggles = RuleToggles::none();
	}
	// What the rules hold back is still on the drive, and a source that reports
	// a size missing its excluded directories is reporting the wrong size.
	config.retention = crate::ops::indexing::summary::Retention::source();

	let mut job = IndexerJob::new(config);
	job.set_ephemeral_index(index);
	if let Some(store) = context.ephemeral_cache().store_for(&root).await {
		job.set_source_store(store);
	}

	// The walk is the long pole and the row already exists, so this answers now
	// and the job reports against the source it just made.
	let job_id = match library.jobs().dispatch(job).await {
		Ok(handle) => Some(handle.id().0),
		Err(e) => {
			tracing::error!(source = %id, %e, "tracked the source but could not start indexing");
			None
		}
	};

	// Behind the walk, because it has nothing to hash until the walk has
	// written the records. Dispatched rather than chained: the job queue runs
	// it when the walk is out of the way, which is what LOW means.
	if let Err(e) = library
		.jobs()
		.dispatch_with_priority(
			crate::ops::indexing::content_identity::ContentIdentityJob::new(root.clone()),
			crate::infra::job::types::JobPriority::LOW,
			None,
		)
		.await
	{
		tracing::warn!(source = %id, %e, "could not start content identification");
	}

	job_id
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(path: &str) -> TrackSourceInput {
		TrackSourceInput {
			path: PathBuf::from(path),
			name: None,
			unfiltered: false,
		}
	}

	#[test]
	fn a_relative_root_is_refused() {
		// A source's root is resolved against a volume mount point, and a
		// relative path has no fixed meaning to resolve.
		assert!(TrackSourceAction::from_input(input("Documents")).is_err());
	}

	#[test]
	fn an_absolute_root_is_accepted() {
		assert!(TrackSourceAction::from_input(input("/Volumes/Archive")).is_ok());
	}
}
