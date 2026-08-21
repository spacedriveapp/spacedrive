//! Mounts service — serves indexed sources as a read-only WebDAV share.
//!
//! The validation surface from `docs/core/design/mounts.md` (phase 2):
//! metadata answers from each source's arena, bytes stream from the source's
//! byte provider (local passthrough or the volume backend for cloud roots).
//! macOS mounts the share natively via Finder's "Connect to Server" with the
//! URL logged at startup; any WebDAV or HTTP-Range client works.
//!
//! Detached sources appear in the share and browse from their snapshots;
//! reads against them fail with 503 naming the source. The share is bound to
//! loopback only.

pub mod cache;
pub mod peer;
pub mod provider;
pub mod smb;
pub mod trace;
mod webdav;

pub use provider::{remote_share_name, resolve_target, share_name};

use crate::context::CoreContext;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

static BOUND_ADDR: OnceLock<SocketAddr> = OnceLock::new();

/// Address the WebDAV share is listening on, when running.
pub fn bound_addr() -> Option<SocketAddr> {
	BOUND_ADDR.get().copied()
}

const DEFAULT_PORT: u16 = 7764;

/// Bind the share on loopback and serve it for the life of the process.
/// The port comes from `SD_MOUNT_HTTP_PORT`, falling back to 7764, falling
/// back to an ephemeral port if the preferred one is taken.
pub async fn start(context: Arc<CoreContext>, cache_max_bytes: u64) -> anyhow::Result<SocketAddr> {
	let port = std::env::var("SD_MOUNT_HTTP_PORT")
		.ok()
		.and_then(|p| p.parse::<u16>().ok())
		.unwrap_or(DEFAULT_PORT);

	let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
		Ok(l) => l,
		Err(err) => {
			tracing::warn!("Mounts share port {port} unavailable ({err}); using ephemeral port");
			tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?
		}
	};
	let addr = listener.local_addr()?;

	// Blocks live in the per-source layout, so forgetting a source takes its
	// cached bytes with it.
	match crate::infra::source_dirs::SourceDirs::under_data_dir(&context.data_dir) {
		Ok(dirs) => {
			cache::init(dirs, cache_max_bytes).await;
		}
		Err(err) => tracing::warn!("Mounts block cache unavailable: {err}"),
	}

	let app = webdav::router(context.clone());
	tokio::spawn(async move {
		if let Err(err) = axum::serve(listener, app).await {
			tracing::error!("Mounts WebDAV server exited: {err}");
		}
	});

	let _ = BOUND_ADDR.set(addr);
	tracing::info!("Mounts WebDAV share at http://{addr}/dav/");

	// The SMB frontend is the one a host client mounts natively; WebDAV
	// stays as plain interop and as the baseline to measure against.
	if let Err(err) = smb::start(context.clone()).await {
		tracing::error!("Failed to start mounts SMB server: {err}");
	}

	tokio::spawn(watch_peers(context));
	Ok(addr)
}

/// Replicate sources from every paired device as it connects, so peer
/// shares appear without any explicit command. Networking starts after this
/// service, so the subscription is acquired lazily and re-acquired if the
/// event channel closes.
async fn watch_peers(context: Arc<CoreContext>) {
	use crate::service::network::core::NetworkEvent;

	loop {
		let Some(networking) = context.networking.read().await.clone() else {
			tokio::time::sleep(std::time::Duration::from_secs(5)).await;
			continue;
		};
		let mut events = networking.subscribe_events();

		// Devices that connected before this subscription existed (fast
		// auto-reconnects race service startup) are swept once here; the
		// event loop below covers everything after.
		for (device_id, label) in connected_devices(&networking).await {
			spawn_sync(&context, device_id, label);
		}

		loop {
			match events.recv().await {
				Ok(NetworkEvent::ConnectionEstablished { device_id, .. }) => {
					let label = connected_devices(&networking)
						.await
						.into_iter()
						.find_map(|(id, name)| (id == device_id).then_some(name))
						.unwrap_or_else(|| device_id.to_string());
					spawn_sync(&context, device_id, label);
				}
				Ok(_) => {}
				Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
				Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
			}
		}
	}
}

async fn connected_devices(
	networking: &Arc<crate::service::network::NetworkingService>,
) -> Vec<(uuid::Uuid, String)> {
	let registry = networking.device_registry();
	let registry = registry.read().await;
	registry
		.get_all_devices()
		.into_iter()
		.filter_map(|(id, state)| match state {
			crate::service::network::device::DeviceState::Connected { info, .. } => {
				Some((id, info.device_name))
			}
			_ => None,
		})
		.collect()
}

fn spawn_sync(context: &Arc<CoreContext>, device_id: uuid::Uuid, label: String) {
	if !peer::should_sync(device_id) {
		return;
	}
	let context = context.clone();
	tokio::spawn(async move {
		match peer::sync_device(&context, device_id, label.clone()).await {
			Ok(count) if count > 0 => {}
			Ok(_) => tracing::debug!("auto peer sync with {label}: no sources"),
			Err(err) => tracing::warn!("auto peer sync with {label} failed: {err}"),
		}
	});
}
