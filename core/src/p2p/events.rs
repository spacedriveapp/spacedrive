use std::sync::Arc;

use sd_p2p2::{flume::bounded, HookEvent, HookId, PeerConnectionCandidate, RemoteIdentity, P2P};
use serde::Serialize;
use specta::Type;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::PeerMetadata;

/// The method used for the connection with this peer.
/// *Technically* you can have multiple under the hood but this simplifies things for the UX.
#[derive(Debug, Clone, Serialize, Type)]
pub enum ConnectionMethod {
	// Connected via the SD Relay
	Relay,
	// Connected directly via an IP address
	Local,
	// Not connected
	Disconnected,
}

/// The method used for the discovery of this peer.
/// *Technically* you can have multiple under the hood but this simplifies things for the UX.
#[derive(Debug, Clone, Serialize, Type)]
pub enum DiscoveryMethod {
	// Found via the SD Relay
	Relay,
	// Found via mDNS or a manual IP
	Local,
}

// This is used for synchronizing events between the backend and the frontend.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum P2PEvent {
	// An add or update event
	PeerChange {
		identity: RemoteIdentity,
		connection: ConnectionMethod,
		discovery: DiscoveryMethod,
		metadata: PeerMetadata,
	},
	// Delete a peer
	PeerDelete {
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
	SpacedropTimedOut {
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
	pub fn spawn(p2p: Arc<P2P>, libraries_hook_id: HookId) -> Self {
		let events = broadcast::channel(15);
		let (tx, rx) = bounded(15);
		let _ = p2p.register_hook("sd-frontend-events", tx);

		let events_tx = events.0.clone();
		tokio::spawn(async move {
			while let Ok(event) = rx.recv_async().await {
				let event = match event {
					// We use `HookEvent::PeerUnavailable`/`HookEvent::PeerAvailable` over `HookEvent::PeerExpiredBy`/`HookEvent::PeerDiscoveredBy` so that having an active connection is treated as "discovered".
					// It's possible to have an active connection without mDNS data (which is what Peer*By` are for)
					HookEvent::PeerConnectedWith(_, peer) | HookEvent::PeerAvailable(peer) => {
						let metadata = match PeerMetadata::from_hashmap(&peer.metadata()) {
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

						P2PEvent::PeerChange {
							identity: peer.identity(),
							connection: if peer.is_connected_with_hook(libraries_hook_id) {
								ConnectionMethod::Relay
							} else if peer.is_connected() {
								ConnectionMethod::Local
							} else {
								ConnectionMethod::Disconnected
							},
							discovery: match peer
								.connection_candidates()
								.contains(&PeerConnectionCandidate::Relay)
							{
								true => DiscoveryMethod::Relay,
								false => DiscoveryMethod::Local,
							},
							metadata,
						}
					}
					HookEvent::PeerUnavailable(identity) => P2PEvent::PeerDelete { identity },
					HookEvent::PeerDisconnectedWith(_, identity) => {
						let peers = p2p.peers();
						let Some(peer) = peers.get(&identity) else {
							continue;
						};

						if !peer.is_connected() {
							P2PEvent::PeerDelete { identity }
						} else {
							continue;
						}
					}
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
