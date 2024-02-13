use std::{
	collections::HashMap,
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
use tokio::{io::AsyncWriteExt, sync::oneshot};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::debug;

use crate::{quic::stream::new_inbound, Peer, P2P};

use super::{behaviour::SpaceTimeState, libp2p::SpaceTimeProtocolName};

pub struct InboundProtocol {
	pub(crate) peer_id: PeerId,
	pub(crate) state: Arc<SpaceTimeState>,
}

impl UpgradeInfo for InboundProtocol {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[SpaceTimeProtocolName::new(&self.state.p2p)]
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

			let Ok(stream) = new_inbound(id, self.state.p2p.identity(), stream).await else {
				return Ok(());
			};
			debug!(
				"stream({id}): upgraded to Unicast stream with '{}'",
				stream.remote_identity()
			);

			// TODO: Hook this up
			// write_hashmap(stream, map).await;
			// read_hashmap(stream).await;
			let metadata = HashMap::new();

			let (shutdown_tx, shutdown_rx) = oneshot::channel();
			let peer = self.state.p2p.clone().connected_to(
				self.state.listener_id,
				stream.remote_identity(),
				metadata,
				shutdown_tx,
			);

			// TODO: Handle `shutdown_rx`

			Ok(())
		})
	}
}
