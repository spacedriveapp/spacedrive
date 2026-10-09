//! Condition waits for integration tests.
//!
//! A fixed `sleep` either wastes the time the condition did not need or fails
//! when the machine is slower than the number. These helpers poll the real
//! condition on a short interval and give up at a deadline, so a test spends
//! only as long as the thing it waits for. The subprocess suites include this
//! file on its own with `#[path]`, which is why it depends on nothing else in
//! `helpers`.

#![allow(dead_code)]

use sd_core::{
	infra::event::Event,
	service::file_sharing::{TransferId, TransferState},
	Core,
};
use std::{future::Future, path::Path, time::Duration};
use tokio::time::Instant;
use uuid::Uuid;

/// How often a wait re-checks its condition.
pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Poll `check` until it returns `Some`, failing with `what` at the deadline.
pub async fn wait_until<T, F, Fut>(
	what: &str,
	deadline: Duration,
	mut check: F,
) -> anyhow::Result<T>
where
	F: FnMut() -> Fut,
	Fut: Future<Output = Option<T>>,
{
	let started = Instant::now();
	loop {
		if let Some(value) = check().await {
			return Ok(value);
		}
		if started.elapsed() >= deadline {
			anyhow::bail!("timed out after {deadline:?} waiting for {what}");
		}
		tokio::time::sleep(POLL_INTERVAL).await;
	}
}

/// Wait for a marker file another process writes and return its trimmed
/// content.
pub async fn wait_for_file(path: impl AsRef<Path>, deadline: Duration) -> anyhow::Result<String> {
	let path = path.as_ref();
	wait_until(&format!("file {}", path.display()), deadline, || async {
		tokio::fs::read_to_string(path)
			.await
			.ok()
			.map(|content| content.trim().to_string())
	})
	.await
}

/// Wait until the core reports at least one connected device and return the
/// first one.
pub async fn wait_for_connected_device(core: &Core, deadline: Duration) -> anyhow::Result<Uuid> {
	wait_until("a connected device", deadline, || async {
		core.services
			.device
			.get_connected_devices()
			.await
			.ok()
			.and_then(|devices| devices.first().copied())
	})
	.await
}

/// Wait until the core has a paired peer and return its id.
///
/// A connected device wins; a device that paired but whose persistent
/// connection has not come up yet is enough to address a transfer, which
/// dials on demand.
pub async fn wait_for_paired_device(core: &Core, deadline: Duration) -> anyhow::Result<Uuid> {
	wait_until("a paired device", deadline, || async {
		if let Some(device) = core
			.services
			.device
			.get_connected_devices()
			.await
			.ok()
			.and_then(|devices| devices.first().copied())
		{
			return Some(device);
		}
		let networking = core.networking()?;
		let registry = networking.device_registry();
		let paired = registry.read().await.get_paired_devices();
		paired.first().map(|device| device.device_id)
	})
	.await
}

/// Wait until a file sharing transfer leaves its in-progress states and
/// return whether it completed.
pub async fn wait_for_transfer(
	core: &Core,
	transfer_id: &TransferId,
	deadline: Duration,
) -> anyhow::Result<bool> {
	wait_until("the transfer to finish", deadline, || async {
		match core
			.services
			.file_sharing
			.get_transfer_status(transfer_id)
			.await
		{
			Ok(status) => match status.state {
				TransferState::Completed => Some(true),
				TransferState::Failed => {
					println!("transfer {transfer_id:?} failed: {:?}", status.error);
					Some(false)
				}
				_ => None,
			},
			Err(e) => {
				println!("could not read transfer status: {e}");
				None
			}
		}
	})
	.await
}

/// Wait until the core reports `peer` as connected.
pub async fn wait_for_peer(core: &Core, peer: Uuid, deadline: Duration) -> anyhow::Result<()> {
	wait_until(&format!("peer {peer} to connect"), deadline, || async {
		core.services
			.device
			.get_connected_devices()
			.await
			.ok()
			.filter(|devices| devices.contains(&peer))
			.map(|_| ())
	})
	.await
}

/// Wait until a directory holds `expected` entries and return their names.
pub async fn wait_for_dir_entries(
	dir: impl AsRef<Path>,
	expected: usize,
	deadline: Duration,
) -> anyhow::Result<Vec<String>> {
	let dir = dir.as_ref();
	wait_until(
		&format!("{expected} entries in {}", dir.display()),
		deadline,
		|| async {
			let names: Vec<String> = std::fs::read_dir(dir)
				.ok()?
				.filter_map(|entry| entry.ok())
				.map(|entry| entry.file_name().to_string_lossy().to_string())
				.collect();
			(names.len() >= expected).then_some(names)
		},
	)
	.await
}

/// Wait for the first event on the core bus that `matches`.
///
/// Subscribe before causing the event: a broadcast subscriber only sees what
/// is emitted after it exists.
pub async fn wait_for_event<F>(
	core: &Core,
	deadline: Duration,
	mut matches: F,
) -> anyhow::Result<Event>
where
	F: FnMut(&Event) -> bool,
{
	let mut subscriber = core.events.subscribe();
	let wait = async {
		loop {
			match subscriber.recv().await {
				Ok(event) if matches(&event) => return Ok(event),
				Ok(_) => continue,
				Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
				Err(tokio::sync::broadcast::error::RecvError::Closed) => {
					anyhow::bail!("event bus closed before the event arrived")
				}
			}
		}
	};
	tokio::time::timeout(deadline, wait)
		.await
		.map_err(|_| anyhow::anyhow!("timed out after {deadline:?} waiting for an event"))?
}
