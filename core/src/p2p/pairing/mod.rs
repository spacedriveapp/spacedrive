#![allow(clippy::panic, clippy::unwrap_used)] // TODO: Finish this

use crate::{
	library::{Libraries, LibraryName},
	node::Platform,
	p2p::{Header, IdentityOrRemoteIdentity},
	Node,
};

use sd_p2p::{
	spacetunnel::{Identity, RemoteIdentity},
	Manager,
};
use sd_prisma::prisma::instance;

use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicU16, Ordering},
		Arc, RwLock,
	},
};

use chrono::Utc;
use futures::channel::oneshot;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{
	io::{AsyncRead, AsyncWrite, AsyncWriteExt},
	sync::broadcast,
};
use tracing::{info, warn};
use uuid::Uuid;

mod proto;

use proto::*;

use super::P2PEvent;

pub struct PairingManager {
	id: AtomicU16,
	events_tx: broadcast::Sender<P2PEvent>,
	pairing_response: RwLock<HashMap<u16, oneshot::Sender<PairingDecision>>>,
	manager: Arc<Manager>,
}

impl PairingManager {
	pub fn new(manager: Arc<Manager>, events_tx: broadcast::Sender<P2PEvent>) -> Arc<Self> {
		Arc::new(Self {
			id: AtomicU16::new(0),
			events_tx,
			pairing_response: RwLock::new(HashMap::new()),
			manager,
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

	pub async fn originator(self: Arc<Self>, identity: RemoteIdentity, node: Arc<Node>) -> u16 {
		// TODO: Timeout for max number of pairings in a time period

		let pairing_id = self.id.fetch_add(1, Ordering::SeqCst);
		self.emit_progress(pairing_id, PairingStatus::EstablishingConnection);

		info!("Beginning pairing '{pairing_id}' as originator to remote peer '{identity}'");

		tokio::spawn(async move {
			let mut stream = self.manager.stream(identity).await.unwrap();
			stream.write_all(&Header::Pair.to_bytes()).await.unwrap();

			// TODO: Ensure both clients are on a compatible version cause Prisma model changes will cause issues

			// 1. Create new instance for originator and send it to the responder
			self.emit_progress(pairing_id, PairingStatus::PairingRequested);
			let node_config = node.config.get().await;
			let now = Utc::now();
			let identity = Identity::new();
			let self_instance_id = Uuid::new_v4();
			let req = PairingRequest(Instance {
				id: self_instance_id,
				identity: identity.to_remote_identity(),
				node_id: node_config.id,
				node_name: node_config.name.clone(),
				node_platform: Platform::current(),
				last_seen: now,
				date_created: now,
			});
			stream.write_all(&req.to_bytes()).await.unwrap();

			// 2.
			match PairingResponse::from_stream(&mut stream).await.unwrap() {
				PairingResponse::Accepted {
					library_id,
					library_name,
					library_description,
					instances,
				} => {
					info!("Pairing '{pairing_id}' accepted by remote into library '{library_id}'");
					// TODO: Log all instances and library info
					self.emit_progress(
						pairing_id,
						PairingStatus::PairingInProgress {
							library_name: library_name.clone(),
							library_description: library_description.clone(),
						},
					);

					// TODO: Future - Library in pairing state
					// TODO: Create library

					if node
						.libraries
						.get_all()
						.await
						.into_iter()
						.any(|i| i.id == library_id)
					{
						self.emit_progress(pairing_id, PairingStatus::LibraryAlreadyExists);

						// TODO: Properly handle this at a protocol level so the error is on both sides

						return;
					}

					let (this, instances): (Vec<_>, Vec<_>) = instances
						.into_iter()
						.partition(|i| i.id == self_instance_id);

					if this.len() != 1 {
						todo!("error handling");
					}
					let this = this.first().expect("unreachable");
					if this.identity != identity.to_remote_identity() {
						todo!("error handling. Something went really wrong!");
					}

					let library = node
						.libraries
						.create_with_uuid(
							library_id,
							LibraryName::new(library_name).unwrap(),
							library_description,
							false, // We will sync everything which will conflict with the seeded stuff
							Some(instance::Create {
								pub_id: this.id.as_bytes().to_vec(),
								identity: IdentityOrRemoteIdentity::Identity(identity).to_bytes(),
								node_id: this.node_id.as_bytes().to_vec(),
								node_name: this.node_name.clone(), // TODO: Remove `clone`
								node_platform: this.node_platform as i32,
								last_seen: this.last_seen.into(),
								date_created: this.date_created.into(),
								_params: vec![],
							}),
							&node,
						)
						.await
						.unwrap();

					let library = node.libraries.get_library(&library.id).await.unwrap();

					library
						.db
						.instance()
						.create_many(
							instances
								.into_iter()
								.map(|i| {
									instance::CreateUnchecked {
										pub_id: i.id.as_bytes().to_vec(),
										identity: IdentityOrRemoteIdentity::RemoteIdentity(
											i.identity,
										)
										.to_bytes(),
										node_id: i.node_id.as_bytes().to_vec(),
										node_name: i.node_name,
										node_platform: i.node_platform as i32,
										last_seen: i.last_seen.into(),
										date_created: i.date_created.into(),
										// timestamp: Default::default(), // TODO: Source this properly!
										_params: vec![],
									}
								})
								.collect(),
						)
						.exec()
						.await
						.unwrap();

					// Called again so the new instances are picked up
					node.libraries.update_instances(library.clone()).await;

					// TODO: Done message to frontend
					self.emit_progress(pairing_id, PairingStatus::PairingComplete(library_id));
					stream.flush().await.unwrap();

					// Remember, originator creates a new stream internally so the handler for this doesn't have to do anything.
					super::sync::originator(library_id, &library.sync, &node.p2p).await;
				}
				PairingResponse::Rejected => {
					info!("Pairing '{pairing_id}' rejected by remote");
					self.emit_progress(pairing_id, PairingStatus::PairingRejected);
				}
			}
		});

		pairing_id
	}

	pub async fn responder(
		self: Arc<Self>,
		identity: RemoteIdentity,
		mut stream: impl AsyncRead + AsyncWrite + Unpin,
		library_manager: &Libraries,
		node: Arc<Node>,
	) -> Result<(), ()> {
		let pairing_id = self.id.fetch_add(1, Ordering::SeqCst);
		self.emit_progress(pairing_id, PairingStatus::EstablishingConnection);

		info!("Beginning pairing '{pairing_id}' as responder to remote peer '{identity}'");

		let remote_instance = match PairingRequest::from_stream(&mut stream).await {
			Ok(v) => v,
			Err((field_name, err)) => {
				warn!("Error reading field '{field_name}' of pairing request from remote: {err}");
				self.emit_progress(pairing_id, PairingStatus::PairingRejected);

				// TODO: Attempt to send error to remote and reset connection
				return Ok(());
			}
		}
		.0;
		self.emit_progress(pairing_id, PairingStatus::PairingDecisionRequest);
		self.events_tx
			.send(P2PEvent::PairingRequest {
				id: pairing_id,
				name: remote_instance.node_name.clone(),
				os: remote_instance.node_platform.into(),
			})
			.ok();

		// Prompt the user and wait
		// TODO: After 1 minute remove channel from map and assume it was rejected
		let (tx, rx) = oneshot::channel();
		self.pairing_response
			.write()
			.unwrap()
			.insert(pairing_id, tx);
		let PairingDecision::Accept(library_id) = rx.await.unwrap() else {
			info!("The user rejected pairing '{pairing_id}'!");
			// self.emit_progress(pairing_id, PairingStatus::PairingRejected); // TODO: Event to remove from frontend index
			stream
				.write_all(&PairingResponse::Rejected.to_bytes())
				.await
				.unwrap();
			return Ok(());
		};
		info!("The user accepted pairing '{pairing_id}' for library '{library_id}'!");

		let library = library_manager.get_library(&library_id).await.unwrap();

		// TODO: Rollback this on pairing failure
		instance::Create {
			pub_id: remote_instance.id.as_bytes().to_vec(),
			identity: IdentityOrRemoteIdentity::RemoteIdentity(remote_instance.identity).to_bytes(),
			node_id: remote_instance.node_id.as_bytes().to_vec(),
			node_name: remote_instance.node_name,
			node_platform: remote_instance.node_platform as i32,
			last_seen: remote_instance.last_seen.into(),
			date_created: remote_instance.date_created.into(),
			// timestamp: Default::default(), // TODO: Source this properly!
			_params: vec![],
		}
		.to_query(&library.db)
		.exec()
		.await
		.unwrap();
		library_manager.update_instances(library.clone()).await;

		let library_config = library.config().await;

		stream
			.write_all(
				&PairingResponse::Accepted {
					library_id: library.id,
					library_name: library_config.name.into(),
					library_description: library_config.description,
					instances: library
						.db
						.instance()
						.find_many(vec![])
						.exec()
						.await
						.unwrap()
						.into_iter()
						.filter_map(|i| {
							let Ok(id) = Uuid::from_slice(&i.pub_id) else {
								warn!("Invalid instance pub_id in database: {:?}", i.pub_id);
								return None;
							};

							let Ok(node_id) = Uuid::from_slice(&i.node_id) else {
								warn!("Invalid instance node_id in database: {:?}", i.node_id);
								return None;
							};

							Some(Instance {
								id,
								identity: IdentityOrRemoteIdentity::from_bytes(&i.identity)
									.unwrap()
									.remote_identity(),
								node_id,
								node_name: i.node_name,
								node_platform: Platform::try_from(i.node_platform as u8)
									.unwrap_or(Platform::Unknown),
								last_seen: i.last_seen.into(),
								date_created: i.date_created.into(),
							})
						})
						.collect(),
				}
				.to_bytes(),
			)
			.await
			.unwrap();

		// TODO: Pairing confirmation + rollback

		self.emit_progress(pairing_id, PairingStatus::PairingComplete(library_id));
		stream.flush().await.unwrap();

		// Remember, originator creates a new stream internally so the handler for this doesn't have to do anything.
		super::sync::originator(library_id, &library.sync, &node.p2p).await;

		Ok(())
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
