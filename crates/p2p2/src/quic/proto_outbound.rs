use std::{
	convert::Infallible,
	future::{ready, Ready},
	sync::{atomic::AtomicU64, Arc},
};

use libp2p::{core::UpgradeInfo, OutboundUpgrade, Stream};
use tokio::sync::oneshot;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::warn;

use crate::{Identity, P2P};

use super::{behaviour::SpaceTimeState, libp2p::SpaceTimeProtocolName};

#[derive(Debug)] // TODO: Would this be better as another type????
pub struct OutboundRequest(Infallible); // TODO: oneshot::Sender<UnicastStreamBuilder>

pub struct OutboundProtocol {
	pub(crate) state: Arc<SpaceTimeState>,
	pub(crate) req: OutboundRequest,
}

impl UpgradeInfo for OutboundProtocol {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[SpaceTimeProtocolName(self.state.p2p.app_name())]
	}
}

impl OutboundUpgrade<Stream> for OutboundProtocol {
	type Output = ();
	type Error = ();
	type Future = Ready<Result<(), ()>>;

	fn upgrade_outbound(self, io: Stream, _protocol: Self::Info) -> Self::Future {
		// let result = match self.req {
		// 	OutboundRequest::Unicast(sender) => {
		// 		// We write the discriminator to the stream in the `Manager::stream` method before returning the stream to the user to make async a tad nicer.
		// 		sender
		// 			.send(UnicastStreamBuilder::new(
		// 				self.identity.clone(),
		// 				io.compat(),
		// 			))
		// 			.map_err(|err| {
		// 				warn!("error transmitting unicast stream: {err:?}");
		// 			})
		// 	}
		// };

		// ready(result)
		todo!();
	}
}
