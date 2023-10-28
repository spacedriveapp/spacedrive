use std::sync::Arc;

use futures::StreamExt;
use sd_p2p::{spacetunnel::Tunnel, Event, ManagerStream, Service, ServiceEvent};
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
			async move {
				let mut shutdown = false;
				let mut node_rx = this.node.listen();

				loop {
					tokio::select! {
					   // TODO: We should subscribe to library-level events too but frontend isn't cut out for them right now.
					   Some(Ok(event)) = node_rx.next() => {
								this.events.0
										.send(match event {
											   ServiceEvent::Discovered { identity, metadata } =>
														P2PEvent::DiscoveredPeer {
															   identity,
															   metadata,
														},
											   ServiceEvent::Expired { identity } =>
														P2PEvent::ExpiredPeer {
															   identity,
														},
										})
										.map_err(|_| error!("Failed to send event to p2p event stream!"))
										.ok();
						}
						Some(event) = stream.next() => {
							match event {
								Event::PeerConnected(event) => {
									this.events
										.0
										.send(P2PEvent::ConnectedPeer {
											identity: event.identity,
										})
										.map_err(|_| error!("Failed to send event to p2p event stream!"))
										.ok();
								}
								Event::PeerDisconnected(identity) => {
									this.events
										.0
										.send(P2PEvent::DisconnectedPeer { identity })
										.map_err(|_| error!("Failed to send event to p2p event stream!"))
										.ok();
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
