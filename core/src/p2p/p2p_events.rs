use std::sync::Arc;

use sd_p2p2::{HookEvent, RemoteIdentity, P2P};
use serde::Serialize;
use specta::Type;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use super::PeerMetadata;

/// TODO: P2P event for the frontend
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum P2PEvent {
	DiscoveredPeer {
		identity: RemoteIdentity,
		metadata: PeerMetadata,
	},
	ExpiredPeer {
		identity: RemoteIdentity,
	},
	ConnectedPeer {
		identity: RemoteIdentity,
	},
	DisconnectedPeer {
		identity: RemoteIdentity,
	},
	SpacedropRequest {
		id: Uuid,
		identity: RemoteIdentity,
		peer_name: String,
		files: Vec<String>,
	},
	SpacedropProgress {
		id: Uuid,
		percent: u8,
	},
	SpacedropTimedout {
		id: Uuid,
	},
	SpacedropRejected {
		id: Uuid,
	},
}

/// A P2P hook which listens for events and sends them over a channel which can be connected to the frontend.
pub struct P2PEvents {
	events: (broadcast::Sender<P2PEvent>, broadcast::Receiver<P2PEvent>),
}

impl P2PEvents {
	pub fn spawn(p2p: Arc<P2P>) -> Self {
		let events = broadcast::channel(15);
		let (tx, mut rx) = mpsc::channel(15);
		let _ = p2p.register_hook(tx);

		let events_tx = events.0.clone();
		tokio::spawn(async move {
			while let Some(event) = rx.recv().await {
				match event {
					// TODO: Create `P2PEvent` from `HookEvent` and emit on `events_tx`
					HookEvent::MetadataChange(_) => todo!(),
					HookEvent::DiscoveredChange(_) => todo!(),
					HookEvent::ListenersChange(_) => todo!(),
					HookEvent::Shutdown => return,
				}
			}
		});

		Self { events }
	}

	pub fn subscribe(&self) -> broadcast::Receiver<P2PEvent> {
		self.events.0.subscribe()
	}

	pub fn send(&self, event: P2PEvent) -> Result<usize, broadcast::error::SendError<P2PEvent>> {
		self.events.0.send(event)
	}
}
