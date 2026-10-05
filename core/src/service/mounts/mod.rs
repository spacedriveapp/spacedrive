//! Mounts service: serves every indexed source, local and replicated from
//! paired devices, as one read-only share over HTTP (WebDAV) and SMB.
//!
//! Metadata answers from each source's arena and bytes stream from the
//! source's byte provider, so peer and cloud reads go through the block cache.
//! Spacedrive's own viewers stream a file from its HTTP URL
//! ([`file_url`] of its [`share_path`]); other apps open it by path inside the
//! SMB share, which [`attach`] mounts on this machine. See
//! `docs/core/design/mounts.md`.
//!
//! Detached sources appear in the share and browse from their snapshots;
//! reads against them fail with 503 naming the source. Both servers bind to
//! loopback only.

pub mod attach;
pub mod cache;
pub mod peer;
pub mod provider;
pub mod replication;
pub mod smb;
pub mod trace;
mod webdav;

pub use provider::{remote_share_name, resolve_target, share_name, share_path};
pub use webdav::file_url;

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

	// Known replicas come back from disk before anyone connects, so a
	// restart with every owner offline still lists and serves the inventory.
	// Runs ahead of the watchers: a reconnect sync only replaces a restored
	// share when the owner's generation moved.
	{
		let context = context.clone();
		tokio::spawn(async move {
			let (loaded, known) = peer::restore_known_replicas(&context).await;
			if known > 0 {
				tracing::info!(
					"peer mounts: restored {loaded} of {known} known replica(s) from disk"
				);
			}
		});
	}

	tokio::spawn(watch_peers(context.clone()));
	tokio::spawn(refresh_peers(context));
	Ok(addr)
}

/// How often to ask connected peers whether their indexes moved. The check
/// itself is one small request and transfers nothing unless a generation
/// changed, so this is paced for freshness rather than for cost.
const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Keep replicated peer indexes current while both machines stay connected.
///
/// Without this a peer's index is whatever it was at connect time: a file
/// added on the other machine never appears until a reconnect. The listing
/// carries a snapshot generation, so an unchanged source costs one request
/// and no transfer — which is why polling is adequate here and a push
/// protocol would only add a delivery guarantee we would still have to
/// back with this comparison.
async fn refresh_peers(context: Arc<CoreContext>) {
	loop {
		tokio::time::sleep(REFRESH_INTERVAL).await;

		let Some(networking) = context.networking.read().await.clone() else {
			continue;
		};
		for (device_id, label) in connected_devices(&networking).await {
			// Nothing replicated from this device yet: leave it to the
			// connect path rather than racing it.
			if !peer::remote_shares()
				.await
				.iter()
				.any(|share| share.device_id == device_id)
			{
				continue;
			}
			peer::allow_next_sync(device_id);
			match peer::sync_device(&context, device_id, label.clone()).await {
				Ok(_) => {}
				Err(err) => tracing::debug!("peer refresh with {label} failed: {err}"),
			}
		}
	}
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
