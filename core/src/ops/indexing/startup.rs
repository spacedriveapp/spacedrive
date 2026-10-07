//! # Startup discovery
//!
//! A daemon restores libraries and serves requests without assuming why it was
//! launched. A desktop client starts discovery after its window is visible, and
//! a headless client can request the same defaults explicitly. This keeps macOS
//! permission prompts attached to an interface a person can see.
//!
//! The defaults are the system volume, the home folder and a map of every
//! attached drive. A daemon started with `--no-default-sources` leaves them
//! out, for a library that should hold only what was added to it, and still
//! restores, heals and hashes the sources it has.

use std::{
	collections::HashSet,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, Mutex,
	},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};

/// Prevents several clients from starting the same launch pass, and holds
/// whether this daemon's passes include the default places.
#[derive(Default)]
pub struct StartupIndexingGate {
	started: Mutex<HashSet<Uuid>>,
	skip_default_sources: AtomicBool,
}

impl StartupIndexingGate {
	fn claim(&self, library_id: Uuid) -> bool {
		self.started
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
			.insert(library_id)
	}

	/// Leave the default places out of every launch pass from now on.
	pub fn skip_default_sources(&self) {
		self.skip_default_sources.store(true, Ordering::Relaxed);
	}

	fn includes_default_sources(&self) -> bool {
		!self.skip_default_sources.load(Ordering::Relaxed)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StartupIndexingInput {
	/// Ignore the automatic-start preference for an explicit CLI request.
	#[serde(default)]
	pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StartupIndexingDisposition {
	Started,
	AlreadyStarted,
	Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StartupIndexingOutput {
	pub disposition: StartupIndexingDisposition,
}

pub struct StartupIndexingAction {
	input: StartupIndexingInput,
}

impl LibraryAction for StartupIndexingAction {
	type Input = StartupIndexingInput;
	type Output = StartupIndexingOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		if !self.input.force && !library.config().await.settings.auto_track_system_volumes {
			return Ok(StartupIndexingOutput {
				disposition: StartupIndexingDisposition::Disabled,
			});
		}

		if !context.startup_indexing.claim(library.id()) {
			return Ok(StartupIndexingOutput {
				disposition: StartupIndexingDisposition::AlreadyStarted,
			});
		}

		let defaults = context.startup_indexing.includes_default_sources();
		tokio::spawn(async move {
			info!(
				library = %library.id(),
				defaults,
				"Starting automatic filesystem discovery"
			);

			if defaults {
				if let Err(error) = context
					.volume_manager
					.auto_track_user_volumes(&library)
					.await
				{
					warn!(%error, "Could not track the system volume");
				}
			}

			context.volume_index().restore_everything().await;
			if defaults {
				add_home_to_library(&library, &context).await;
			}
			crate::ops::volumes::index::map_attached_volumes(&library, &context, defaults).await;
			crate::ops::indexing::content_identity::identify_every_source(&library, &context).await;
		});

		Ok(StartupIndexingOutput {
			disposition: StartupIndexingDisposition::Started,
		})
	}

	fn action_kind(&self) -> &'static str {
		"indexing.startup"
	}
}

crate::register_library_action!(StartupIndexingAction, "indexing.startup");

/// Keep a person's home directory when this machine has no attached source.
///
/// A detached source belongs to another device or an unplugged drive and must
/// not prevent this device from becoming useful. Any attached source means the
/// person has already chosen a local scope, so automatic discovery leaves it
/// alone.
async fn add_home_to_library(library: &Arc<Library>, context: &Arc<CoreContext>) {
	if context
		.volume_index()
		.sources_of(library.id())
		.iter()
		.any(|source| source.attached)
	{
		return;
	}

	let Some(home) = dirs::home_dir() else {
		debug!("No home directory on this platform");
		return;
	};
	if !home.is_dir() {
		return;
	}

	match crate::ops::sources::track::track_and_index(library, context, home.clone(), false).await {
		Ok(output) => info!(
			root = %output.root.display(),
			library = %library.id(),
			source = %output.id,
			"Added the home source"
		),
		Err(error) => warn!(root = %home.display(), %error, "Could not add the home source"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_library_can_claim_startup_only_once() {
		let gate = StartupIndexingGate::default();
		let first = Uuid::now_v7();
		let second = Uuid::now_v7();

		assert!(gate.claim(first));
		assert!(!gate.claim(first));
		assert!(gate.claim(second));
	}

	#[test]
	fn startup_is_available_through_the_registered_api() {
		assert!(crate::infra::wire::registry::LIBRARY_ACTIONS
			.contains_key(crate::action_method!("indexing.startup")));
	}
}
