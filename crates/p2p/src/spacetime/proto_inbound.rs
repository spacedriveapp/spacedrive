use std::{
	future::Future,
	pin::Pin,
	sync::{atomic::Ordering, Arc},
};

use libp2p::{core::UpgradeInfo, swarm::NegotiatedSubstream, InboundUpgrade};
use tokio::io::AsyncReadExt;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::debug;

use crate::{
	spacetime::{BroadcastStream, UnicastStream},
	Manager, ManagerStreamAction2, Metadata, PeerId, PeerMessageEvent,
};

use super::SpaceTimeProtocolName;

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
	type Output = ManagerStreamAction2<TMetadata>;
	type Error = ();
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

	fn upgrade_inbound(self, io: NegotiatedSubstream, _: Self::Info) -> Self::Future {
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
					Ok(ManagerStreamAction2::Event(
						PeerMessageEvent {
							stream_id: id,
							peer_id: self.peer_id,
							manager: self.manager.clone(),
							stream: BroadcastStream::new(io),
							_priv: (),
						}
						.into(),
					))
				}
				crate::spacetime::UNICAST_DISCRIMINATOR => {
					debug!("stream({}, {id}): unicast stream accepted", self.peer_id);

					Ok(ManagerStreamAction2::Event(
						PeerMessageEvent {
							stream_id: id,
							peer_id: self.peer_id,
							manager: self.manager.clone(),
							stream: UnicastStream::new(io),
							_priv: (),
						}
						.into(),
					))
				}
				_ => todo!(), // TODO: Error handling
			}
		})
	}
}
