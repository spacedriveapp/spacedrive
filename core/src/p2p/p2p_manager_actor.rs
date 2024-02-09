use crate::Node;

use sd_p2p::{spacetunnel::Tunnel, Event, ManagerStream, Service, ServiceEvent};

use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::error;

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
			mut register_service_rx,
		} = self;

		tokio::spawn({
			async move {
				let mut node_rx = this.node.listen();

				loop {
					tokio::select! {
					   // TODO: We ignore the response of this but I suspect it will be useful in the future so it stays for now.
					   Some(_event) = register_service_rx.recv() => {},
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
										let header = Header::from_stream(&mut event.stream)
											.await
											.map_err(|err| {
												error!("Failed to read header from stream: {}", err);
											})?;

										match header {
											Header::Ping => operations::ping::reciever(event).await,
											Header::Spacedrop(req) => {
												operations::spacedrop::reciever(&this, req, event).await?
											}
											Header::Sync(library_id) => {
												let mut tunnel =
													Tunnel::responder(event.stream).await.map_err(|err| {
														error!("Failed `Tunnel::responder`: {}", err);
													})?;

												let msg =
													SyncMessage::from_stream(&mut tunnel).await.map_err(|err| {
														error!("Failed `SyncMessage::from_stream`: {}", err);
													})?;

												let library =
													node.libraries.get_library(&library_id).await.ok_or_else(|| {
														error!("Failed to get library '{library_id}'");

														// TODO: Respond to remote client with warning!
													})?;

												match msg {
													SyncMessage::NewOperations => {
														super::sync::responder(&mut tunnel, library).await?;
													}
												};
											}
											Header::File(req) => {
												operations::request_file::receiver(&node, req, event).await?;
											}
										}

										Ok::<_, ()>(())
									});
								}
								Event::Shutdown => break,
								_ => {}
							}
						}
					}
				}

				error!(
					"Manager event stream closed! The core is unstable from this point forward!"
				);
			}
		});
	}
}
