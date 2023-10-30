use std::{
	future::Future,
	pin::Pin,
	sync::{atomic::Ordering, Arc, PoisonError},
};

use libp2p::{
	core::{ConnectedPoint, UpgradeInfo},
	InboundUpgrade, PeerId, Stream,
};
use tokio::io::AsyncReadExt;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, warn};

use crate::{
	spacetime::UnicastStream, ConnectedPeer, Event, Manager, ManagerStreamAction2, PeerMessageEvent,
};

use super::SpaceTimeProtocolName;

pub struct InboundProtocol {
	pub(crate) peer_id: PeerId,
	pub(crate) manager: Arc<Manager>,
}

impl UpgradeInfo for InboundProtocol {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[SpaceTimeProtocolName(self.manager.application_name.clone())]
	}
}

impl InboundUpgrade<Stream> for InboundProtocol {
	type Output = ManagerStreamAction2;
	type Error = ();
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

	fn upgrade_inbound(self, io: Stream, _: Self::Info) -> Self::Future {
		let id = self.manager.stream_id.fetch_add(1, Ordering::Relaxed);
		Box::pin(async move {
			debug!(
				"stream({}, {id}): accepting inbound connection",
				self.peer_id
			);

			let mut io = io.compat();
			let discriminator = io.read_u8().await.unwrap(); // TODO: Timeout on this
			match discriminator {
				crate::spacetime::BROADCAST_DISCRIMINATOR => {
					debug!("stream({}, {id}): broadcast stream accepted", self.peer_id);
					// Ok(ManagerStreamAction2::Event(
					// 	PeerMessageEvent {
					// 		stream_id: id,
					// 		identity: self.identity,
					// 		manager: self.manager.clone(),
					// 		stream: BroadcastStream::new(io),
					// 		_priv: (),
					// 	}
					// 	.into(),
					// ))
					todo!("Broadcast's are cringe!");
				}
				crate::spacetime::UNICAST_DISCRIMINATOR => {
					debug!("stream({}, {id}): unicast stream accepted", self.peer_id);

					let stream =
						UnicastStream::new_inbound(self.manager.identity.clone(), io).await;

					let establisher = {
						let mut state = self
							.manager
							.state
							.write()
							.unwrap_or_else(PoisonError::into_inner);

						state
							.connected
							.insert(self.peer_id, stream.remote_identity());

						match state.connections.get(&self.peer_id) {
							Some((endpoint, 0)) => Some(match endpoint {
								ConnectedPoint::Dialer { .. } => true,
								ConnectedPoint::Listener { .. } => false,
							}),
							None => {
								warn!("Error getting PeerId({})'s connection state. This indicates a bug in P2P", self.peer_id);
								None
							}
							_ => None,
						}
					};

					debug!(
						"sending establishment request to peer '{}'",
						stream.remote_identity()
					);

					let identity = stream.remote_identity();
					let mut events = vec![PeerMessageEvent {
						stream_id: id,
						identity,
						manager: self.manager.clone(),
						stream,
						_priv: (),
					}
					.into()];

					if let Some(establisher) = establisher {
						events.push(Event::PeerConnected(ConnectedPeer {
							identity,
							establisher,
						}));
					}

					Ok(ManagerStreamAction2::Events(events))
				}
				_ => todo!(), // TODO: Error handling
			}
		})
	}
}
