use std::{
	collections::HashMap,
	future::Future,
	pin::Pin,
	sync::{atomic::Ordering, Arc, PoisonError},
};

use libp2p::{core::UpgradeInfo, swarm::ConnectionId, InboundUpgrade, PeerId, Stream};
use tokio::sync::oneshot;

use tracing::debug;

use crate::quic::stream::new_inbound;

use super::{behaviour::SpaceTimeState, libp2p::SpaceTimeProtocolName};

pub struct InboundProtocol {
	pub(crate) peer_id: PeerId,
	pub(crate) connection_id: ConnectionId,
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
	type Output = ();
	type Error = ();
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

	fn upgrade_inbound(self, stream: Stream, _: Self::Info) -> Self::Future {
		let id = self.state.stream_id.fetch_add(1, Ordering::Relaxed);
		debug!("Establishing inbound connection {id}");
		Box::pin(async move {
			let Ok(stream) = new_inbound(id, self.state.p2p.identity(), stream).await else {
				return Ok(());
			};
			debug!(
				"stream({id}): upgraded to Unicast stream with '{}'",
				stream.remote_identity()
			);

			// // TODO: This is temporary for debugging
			// if let Some(req) = self
			// 	.state
			// 	.establishing_outbound
			// 	.lock()
			// 	.unwrap_or_else(PoisonError::into_inner)
			// 	.remove(&self.connection_id)
			// {
			// 	println!("\n\nFIRED FROM INBOUND\n\n");
			// 	let _ = req.tx.send(Ok(stream));
			// 	return Ok(());
			// };

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
