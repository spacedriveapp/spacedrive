use futures::{channel::oneshot, future::BoxFuture, prelude::*};
use libp2p::{
	core::upgrade::{InboundUpgrade, OutboundUpgrade, UpgradeInfo},
	swarm::NegotiatedSubstream,
};
use smallvec::SmallVec;
use std::{fmt, io};

use crate::spacetime::{Codec, RequestId, SpaceTimeCodec};

/// The level of support for a particular protocol.
#[derive(Debug, Clone)]
pub enum ProtocolSupport {
	/// The protocol is only supported for inbound requests.
	Inbound,
	/// The protocol is only supported for outbound requests.
	Outbound,
	/// The protocol is supported for inbound and outbound requests.
	Full,
}

impl ProtocolSupport {
	/// Whether inbound requests are supported.
	pub fn inbound(&self) -> bool {
		match self {
			ProtocolSupport::Inbound | ProtocolSupport::Full => true,
			ProtocolSupport::Outbound => false,
		}
	}

	/// Whether outbound requests are supported.
	pub fn outbound(&self) -> bool {
		match self {
			ProtocolSupport::Outbound | ProtocolSupport::Full => true,
			ProtocolSupport::Inbound => false,
		}
	}
}

/// Response substream upgrade protocol.
///
/// Receives a request and sends a response.
// #[derive(Debug)]
pub struct ResponseProtocol {
	pub(crate) codec: SpaceTimeCodec,
	pub(crate) protocols: SmallVec<[<SpaceTimeCodec as Codec>::Protocol; 2]>,
	pub(crate) request_sender: oneshot::Sender<(RequestId, <SpaceTimeCodec as Codec>::Request)>,
	pub(crate) response_receiver: oneshot::Receiver<<SpaceTimeCodec as Codec>::Response>,
	pub(crate) request_id: RequestId,
}

impl UpgradeInfo for ResponseProtocol {
	type Info = <SpaceTimeCodec as Codec>::Protocol;
	type InfoIter = smallvec::IntoIter<[Self::Info; 2]>;

	fn protocol_info(&self) -> Self::InfoIter {
		self.protocols.clone().into_iter()
	}
}

impl InboundUpgrade<NegotiatedSubstream> for ResponseProtocol {
	type Output = bool;
	type Error = io::Error;
	type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

	fn upgrade_inbound(
		mut self,
		mut io: NegotiatedSubstream,
		protocol: Self::Info,
	) -> Self::Future {
		async move {
			let read = self.codec.read_request(&protocol, &mut io);
			let request = read.await?;
			match self.request_sender.send((self.request_id, request)) {
				Ok(()) => {}
				Err(_) => {
					panic!("Expect request receiver to be alive i.e. protocol handler to be alive.",)
				}
			}

			if let Ok(response) = self.response_receiver.await {
				let write = self.codec.write_response(&protocol, &mut io, response);
				write.await?;

				io.close().await?;
				// Response was sent. Indicate to handler to emit a `ResponseSent` event.
				Ok(true)
			} else {
				io.close().await?;
				// No response was sent. Indicate to handler to emit a `ResponseOmission` event.
				Ok(false)
			}
		}
		.boxed()
	}
}

/// Request substream upgrade protocol.
///
/// Sends a request and receives a response.
pub struct RequestProtocol {
	pub(crate) codec: SpaceTimeCodec,
	pub(crate) protocols: SmallVec<[<SpaceTimeCodec as Codec>::Protocol; 2]>,
	pub(crate) request_id: RequestId,
	pub(crate) request: <SpaceTimeCodec as Codec>::Request,
}

impl fmt::Debug for RequestProtocol {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("RequestProtocol")
			.field("request_id", &self.request_id)
			.finish()
	}
}

impl UpgradeInfo for RequestProtocol {
	type Info = <SpaceTimeCodec as Codec>::Protocol;
	type InfoIter = smallvec::IntoIter<[Self::Info; 2]>;

	fn protocol_info(&self) -> Self::InfoIter {
		self.protocols.clone().into_iter()
	}
}

impl OutboundUpgrade<NegotiatedSubstream> for RequestProtocol {
	type Output = <SpaceTimeCodec as Codec>::Response;
	type Error = io::Error;
	type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

	fn upgrade_outbound(
		mut self,
		mut io: NegotiatedSubstream,
		protocol: Self::Info,
	) -> Self::Future {
		async move {
			let write = self.codec.write_request(&protocol, &mut io, self.request);
			write.await?;
			io.close().await?;
			let read = self.codec.read_response(&protocol, &mut io);
			let response = read.await?;
			Ok(response)
		}
		.boxed()
	}
}
