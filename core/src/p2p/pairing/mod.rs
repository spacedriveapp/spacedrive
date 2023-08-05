#![allow(dead_code, unused)] // TODO: Remove once sorted outs

use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicU16, Ordering},
		Arc, RwLock,
	},
};

use chrono::Utc;
use futures::channel::oneshot;
use sd_p2p::{spacetunnel::Identity, Manager, MetadataManager, PeerId};

use sd_prisma::prisma::instance;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{
	io::{AsyncRead, AsyncWrite, AsyncWriteExt},
	sync::broadcast,
};
use tracing::{error, info};
use uuid::Uuid;

mod initial_sync;
mod proto;

pub use initial_sync::*;
use proto::*;

use crate::{
	library::{LibraryManager, LibraryName},
	node::{NodeConfig, Platform},
	p2p::{Header, IdentityOrRemoteIdentity, P2PManager},
};

use super::{P2PEvent, PeerMetadata};

pub struct PairingManager {
	id: AtomicU16,
	events_tx: broadcast::Sender<P2PEvent>,
	pairing_response: RwLock<HashMap<u16, oneshot::Sender<PairingDecision>>>,
	manager: Arc<Manager<PeerMetadata>>,
	metadata_manager: Arc<MetadataManager<PeerMetadata>>,
}

impl PairingManager {
	pub fn new(
		manager: Arc<Manager<PeerMetadata>>,
		events_tx: broadcast::Sender<P2PEvent>,
		metadata_manager: Arc<MetadataManager<PeerMetadata>>,
	) -> Arc<Self> {
		Arc::new(Self {
			id: AtomicU16::new(0),
			events_tx,
			pairing_response: RwLock::new(HashMap::new()),
			manager,
			metadata_manager,
		})
	}

	fn emit_progress(&self, id: u16, status: PairingStatus) {
		self.events_tx
			.send(P2PEvent::PairingProgress { id, status })
			.ok();
	}

	pub fn decision(&self, id: u16, decision: PairingDecision) {
		if let Some(tx) = self.pairing_response.write().unwrap().remove(&id) {
			tx.send(decision).ok();
		}
	}

	// TODO: Error handling

	pub async fn originator(
		self: Arc<Self>,
		peer_id: PeerId,
		node_config: NodeConfig,
		library_manager: Arc<LibraryManager>,
	) -> u16 {
		todo!();
	}

	pub async fn responder(
		self: Arc<Self>,
		peer_id: PeerId,
		mut stream: impl AsyncRead + AsyncWrite + Unpin,
		library_manager: &LibraryManager,
	) {
		todo!();
	}
}

#[derive(Debug, Type, Serialize, Deserialize)]
#[serde(tag = "decision", content = "libraryId", rename_all = "camelCase")]
pub enum PairingDecision {
	Accept(Uuid),
	Reject,
}

#[derive(Debug, Hash, Clone, Serialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum PairingStatus {
	EstablishingConnection,
	PairingRequested,
	LibraryAlreadyExists,
	PairingDecisionRequest,
	PairingInProgress {
		library_name: String,
		library_description: Option<String>,
	},
	InitialSyncProgress(u8),
	PairingComplete(Uuid),
	PairingRejected,
}

// TODO: Unit tests
