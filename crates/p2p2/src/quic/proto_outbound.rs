use std::{
	future::Future,
	pin::Pin,
	sync::{atomic::Ordering, Arc, PoisonError},
};

use libp2p::{core::UpgradeInfo, swarm::ConnectionId, OutboundUpgrade, Stream};
use tracing::{debug, warn};

use super::{behaviour::SpaceTimeState, libp2p::SpaceTimeProtocolName, stream::new_outbound};

pub struct OutboundProtocol {
	pub(crate) connection_id: ConnectionId,
	pub(crate) state: Arc<SpaceTimeState>,
}

impl UpgradeInfo for OutboundProtocol {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[SpaceTimeProtocolName::new(&self.state.p2p)]
	}
}

impl OutboundUpgrade<Stream> for OutboundProtocol {
	type Output = ();
	type Error = ();
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

	fn upgrade_outbound(self, io: Stream, _protocol: Self::Info) -> Self::Future {
		let id = self.state.stream_id.fetch_add(1, Ordering::Relaxed);
		debug!("Establishing outbound connection {id}");
		Box::pin(async move {
			// TODO: Skip this and handle it if the `self.state.establishing_outbound` is empty
			let result = new_outbound(id, self.state.p2p.identity(), io)
				.await
				.map_err(|_| "error creating outbound stream".to_string());

			let Some(req) = self
				.state
				.establishing_outbound
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.remove(&self.connection_id)
			else {
				warn!(
					"id({id}): outbound connection '{}', no request found",
					self.connection_id
				);
				return Ok(());
			};

			let _ = req.tx.send(result);

			Ok(())
		})
	}
}
