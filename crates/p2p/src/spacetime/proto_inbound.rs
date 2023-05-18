use std::{
	future::Future,
	pin::Pin,
	sync::{atomic::Ordering, Arc},
};

use libp2p::{core::UpgradeInfo, swarm::NegotiatedSubstream, InboundUpgrade};
use tracing::debug;

use crate::{Manager, ManagerStreamAction, Metadata, PeerId, PeerMessageEvent};

use super::{SpaceTimeProtocolName, SpaceTimeStream};

pub struct InboundProtocol<TMetadata: Metadata> {
	pub(crate) peer_id: PeerId,
	pub(crate) manager: Arc<Manager<TMetadata>>,
}

impl<TMetadata: Metadata> UpgradeInfo for InboundProtocol<TMetadata> {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[SpaceTimeProtocolName(self.manager.application_name)]
	}
}

impl<TMetadata: Metadata> InboundUpgrade<NegotiatedSubstream> for InboundProtocol<TMetadata> {
	type Output = ManagerStreamAction<TMetadata>;
	type Error = ();
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

	fn upgrade_inbound(self, io: NegotiatedSubstream, _: Self::Info) -> Self::Future {
		let id = self.manager.stream_id.fetch_add(1, Ordering::Relaxed);
		Box::pin(async move {
			debug!(
				"stream({}, {id}): accepting inbound connection",
				self.peer_id
			);

			let stream = SpaceTimeStream::from_stream(io).await;
			debug!(
				"stream({}, {id}): stream of type {} accepted",
				self.peer_id,
				stream.stream_type(),
			);

			Ok(ManagerStreamAction::Event(
				PeerMessageEvent {
					stream_id: id,
					peer_id: self.peer_id,
					manager: self.manager.clone(),
					stream,
					_priv: (),
				}
				.into(),
			))
		})
	}
}
