use std::{
	future::{ready, Ready},
	io::ErrorKind,
};

use libp2p::{
	core::UpgradeInfo,
	futures::{AsyncReadExt, AsyncWriteExt},
	OutboundUpgrade, Stream,
};
use tokio::sync::oneshot;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::error;

use crate::spacetunnel::Identity;

use super::{SpaceTimeProtocolName, UnicastStreamBuilder, BROADCAST_DISCRIMINATOR};

#[derive(Debug)]
pub enum OutboundRequest {
	Broadcast(Vec<u8>),
	Unicast(oneshot::Sender<UnicastStreamBuilder>),
}

pub struct OutboundProtocol {
	pub(crate) application_name: String,
	pub(crate) req: OutboundRequest,
	pub(crate) identity: Identity,
}

impl UpgradeInfo for OutboundProtocol {
	type Info = SpaceTimeProtocolName;
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[SpaceTimeProtocolName(self.application_name.clone())]
	}
}

impl OutboundUpgrade<Stream> for OutboundProtocol {
	type Output = ();
	type Error = ();
	type Future = Ready<Result<(), ()>>;

	fn upgrade_outbound(self, mut io: Stream, _protocol: Self::Info) -> Self::Future {
		match self.req {
			OutboundRequest::Broadcast(data) => {
				tokio::spawn(async move {
					io.write_all(&[BROADCAST_DISCRIMINATOR]).await.unwrap();
					if let Err(err) = io.write_all(&data).await {
						// TODO: Print the peer which we failed to send to here
						error!("Error sending broadcast: {:?}", err);
					}
					io.flush().await.unwrap();

					let mut buf = [0u8; 1];
					io.read_exact(&mut buf).await.unwrap();
					debug_assert_eq!(buf[0], b'D', "Peer should let us know they were done!");

					match io.close().await {
						Ok(_) => {}
						Err(err) if err.kind() == ErrorKind::ConnectionReset => {} // The other end shut the connection before us
						Err(err) => {
							error!("Error closing broadcast stream: {:?}", err);
						}
					}
				});
			}
			OutboundRequest::Unicast(sender) => {
				// We write the discriminator to the stream in the `Manager::stream` method before returning the stream to the user to make async a tad nicer.
				sender
					.send(UnicastStreamBuilder::new(
						self.identity.clone(),
						io.compat(),
					))
					.unwrap();
			}
		}

		ready(Ok(()))
	}
}
