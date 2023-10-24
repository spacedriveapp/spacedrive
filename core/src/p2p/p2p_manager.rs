use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, PoisonError, RwLock,
	},
};

use sd_p2p::{
	spaceblock::{BlockSize, SpaceblockRequest, SpaceblockRequests, Transfer},
	spacetunnel::{RemoteIdentity, Tunnel},
	DiscoveredPeer, Event, Manager, ManagerError, ManagerStream, PeerId, PeerStatus, Service,
};
use sd_prisma::prisma::file_path;
use tokio::{
	fs::{create_dir_all, File},
	io::{AsyncWriteExt, BufReader, BufWriter},
	sync::{broadcast, oneshot, Mutex},
	time::sleep,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	location::file_path_helper::{file_path_to_handle_p2p_serve_file, IsolatedFilePathData},
	node::config::{self, NodeConfig},
	p2p::{
		operations::SPACEDROP_TIMEOUT, sync::networked_libraries_v2, OperatingSystem,
		SPACEDRIVE_APP_ID,
	},
	Node,
};

use super::{sync::SyncMessage, Header, LibraryServices, P2PEvent, PairingManager, PeerMetadata};

pub struct P2PManager {
	// TODO: Remove `pub(crate)` from these
	pub(crate) node: Service<PeerMetadata>,
	libraries: LibraryServices,

	pub events: (broadcast::Sender<P2PEvent>, broadcast::Receiver<P2PEvent>),
	pub manager: Arc<Manager>,
	pub(super) spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub(super) spacedrop_cancelations: Arc<Mutex<HashMap<Uuid, Arc<AtomicBool>>>>,
	pub pairing: Arc<PairingManager>,
	node_config_manager: Arc<config::Manager>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<config::Manager>,
		libraries: Arc<crate::library::Libraries>,
	) -> Result<(Arc<P2PManager>, ManagerStream), ManagerError> {
		let (keypair, manager_config) = {
			let config = node_config.get().await;
			(config.keypair, config.p2p.clone())
		};

		let (manager, stream) =
			sd_p2p::Manager::new(SPACEDRIVE_APP_ID, &keypair, manager_config).await?;

		info!(
			"Node '{}' is now online listening at addresses: {:?}",
			manager.peer_id(),
			stream.listen_addrs()
		);

		// need to keep 'rx' around so that the channel isn't dropped
		let (tx, rx) = broadcast::channel(100);
		let pairing = PairingManager::new(manager.clone(), tx.clone());

		let this = Arc::new(Self {
			node: Service::new("node", manager.clone()).unwrap(),
			libraries: Default::default(), // TODO: Initially populate this
			pairing,
			events: (tx, rx),
			manager,
			spacedrop_pairing_reqs: Default::default(),
			spacedrop_cancelations: Default::default(),
			node_config_manager: node_config,
		});

		tokio::spawn(networked_libraries_v2(this.clone(), libraries));

		Ok((this, stream))
	}

	pub fn start(self: Arc<Self>, mut stream: ManagerStream, node: Arc<Node>) {
		tokio::spawn({
			let this = self.clone();

			async move {
				let mut shutdown = false;
				while let Some(event) = stream.next().await {
					match event {
						// TODO: Bring these back
						// Event::PeerDiscovered(event) => {
						// 	this.events
						// 		.0
						// 		.send(P2PEvent::DiscoveredPeer {
						// 			peer_id: event.peer_id,
						// 			metadata: event.metadata.clone(),
						// 		})
						// 		.map_err(|_| error!("Failed to send event to p2p event stream!"))
						// 		.ok();

						// 	this.peer_discovered(event).await;
						// }
						// Event::PeerExpired { id, .. } => {
						// 	this.events
						// 		.0
						// 		.send(P2PEvent::ExpiredPeer { peer_id: id })
						// 		.map_err(|_| error!("Failed to send event to p2p event stream!"))
						// 		.ok();

						// 	this.peer_expired(id);
						// }
						Event::PeerConnected(event) => {
							this.events
								.0
								.send(P2PEvent::ConnectedPeer {
									peer_id: event.peer_id,
								})
								.map_err(|_| error!("Failed to send event to p2p event stream!"))
								.ok();

							this.libraries.peer_connected(event.peer_id);

							let this = this.clone();
							let node = node.clone();
							// let instances = this.metadata_manager.get().instances;
							tokio::spawn(async move {
								if event.establisher {
									let mut stream =
										this.manager.stream(event.peer_id).await.unwrap();
									// Self::resync(
									// 	&this.libraries,
									// 	&mut stream,
									// 	event.peer_id,
									// 	instances,
									// )
									// .await;
									todo!();
								}

								Self::resync_part2(&this.libraries, node, &event.peer_id).await;
							});
						}
						Event::PeerDisconnected(peer_id) => {
							this.events
								.0
								.send(P2PEvent::DisconnectedPeer { peer_id })
								.map_err(|_| error!("Failed to send event to p2p event stream!"))
								.ok();

							this.libraries.peer_disconnected(peer_id);
						}
						Event::PeerMessage(event) => {
							let this = this.clone();
							let node = node.clone();

							tokio::spawn(async move {
								let mut stream = event.stream;
								let header = Header::from_stream(&mut stream).await.unwrap();

								match header {
									Header::Ping => {
										debug!("Received ping from peer '{}'", event.peer_id);
									}
									Header::Spacedrop(req) => {
										let id = req.id;
										let (tx, rx) = oneshot::channel();

										info!(
											"({id}): received '{}' files from peer '{}' with block size '{:?}'",
											req.requests.len(), event.peer_id, req.block_size
										);
										this.spacedrop_pairing_reqs.lock().await.insert(id, tx);

										if this
											.events
											.0
											.send(P2PEvent::SpacedropRequest {
												id,
												peer_id: event.peer_id,
												peer_name: "Unknown".into(),
												// TODO: A better solution to this
												// manager
												// 	.get_discovered_peers()
												// 	.await
												// 	.into_iter()
												// 	.find(|p| p.peer_id == event.peer_id)
												// 	.map(|p| p.metadata.name)
												// 	.unwrap_or_else(|| "Unknown".to_string()),
												files: req
													.requests
													.iter()
													.map(|req| req.name.clone())
													.collect::<Vec<_>>(),
											})
											.is_err()
										{
											// No frontend's are active

											todo!("Outright reject Spacedrop");
										}

										tokio::select! {
											_ = sleep(SPACEDROP_TIMEOUT) => {
												info!("({id}): timeout, rejecting!");

												stream.write_all(&[0]).await.unwrap();
												stream.flush().await.unwrap();
											}
											file_path = rx => {
												match file_path {
													Ok(Some(file_path)) => {
														info!("({id}): accepted saving to '{:?}'", file_path);

														let cancelled = Arc::new(AtomicBool::new(false));
														this.spacedrop_cancelations
															.lock()
															.await
															.insert(id, cancelled.clone());

														stream.write_all(&[1]).await.unwrap();

														let names = req.requests.iter().map(|req| req.name.clone()).collect::<Vec<_>>();
														let mut transfer = Transfer::new(&req, |percent| {
															this.events.0.send(P2PEvent::SpacedropProgress { id, percent }).ok();
														}, &cancelled);

														let file_path = PathBuf::from(file_path);
														let names_len = names.len();
														for file_name in names {
															 // When transferring more than 1 file we wanna join the incoming file name to the directory provided by the user
															 let mut path = file_path.clone();
															 if names_len != 1 {
																// We know the `file_path` will be a directory so we can just push the file name to it
																path.push(&file_name);
															}

															debug!("({id}): accepting '{file_name}' and saving to '{:?}'", path);

															if let Some(parent) = path.parent() {
															 create_dir_all(parent).await.unwrap();
															}

															let f = File::create(path).await.unwrap();
															let f = BufWriter::new(f);
															transfer.receive(&mut stream, f).await;
														}

														info!("({id}): complete");
													}
													Ok(None) => {
														info!("({id}): rejected");

														stream.write_all(&[0]).await.unwrap();
														stream.flush().await.unwrap();
													}
													Err(_) => {
														info!("({id}): error with Spacedrop pairing request receiver!");
													}
												}
											}
										};
									}
									Header::Pair => {
										this.pairing
											.clone()
											.responder(
												event.peer_id,
												stream,
												&node.libraries,
												node.clone(),
											)
											.await;
									}
									Header::Sync(library_id) => {
										let mut tunnel = Tunnel::responder(stream).await.unwrap();

										let msg =
											SyncMessage::from_stream(&mut tunnel).await.unwrap();

										let library =
											node.libraries.get_library(&library_id).await.unwrap();

										match msg {
											SyncMessage::NewOperations => {
												super::sync::responder(&mut tunnel, library).await;
											}
										};
									}
									Header::File {
										id,
										library_id,
										file_path_id,
										range,
									} => {
										if !node.files_over_p2p_flag.load(Ordering::Relaxed) {
											panic!("Files over P2P is disabled!");
										}

										// TODO: Tunnel and authentication
										// TODO: Use BufReader

										let library =
											node.libraries.get_library(&library_id).await.unwrap();

										let file_path = library
											.db
											.file_path()
											.find_unique(file_path::pub_id::equals(
												file_path_id.as_bytes().to_vec(),
											))
											.select(file_path_to_handle_p2p_serve_file::select())
											.exec()
											.await
											.unwrap()
											.unwrap();

										let location = file_path.location.as_ref().unwrap();
										let location_path = location.path.as_ref().unwrap();
										let path = Path::new(location_path).join(
											IsolatedFilePathData::try_from((
												location.id,
												&file_path,
											))
											.unwrap(),
										);

										debug!("Serving path '{:?}' over P2P", path);

										let file = File::open(&path).await.unwrap();

										let metadata = file.metadata().await.unwrap();
										let block_size = BlockSize::from_size(metadata.len());

										stream.write_all(&block_size.to_bytes()).await.unwrap();
										stream
											.write_all(&metadata.len().to_le_bytes())
											.await
											.unwrap();

										let file = BufReader::new(file);
										Transfer::new(
											&SpaceblockRequests {
												id,
												block_size,
												requests: vec![SpaceblockRequest {
													// TODO: Removing need for this field in this case
													name: "todo".to_string(),
													size: metadata.len(),
													range,
												}],
											},
											|percent| {
												debug!(
													"P2P loading file path '{}' - progress {}%",
													file_path_id, percent
												);
											},
											&Arc::new(AtomicBool::new(false)),
										)
										.send(&mut stream, file)
										.await;
									}
									Header::Connected(identities) => {
										// Self::resync_handler(
										// 	&this.libraries,
										// 	&mut stream,
										// 	event.peer_id,
										// 	this.metadata_manager.get().instances,
										// 	identities,
										// )
										// .await;
										todo!();
									}
								}
							});
						}
						Event::PeerBroadcast(_event) => {
							// todo!();
						}
						Event::Shutdown => {
							shutdown = true;
							break;
						}
						_ => {}
					}
				}

				if !shutdown {
					error!(
						"Manager event stream closed! The core is unstable from this point forward!"
					);
				}
			}
		});
	}

	pub fn get_library_service(&self, library_id: &Uuid) -> Option<Arc<Service<PeerMetadata>>> {
		Some(self.libraries.get(library_id)?)
	}

	pub async fn update_metadata(&self, instances: Vec<RemoteIdentity>) {
		self.node.update({
			let config = self.node_config_manager.get().await;
			PeerMetadata {
				name: config.name.clone(),
				operating_system: Some(OperatingSystem::get_os()),
				version: Some(env!("CARGO_PKG_VERSION").to_string()),
			}
		});

		// TODO: Update the instance services
		for instance in instances {
			// self.libraries.
		}
	}

	pub(super) async fn peer_discovered(&self, event: DiscoveredPeer<PeerMetadata>) {
		let mut should_connect = false;
		// for lib in self
		// 	.libraries
		// 	.write()
		// 	.unwrap_or_else(PoisonError::into_inner)
		// 	.values_mut()
		// {
		// 	if let Some((_pk, instance)) = lib
		// 		._get_mut()
		// 		.iter_mut()
		// 		.find(|(pk, _)| event.metadata.instances.iter().any(|pk2| *pk2 == **pk))
		// 	{
		// 		if !matches!(instance, PeerStatus::Connected(_)) {
		// 			should_connect = matches!(instance, PeerStatus::Unavailable);

		// 			*instance = PeerStatus::Discovered(event.peer_id);
		// 		}

		// 		break; // PK can only exist once so we short circuit
		// 	}
		// }
		todo!();

		// We do this here not in the loop so the future can be `Send`
		if should_connect {
			event.dial().await;
		}
	}

	pub fn subscribe(&self) -> broadcast::Receiver<P2PEvent> {
		self.events.0.subscribe()
	}

	pub async fn shutdown(&self) {
		self.manager.shutdown().await;
	}
}
