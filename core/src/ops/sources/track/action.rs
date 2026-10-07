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
	library::{config::AddOverrides, Library},
	ops::indexing::{
		job::{IndexScope, IndexerJob},
		rules::RuleToggles,
		sources::{SourceConfig, StorePlacement},
		VolumeAnchor,
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
	/// What this add changes from the library's defaults under Library
	/// Settings > Adding content. Absent fields take the default; nothing
	/// here writes back to the defaults.
	#[serde(default)]
	pub overrides: AddOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TrackSourceOutput {
	pub id: Uuid,
	pub root: PathBuf,
	pub name: String,
	/// The medium underneath, when Spacedrive tracks one. A source with no
	/// volume still works; it just cannot follow a remount.
	pub volume_uuid: Option<Uuid>,
	/// Whether this root is the whole volume rather than a subtree of one.
	pub whole_volume: bool,
	/// The settings the source was saved with: the library's defaults with
	/// this add's overrides applied.
	pub settings: SourceConfig,
	/// Where the catalog lives on this machine.
	pub store_path: Option<PathBuf>,
	/// Whether the store already existed and was reopened rather than
	/// started empty: the scope was added before and its catalog kept.
	pub catalog_reused: bool,
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
		track_and_index(
			&library,
			&context,
			self.input.path,
			self.input.name,
			&self.input.overrides,
		)
		.await
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
/// easy to forget: registering the volume, resolving the effective settings,
/// seeding from the snapshot, and clearing what the new pass will not revisit.
///
/// Every step that saves state either lands or fails the add. A registration
/// whose volume row, settings or name did not persist is reported as the
/// failure it is rather than as a tracked source, so a retry reuses the same
/// identities instead of repairing a half-saved one.
pub async fn track_and_index(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	root: PathBuf,
	name: Option<String>,
	overrides: &AddOverrides,
) -> Result<TrackSourceOutput, ActionError> {
	if !root.is_dir() {
		return Err(ActionError::Internal(format!(
			"{} is not a directory",
			root.display()
		)));
	}

	// The directory a drive leaves behind at its mount point belongs to the
	// parent filesystem. Registering it would make a second source over the
	// parent volume, and its listing would read as an empty drive.
	if let Some(volume) = context.volume_index().unmounted_volume_at(&root) {
		return Err(ActionError::Internal(format!(
			"{} is the mount point of volume {volume}, which is not mounted",
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

	let settings = library.config().await.settings.adding.resolve(overrides);

	// A store opens in WAL mode, which SQLite does not support over a
	// network filesystem, so a share cannot host its own catalog; its
	// serving daemon keeps it in the library instead.
	if settings.placement == StorePlacement::OnSource {
		let on_network = volume.as_ref().is_some_and(|volume| {
			matches!(volume.mount_type, crate::domain::volume::MountType::Network)
				|| volume.parse_cloud_identity().is_some()
		});
		if on_network {
			return Err(ActionError::Internal(format!(
				"{} is on a network volume, which cannot hold its own store; add it with the store in the library",
				root.display()
			)));
		}
	}

	// A source always has its volume: the anchor is only as durable as the
	// volume row it points at, and the row is what lets the registry resolve
	// this source's absolute root on every later boot. Adding a path inside
	// a volume therefore tracks that volume in the library, once, and a
	// second folder on the same drive finds the row already there. The
	// index maps the drive too, so its mount point check and detached flag
	// apply to this source from now on.
	if let Some(volume) = &volume {
		context
			.volume_manager
			.ensure_volume_in_db(volume, library)
			.await
			.map_err(|e| {
				ActionError::Internal(format!(
					"could not track volume {} for {}: {e}",
					volume.name,
					root.display()
				))
			})?;
		context.volume_index().track_detected_volume(
			volume.id,
			volume.mount_point.clone(),
			volume.is_mounted,
		);
	}

	// A store this library already wrote for this scope carries its identity
	// with it. Reopening it is what makes removing and re-adding a folder,
	// or plugging in a drive with its catalog on it, continue where the
	// catalog left off instead of starting over beside it.
	let adopt = context
		.volume_index()
		.portable_identity(&root, anchor.as_ref(), settings.placement, library.id())
		.await;

	let (id, existed) = context
		.volume_index()
		.register_source_with(&root, anchor, adopt)
		.await
		.map_err(|e| ActionError::Internal(format!("Failed to register source: {e}")))?;

	// The settings are the source's, not the caller's moment: the watcher
	// reads the capture policy for every later event, so they have to
	// survive with the registration. Re-tracking may widen capture and never
	// narrows it, so a plain re-track cannot silently demote an archival
	// source; narrowing is `sources.update`'s explicit job. Placement is the
	// one setting a re-track never changes: moving a store is relocation,
	// not an add.
	let settings = match context.volume_index().source_config(id) {
		Some(previous) if existed => SourceConfig {
			unfiltered: settings.unfiltered || previous.unfiltered,
			placement: previous.placement,
			..settings
		},
		_ => settings,
	};
	context
		.volume_index()
		.set_source_config(id, settings.clone())
		.await
		.map_err(|e| ActionError::Internal(format!("could not save the source's settings: {e}")))?;
	if let Some(name) = name.filter(|name| !name.trim().is_empty()) {
		context
			.volume_index()
			.set_source_name(id, name)
			.await
			.map_err(|e| ActionError::Internal(format!("could not save the source's name: {e}")))?;
	}
	let name = context.volume_index().source_name(id).unwrap_or_default();

	let store_path = context.volume_index().store_dir(id);
	let catalog_reused = store_path
		.as_ref()
		.is_some_and(|dir| dir.join("data.db").exists());
	context
		.volume_index()
		.write_descriptor(id, library.id())
		.await
		.map_err(|e| ActionError::Internal(e.to_string()))?;

	let job_id = dispatch_source_walk(library, context, id, root.clone(), whole_volume, true).await;

	Ok(TrackSourceOutput {
		id,
		root,
		name,
		volume_uuid: volume.map(|volume| volume.id),
		whole_volume,
		settings,
		store_path,
		catalog_reused,
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
	// Whatever the stored state says, the directory left behind by an
	// unmounted drive is never walked as the drive.
	if let Some(reason) = context.volume_index().dispatch_refusal(&root) {
		tracing::warn!(source = %id, %reason, "not walking the source");
		return None;
	}

	let source_config = context.volume_index().source_config(id).unwrap_or_default();
	let unfiltered = source_config.unfiltered;

	// Seed the partition from its snapshot before walking over it. A partition
	// that skipped restore is barred from saving over an existing snapshot, so
	// tracking a root that already has one would index and then fail to persist.
	context.volume_index().ensure_restored(&root).await;

	let index = context.volume_index().create_for_indexing(root.clone());

	// Entries from a previous pass that this one will not revisit would
	// otherwise linger in the arena as files that no longer exist.
	let cleared = context.volume_index().clear_for_reindex(&root).await;
	if cleared > 0 {
		tracing::debug!(source = %id, cleared, "cleared stale entries before re-indexing");
	}

	let sd_path = SdPath::Physical {
		device_slug: crate::device::get_current_device_slug(),
		path: root.clone(),
	};

	let mut config = crate::ops::indexing::job::IndexerJobConfig::new(
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
	job.set_arena(index);
	if let Some(store) = context.volume_index().store_for(&root).await {
		job.set_source_store(store);
	}

	// The walk is the long pole and the row already exists, so this answers now
	// and the job reports against the source it just made.
	let handle = match library.jobs().dispatch(job).await {
		Ok(handle) => handle,
		Err(e) => {
			tracing::error!(source = %id, %e, "tracked the source but could not start indexing");
			return None;
		}
	};
	let job_id = handle.id().0;

	// Hashing reads the records the walk writes, so it starts when the walk
	// completes. The job queue runs a LOW job as soon as a worker is free, and
	// one dispatched beside the walk finds an empty store and finishes. A walk
	// that fails or is cancelled leaves hashing to the next track. A source
	// added without content identification gets its walk and nothing after.
	if !source_config.identify_content {
		return Some(job_id);
	}
	let library = library.clone();
	tokio::spawn(async move {
		if handle.wait().await.is_err() {
			return;
		}
		if let Err(e) = library
			.jobs()
			.dispatch_with_priority(
				crate::ops::indexing::content_identity::ContentIdentityJob::new(root),
				crate::infra::job::types::JobPriority::LOW,
				None,
			)
			.await
		{
			tracing::warn!(source = %id, %e, "could not start content identification");
		}
	});

	Some(job_id)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(path: &str) -> TrackSourceInput {
		TrackSourceInput {
			path: PathBuf::from(path),
			name: None,
			overrides: AddOverrides::default(),
		}
	}

	/// The wire shape stays what the CLI and the modal send today: a path
	/// and an optional name, with every override optional on top.
	#[test]
	fn an_input_without_overrides_takes_the_defaults() {
		let input: TrackSourceInput =
			serde_json::from_str(r#"{"path":"/Volumes/Archive","name":null}"#).unwrap();
		assert_eq!(input.overrides, AddOverrides::default());

		let input: TrackSourceInput = serde_json::from_str(
			r#"{"path":"/Volumes/Archive","name":"Archive","overrides":{"placement":"on_source","unfiltered":true}}"#,
		)
		.unwrap();
		assert_eq!(input.overrides.placement, Some(StorePlacement::OnSource));
		assert_eq!(input.overrides.unfiltered, Some(true));
		assert_eq!(input.overrides.keep_offline_copy, None);
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
