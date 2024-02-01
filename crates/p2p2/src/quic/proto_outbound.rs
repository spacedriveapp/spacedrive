// use std::future::{ready, Ready};

// use libp2p::{core::UpgradeInfo, OutboundUpgrade, Stream};
// use tokio::sync::oneshot;
// use tokio_util::compat::FuturesAsyncReadCompatExt;
// use tracing::warn;

// use crate::spacetunnel::Identity;

// use super::{SpaceTimeProtocolName, UnicastStreamBuilder};

// #[derive(Debug)]
// pub enum OutboundRequest {
// 	Unicast(oneshot::Sender<UnicastStreamBuilder>),
// }

// pub struct OutboundProtocol {
// 	pub(crate) application_name: String,
// 	pub(crate) req: OutboundRequest,
// 	pub(crate) identity: Identity,
// }

// impl UpgradeInfo for OutboundProtocol {
// 	type Info = SpaceTimeProtocolName;
// 	type InfoIter = [Self::Info; 1];

// 	fn protocol_info(&self) -> Self::InfoIter {
// 		[SpaceTimeProtocolName(self.application_name.clone())]
// 	}
// }

// impl OutboundUpgrade<Stream> for OutboundProtocol {
// 	type Output = ();
// 	type Error = ();
// 	type Future = Ready<Result<(), ()>>;

// 	fn upgrade_outbound(self, io: Stream, _protocol: Self::Info) -> Self::Future {
// 		let result = match self.req {
// 			OutboundRequest::Unicast(sender) => {
// 				// We write the discriminator to the stream in the `Manager::stream` method before returning the stream to the user to make async a tad nicer.
// 				sender
// 					.send(UnicastStreamBuilder::new(
// 						self.identity.clone(),
// 						io.compat(),
// 					))
// 					.map_err(|err| {
// 						warn!("error transmitting unicast stream: {err:?}");
// 					})
// 			}
// 		};

// 		ready(result)
// 	}
// }
