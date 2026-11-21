//! # Update checks
//!
//! The desktop app checks the release feed itself instead of calling the
//! updater plugin from the webview, because the plugin logs ERROR for a feed
//! that is missing or unparseable. The feed is `latest.json` on the newest
//! GitHub release, so it is missing whenever that release ships without one,
//! such as a pre-release or an older line. That is "no update" and logs at
//! info. A failure to reach the feed or to verify a download logs at warn.

use std::collections::HashMap;

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Error, Update, UpdaterExt};
use tokio::sync::Mutex;

/// Every update a check has found, by version, so a prompt left open across
/// later checks still installs the version it shows.
#[derive(Default)]
pub struct PendingUpdate(Mutex<HashMap<String, Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
	version: String,
	current_version: String,
}

/// Whether a failed check only means the feed offers nothing for this build.
///
/// The plugin reports a 404 or other non-success response as
/// `ReleaseNotFound`, a body that is not a release as `Serialization`, a
/// `Semver` error, or a reqwest decode error, and a feed without this
/// platform as `TargetNotFound`.
fn is_no_update(error: &Error) -> bool {
	match error {
		Error::ReleaseNotFound
		| Error::Serialization(_)
		| Error::Semver(_)
		| Error::TargetNotFound(_) => true,
		Error::Reqwest(error) => error.is_decode(),
		_ => false,
	}
}

#[tauri::command]
pub async fn check_for_update(
	app: AppHandle,
	pending: State<'_, PendingUpdate>,
) -> Result<Option<AvailableUpdate>, String> {
	let result = match app.updater() {
		Ok(updater) => updater.check().await,
		Err(error) => Err(error),
	};
	let update = match result {
		Ok(update) => update,
		Err(error) if is_no_update(&error) => {
			tracing::info!(%error, "No update: the release feed offers nothing for this build");
			None
		}
		Err(error) => {
			tracing::warn!(%error, "Update check failed");
			return Err(error.to_string());
		}
	};

	let available = update.as_ref().map(|update| AvailableUpdate {
		version: update.version.clone(),
		current_version: update.current_version.clone(),
	});
	match &available {
		Some(available) => tracing::info!(version = %available.version, "Update available"),
		None => tracing::debug!("No newer release than the running version"),
	}
	if let Some(update) = update {
		pending
			.0
			.lock()
			.await
			.insert(update.version.clone(), update);
	}
	Ok(available)
}

/// Downloads, verifies and installs `version`, which a check must have found.
///
/// Sends the download percentage on `on_progress` when the size is known.
/// The caller restarts the app afterwards.
#[tauri::command]
pub async fn install_update(
	pending: State<'_, PendingUpdate>,
	version: String,
	on_progress: Channel<Option<u32>>,
) -> Result<(), String> {
	let Some(update) = pending.0.lock().await.get(&version).cloned() else {
		return Err(format!("No checked update for version {version}"));
	};

	let mut downloaded: u64 = 0;
	update
		.download_and_install(
			|chunk, total| {
				downloaded += chunk as u64;
				let percent = total
					.filter(|total| *total > 0)
					.map(|total| (downloaded * 100 / total) as u32);
				let _ = on_progress.send(percent);
			},
			|| {},
		)
		.await
		.map_err(|error| {
			tracing::warn!(%error, version = %update.version, "Update install failed");
			error.to_string()
		})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_missing_or_unparseable_feed_is_no_update() {
		assert!(is_no_update(&Error::ReleaseNotFound));
		assert!(is_no_update(&Error::Serialization(
			serde_json::from_str::<serde_json::Value>("<html>").unwrap_err()
		)));
		assert!(is_no_update(&Error::Semver(
			"not-a-version".parse::<semver::Version>().unwrap_err()
		)));
		assert!(is_no_update(&Error::TargetNotFound(
			"darwin-aarch64".into()
		)));
	}

	#[test]
	fn network_and_signature_failures_are_real_failures() {
		assert!(!is_no_update(&Error::Network("connection reset".into())));
		assert!(!is_no_update(&Error::SignatureUtf8("bad".into())));
		assert!(!is_no_update(&Error::Io(std::io::Error::other(
			"disk full"
		))));
	}
}
