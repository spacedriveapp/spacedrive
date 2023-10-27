use std::sync::Arc;

use sd_p2p::{spacetunnel::Tunnel, Event, ManagerStream, Service};
use streamunordered::StreamUnordered;
use tokio::sync::mpsc;
use tracing::error;

use crate::Node;

use super::{operations, sync::SyncMessage, Header, LibraryMetadata, P2PEvent, P2PManager};

pub struct P2PManagerActor {
	pub(super) manager: Arc<P2PManager>,
	pub(super) stream: ManagerStream,
	pub(super) register_service_rx: mpsc::Receiver<Arc<Service<LibraryMetadata>>>,
}

impl P2PManagerActor {
	pub fn start(self, node: Arc<Node>) {
		let Self {
			manager: this,
			mut stream,
			register_service_rx,
		} = self;

		tokio::spawn({
			// let service_events = StreamUnordered::new();

			async move {
				let mut shutdown = false;

				// TODO: Hook this event back up
				// Event::PeerExpired { id, .. } => {
				// 	this.events
				// 		.0
				// 		.send(P2PEvent::ExpiredPeer { peer_id: id })
				// 		.map_err(|_| error!("Failed to send event to p2p event stream!"))
				// 		.ok();
				// }

				loop {
					// TODO: Don't use `tokio::select`
					tokio::select! {
						// Some(service) = register_service_rx.recv() => {
						//   service_events.insert(service.listen());
						// }
						// Some(service_event) = service_events => {
						//   this.events.0
						// 		.send(P2PEvent::DiscoveredPeer {
						// 			identity: todo!(),
						// 			metadata: todo!()
						// 		})
						// 		.map_err(|_| error!("Failed to send event to p2p event stream!"))
						// 		.ok();
						// }
						Some(event) = stream.next() => {
							match event {
								Event::PeerConnected(event) => {
									// this.events
									// 	.0
									// 	.send(P2PEvent::ConnectedPeer {
									// 		peer_id: event.peer_id,
									// 	})
									// 	.map_err(|_| error!("Failed to send event to p2p event stream!"))
									// 	.ok();
									todo!();

									// let node = node.clone();
									// let this = this.clone();
									// // let instances = this.metadata_manager.get().instances;
									// tokio::spawn(async move {
									// 	if event.establisher {
									// 		let mut stream =
									// 			this.manager.stream(event.peer_id).await.unwrap();

									// 		// Self::resync(
									// 		// 	&this.libraries,
									// 		// 	&mut stream,
									// 		// 	event.peer_id,
									// 		// 	instances,
									// 		// )
									// 		// .await;
									// 	}

									// 	// P2PManager::resync_part2(&this.libraries, node, &event.peer_id)
									// 	// 	.await;
									// });
								}
								Event::PeerDisconnected(peer_id) => {
									// this.events
									// 	.0
									// 	.send(P2PEvent::DisconnectedPeer { peer_id })
									// 	.map_err(|_| error!("Failed to send event to p2p event stream!"))
									// 	.ok();
									todo!();
								}
								Event::PeerMessage(mut event) => {
									let this = this.clone();
									let node = node.clone();

									tokio::spawn(async move {
										let header = Header::from_stream(&mut event.stream).await.unwrap();

										match header {
											Header::Ping => operations::ping::reciever(event).await,
											Header::Spacedrop(req) => {
												operations::spacedrop::reciever(&this, req, event).await
											}
											Header::Pair => {
												this.pairing
													.clone()
													.responder(
														event.identity,
														event.stream,
														&node.libraries,
														node.clone(),
													)
													.await;
											}
											Header::Sync(library_id) => {
												let mut tunnel =
													Tunnel::responder(event.stream).await.unwrap();

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
											Header::File(req) => {
												operations::request_file::reciever(&node, req, event).await
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
}
