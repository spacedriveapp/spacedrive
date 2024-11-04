/// This module contains the public interface for volume management
use super::{
	actor::VolumeManagerMessage,
	error::VolumeError,
	types::{Volume, VolumeEvent, VolumeFingerprint},
};
use crate::library::Library;
use async_channel as chan;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tracing::instrument;

/// The public interface for volume management
#[derive(Clone)]
pub struct Volumes {
	pub message_tx: chan::Sender<VolumeManagerMessage>,
	pub event_tx: broadcast::Sender<VolumeEvent>,
}

impl Volumes {
	pub(crate) fn new(
		message_tx: chan::Sender<VolumeManagerMessage>,
		event_tx: broadcast::Sender<VolumeEvent>,
	) -> Self {
		Self {
			message_tx,
			event_tx,
		}
	}

	/// Creates a new subscription for volume events
	pub fn subscribe(&self) -> broadcast::Receiver<VolumeEvent> {
		self.event_tx.subscribe()
	}

	/// Lists all volumes, tracked and not tracked on the system
	pub async fn list_system_volumes(
		&self,
		library: Arc<Library>,
	) -> Result<Vec<Volume>, VolumeError> {
		let (tx, rx) = oneshot::channel();
		let msg = VolumeManagerMessage::ListSystemVolumes { ack: tx, library };

		self.message_tx
			.send(msg)
			.await
			.map_err(|_| VolumeError::Cancelled)?;

		rx.await.map_err(|_| VolumeError::Cancelled)?
	}

	/// Lists volumes for a specific library including system volumes
	pub async fn list_library_volumes(
		&self,
		library: Arc<Library>,
	) -> Result<Vec<Volume>, VolumeError> {
		let (tx, rx) = oneshot::channel();
		let msg = VolumeManagerMessage::ListLibraryVolumes { library, ack: tx };

		self.message_tx
			.send(msg)
			.await
			.map_err(|_| VolumeError::Cancelled)?;

		rx.await.map_err(|_| VolumeError::Cancelled)?
	}

	/// Track a volume in a specific library
	#[instrument(skip(self))]
	pub async fn track_volume(
		&self,
		fingerprint: VolumeFingerprint,
		library: Arc<Library>,
	) -> Result<(), VolumeError> {
		let (tx, rx) = oneshot::channel();
		let msg = VolumeManagerMessage::TrackVolume {
			fingerprint,
			library,
			ack: tx,
		};

		self.message_tx
			.send(msg)
			.await
			.map_err(|_| VolumeError::Cancelled)?;

		rx.await.map_err(|_| VolumeError::Cancelled)?
	}

	/// Stop tracking a volume
	#[instrument(skip(self))]
	pub async fn untrack_volume(
		&self,
		fingerprint: VolumeFingerprint,
		library: Arc<Library>,
	) -> Result<(), VolumeError> {
		let (tx, rx) = oneshot::channel();
		let msg = VolumeManagerMessage::UntrackVolume {
			fingerprint,
			library,
			ack: tx,
		};

		self.message_tx
			.send(msg)
			.await
			.map_err(|_| VolumeError::Cancelled)?;

		rx.await.map_err(|_| VolumeError::Cancelled)?
	}

	pub async fn unmount_volume(&self, fingerprint: VolumeFingerprint) -> Result<(), VolumeError> {
		let (tx, rx) = oneshot::channel();
		let msg = VolumeManagerMessage::UnmountVolume {
			fingerprint,
			ack: tx,
		};

		self.message_tx
			.send(msg)
			.await
			.map_err(|_| VolumeError::Cancelled)?;

		rx.await.map_err(|_| VolumeError::Cancelled)?;
		Ok(())
	}

	// Other public methods...
}
