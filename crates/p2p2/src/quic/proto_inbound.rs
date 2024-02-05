use std::{
	future::Future,
	pin::Pin,
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc, PoisonError,
	},
};

use libp2p::{
	core::{ConnectedPoint, UpgradeInfo},
	InboundUpgrade, PeerId, Stream,
};
use tokio::sync::oneshot;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, warn};

use crate::{identity, quic::stream::new_inbound, Peer, P2P};

use super::{behaviour::SpaceTimeState, SpaceTimeProtocolName};

pub struct InboundProtocol {
	pub(crate) peer_id: PeerId,
	pub(crate) p2p: Arc<P2P>,
	pub(crate) state: Arc<SpaceTimeState>,
}

impl UpgradeInfo for InboundProtocol {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[SpaceTimeProtocolName(self.p2p.app_name())]
	}
}

impl InboundUpgrade<Stream> for InboundProtocol {
	type Output = (); // TODO: ManagerStreamAction2;
	type Error = ();
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

	fn upgrade_inbound(self, stream: Stream, _: Self::Info) -> Self::Future {
		let id = self.state.stream_id.fetch_add(1, Ordering::Relaxed);
		Box::pin(async move {
			debug!(
				"stream({id}): accepting inbound connection with libp2p::PeerId({})",
				self.peer_id
			);

			let Ok(stream) = new_inbound(id, self.p2p.identity(), stream).await else {
				return Ok(());
			};
			debug!(
				"stream({id}): upgraded to Unicast stream with '{}'",
				stream.remote_identity()
			);

			// TODO: Sync `peer.metadata` with remote

			// let peer = Peer::new(stream.remote_identity());
			// let (tx, rx) = oneshot::channel();
			// peer.connected_to(listener, tx);
			// TODO: Handle `rx` for shutdown.

			// self.p2p.peers_mut().insert(peer.identity(), peer);

			// TODO: Update state to reflect that we are connected

			// TODO: Send this back to the application to handle

			Ok(())
		})
	}
}
