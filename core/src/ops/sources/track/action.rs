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
		let root = self.input.path;
		if !root.is_dir() {
			return Err(ActionError::Internal(format!(
				"{} is not a directory",
				root.display()
			)));
		}

		// Anchoring to the volume is what lets the source follow a remount, so
		// it is worth resolving even though a source without one still works.
		let volume = context.volume_manager.volume_for_path(&root).await;
		let whole_volume = volume
			.as_ref()
			.is_some_and(|volume| volume.mount_point == root);
		let anchor = volume.as_ref().map(|volume| VolumeAnchor {
			uuid: volume.id,
			mount_point: volume.mount_point.clone(),
		});

		let id = context
			.ephemeral_cache()
			.register_source(&root, anchor)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to register source: {e}")))?;

		let index = context.ephemeral_cache().create_for_indexing(root.clone());

		let device_slug = crate::device::get_current_device_slug();
		let sd_path = SdPath::Physical {
			device_slug,
			path: root.clone(),
		};

		let mut config = crate::ops::indexing::job::IndexerJobConfig::ephemeral_browse(
			sd_path,
			IndexScope::Recursive,
			whole_volume,
		);
		if self.input.unfiltered {
			config.rule_toggles = RuleToggles::none();
		}

		let mut job = IndexerJob::new(config);
		job.set_ephemeral_index(index);
		if let Some(store) = context.ephemeral_cache().store_for(&root).await {
			job.set_source_store(store);
		}

		// The walk is the long pole and the row already exists, so the action
		// answers now and the job reports against the source it just made.
		let job_id = match library.jobs().dispatch(job).await {
			Ok(handle) => Some(handle.id().0),
			Err(e) => {
				tracing::error!(source = %id, %e, "tracked the source but could not start indexing");
				None
			}
		};

		Ok(TrackSourceOutput {
			id,
			root,
			volume_uuid: volume.map(|volume| volume.id),
			whole_volume,
			job_id,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.track"
	}
}

crate::register_library_action!(TrackSourceAction, "sources.track");

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
