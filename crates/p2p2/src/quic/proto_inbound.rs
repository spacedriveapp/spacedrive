use std::{
	future::Future,
	pin::Pin,
	sync::{atomic::Ordering, Arc, PoisonError},
};

use libp2p::{
	core::{ConnectedPoint, UpgradeInfo},
	InboundUpgrade, PeerId, Stream,
};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, warn};

use super::SpaceTimeProtocolName;

pub struct InboundProtocol {
	pub(crate) peer_id: PeerId,
	// pub(crate) manager: Arc<Manager>,
}

impl UpgradeInfo for InboundProtocol {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		// [SpaceTimeProtocolName(self.manager.application_name.clone())]
		todo!();
	}
}

impl InboundUpgrade<Stream> for InboundProtocol {
	type Output = (); // TODO: ManagerStreamAction2;
	type Error = ();
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

	fn upgrade_inbound(self, io: Stream, _: Self::Info) -> Self::Future {
		// let id = self.manager.stream_id.fetch_add(1, Ordering::Relaxed);
		// Box::pin(async move {
		// 	debug!(
		// 		"stream({}, {id}): accepting inbound connection",
		// 		self.peer_id
		// 	);

		// 	let io = io.compat();
		// 	debug!("stream({}, {id}): unicast stream accepted", self.peer_id);

		// 	let stream = match UnicastStream::new_inbound(self.manager.identity.clone(), io).await {
		// 		Ok(v) => v,
		// 		Err(err) => {
		// 			warn!(
		// 				"Failed to construct 'UnicastStream' with Peer('{}'): {err:?}",
		// 				self.peer_id
		// 			);
		// 			return Err(());
		// 		}
		// 	};

		// 	let establisher = {
		// 		let mut state = self
		// 			.manager
		// 			.state
		// 			.write()
		// 			.unwrap_or_else(PoisonError::into_inner);

		// 		state
		// 			.connected
		// 			.insert(self.peer_id, stream.remote_identity());

		// 		match state.connections.get(&self.peer_id) {
		// 			Some((endpoint, 0)) => Some(match endpoint {
		// 				ConnectedPoint::Dialer { .. } => true,
		// 				ConnectedPoint::Listener { .. } => false,
		// 			}),
		// 			None => {
		// 				warn!("Error getting PeerId({})'s connection state. This indicates a bug in P2P", self.peer_id);
		// 				None
		// 			}
		// 			_ => None,
		// 		}
		// 	};

		// 	debug!(
		// 		"sending establishment request to peer '{}'",
		// 		stream.remote_identity()
		// 	);

		// 	let identity = stream.remote_identity();
		// 	let mut events = vec![PeerMessageEvent {
		// 		stream_id: id,
		// 		identity,
		// 		manager: self.manager.clone(),
		// 		stream,
		// 		_priv: (),
		// 	}
		// 	.into()];

		// 	if let Some(establisher) = establisher {
		// 		events.push(Event::PeerConnected(ConnectedPeer {
		// 			identity,
		// 			establisher,
		// 		}));
		// 	}

		// 	Ok(ManagerStreamAction2::Events(events))
		// })
		todo!();
	}
}
