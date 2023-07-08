use std::sync::{
	atomic::{AtomicU16, Ordering},
	Arc,
};

use sd_p2p::{Manager, PeerId};

use serde::Serialize;
use specta::Type;
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;

mod proto;

use proto::*;

use crate::{library::LibraryManager, node::NodeConfig};

use super::{P2PEvent, PeerMetadata};

pub struct PairingManager {
	id: AtomicU16,
	events_tx: broadcast::Sender<P2PEvent>,
	manager: Arc<Manager<PeerMetadata>>,
}

impl PairingManager {
	pub fn new(
		manager: Arc<Manager<PeerMetadata>>,
		events_tx: broadcast::Sender<P2PEvent>,
	) -> Arc<Self> {
		Arc::new(Self {
			id: AtomicU16::new(0),
			events_tx,
			manager,
		})
	}

	fn emit_progress(&self, id: u16, status: PairingStatus) {
		self.events_tx
			.send(P2PEvent::PairingProgress { id, status })
			.ok();
	}

	// TODO: Error handling

	pub async fn originator(self: Arc<Self>, peer_id: PeerId, node_config: NodeConfig) -> u16 {
		// TODO: Timeout for max number of pairings in a time period

		let pairing_id = self.id.fetch_add(1, Ordering::SeqCst);
		self.emit_progress(pairing_id, PairingStatus::PairingRequested);

		info!("Beginning pairing '{pairing_id}' as originator to remote peer '{peer_id}'");

		tokio::spawn(async move {
			loop {
				self.emit_progress(pairing_id, PairingStatus::PairingRequested);
				tokio::time::sleep(std::time::Duration::from_secs(1)).await;
				self.emit_progress(pairing_id, PairingStatus::PairingComplete);
			}

			// let mut stream = self.manager.stream(peer_id).await.unwrap();

			// // TODO: Ensure both clients are on a compatible version cause Prisma model changes will cause issues

			// // 1. Create new instance for originator and send it to the responder
			// let now = Utc::now();
			// let req = PairingRequest(Instance {
			// 	id: Uuid::new_v4(),
			// 	identity: Identity::new(), // TODO: Public key only
			// 	node_id: node_config.id,
			// 	node_name: node_config.name,
			// 	node_platform: Platform::current(),
			// 	last_seen: now.into(),
			// 	date_created: now.into(),
			// });
			// stream.write_all(&mut req.to_bytes()).await.unwrap();

			// // 2.
			// match PairingResponse::from_stream(&mut stream).await.unwrap() {
			// 	PairingResponse::Accepted {
			// 		library_id,
			// 		library_name,
			// 		library_description,
			// 		instances,
			// 	} => {
			// 		// TODO: Tell frontend what's going on using channel

			// 		// TODO: Future - Library in pairing state
			// 		// TODO: Create library

			// 		// TODO: Insert all instances into library
			// 	}
			// 	PairingResponse::Rejected => {
			// 		todo!();

			// 		return;
			// 	}
			// }

			// 3.
			// TODO: Either rollback or update library out of pairing state

			// TODO: Fake initial sync

			// TODO: Done message to frontend

			// TODO: Remove from HashMap after a minute
		});

		pairing_id
	}

	pub async fn responder(self: Arc<Self>, peer_id: PeerId, library_manager: &LibraryManager) {
		info!("Beginning pairing as responder to remote peer '{peer_id}'");

		// let msg: PairingRequest = todo!(); // Receive from network

		// // Prompt the user
		// let PairingDecision::Accept(decision) = todo!() else {
		// 	// info!();

		// 	// send(PairingResponse::Rejected);

		// 	return;
		// };

		// let library: &Library = todo!();
		// let instances: Vec<instance::Data> = todo!().into_iter().map(|i| {
		// 	// TODO: If `i.identity` contains a public/private keypair replace it with the public key

		// 	i
		// });

		// // send(PairingResponse::Accepted {
		// // 	library_id: library.config.id,
		// // 	library_name: library.config.name,
		// // 	library_description: library.config.description,
		// //  instances,
		// // });

		// let msg: InsertOriginatorInstance = todo!(); // Receive from network

		// library.db.instance().create(msg.instance).await?;

		// send(ConfirmInsertOriginatorInstance::Ok);
	}
}

#[derive(Debug)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PairingDecision {
	Accept(Uuid),
	Reject,
}

#[derive(Debug, Hash, Clone, Serialize, Type)]
pub enum PairingStatus {
	PairingRequested,
	PairingInProgress {
		library_name: String,
		library_description: String,
		node_name: String,
	},
	InitialSyncProgress(u8),
	PairingComplete,
}

// TODO: Unit tests
