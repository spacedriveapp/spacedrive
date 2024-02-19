use std::sync::Arc;

use sd_p2p2::{flume::bounded, HookEvent, RemoteIdentity, P2P};
use serde::Serialize;
use specta::Type;
use tokio::sync::broadcast;
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
		let (tx, rx) = bounded(15);
		let _ = p2p.register_hook("sd-frontend-events", tx);

		let events_tx = events.0.clone();
		tokio::spawn(async move {
			while let Ok(event) = rx.recv_async().await {
				let event = match event {
					// We use `HookEvent::PeerUnavailable`/`HookEvent::PeerAvailable` over `HookEvent::PeerExpiredBy`/`HookEvent::PeerDiscoveredBy` so that having an active connection is treated as "discovered".
					// It's possible to have an active connection without mDNS data (which is what Peer*By` are for)
					HookEvent::PeerAvailable(peer) => {
						let metadata = match PeerMetadata::from_hashmap(&*peer.metadata()) {
							Ok(metadata) => metadata,
							Err(e) => {
								println!(
									"Invalid metadata for peer '{}': {:?}",
									peer.identity(),
									e
								);
								continue;
							}
						};

						P2PEvent::DiscoveredPeer {
							identity: peer.identity(),
							metadata,
						}
					}
					HookEvent::PeerUnavailable(identity) => P2PEvent::ExpiredPeer { identity },
					// TODO: Finish this
					// HookEvent::PeerConnectedWith {
					// 	listener,
					// 	peer,
					// 	first_connection,
					// } if first_connection => P2PEvent::ConnectedPeer {
					// 	identity: peer.identity(),
					// },
					// HookEvent::PeerDisconnectedWith {
					// 	listener,
					// 	identity,
					// 	last_connection,
					// } if last_connection => P2PEvent::DisconnectedPeer { identity },
					HookEvent::Shutdown { _guard } => break,
					_ => continue,
				};

				let _ = events_tx.send(event);
			}
		});

		Self { events }
	}

	pub fn subscribe(&self) -> broadcast::Receiver<P2PEvent> {
		self.events.0.subscribe()
	}

	#[allow(clippy::result_large_err)]
	pub fn send(&self, event: P2PEvent) -> Result<usize, broadcast::error::SendError<P2PEvent>> {
		self.events.0.send(event)
	}
}
