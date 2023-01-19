use futures::{future::BoxFuture, prelude::*};
use libp2p::{
	core::{
		upgrade::{
			read_length_prefixed, write_length_prefixed, InboundUpgrade, OutboundUpgrade,
			UpgradeInfo,
		},
		ProtocolName,
	},
	swarm::NegotiatedSubstream,
};
use rmp_serde::{from_slice, to_vec_named};
use std::{
	io::{self, ErrorKind},
	sync::Arc,
};

use crate::{spacetime::SpaceTimeMessage, utils::AsyncFn2, Connection};

use super::SpaceTimeState;

/// Response substream upgrade protocol.
///
/// Receives a request and sends a response.
pub struct ResponseProtocol<TMetadata, TConnFn>
where
	TMetadata: crate::Metadata,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	pub(crate) state: Arc<SpaceTimeState<TMetadata, TConnFn>>,
}

#[derive(Clone)]
#[deprecated = "todo: this is temporary. Remove it!"]
pub struct ThisIsTheProtoOrSomething();

impl ProtocolName for ThisIsTheProtoOrSomething {
	fn protocol_name(&self) -> &[u8] {
		b"/spacedrive/spacetime/1.0.0"
	}
}

impl<TMetadata, TConnFn> UpgradeInfo for ResponseProtocol<TMetadata, TConnFn>
where
	TMetadata: crate::Metadata,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	type Info = ThisIsTheProtoOrSomething; // TODO: This should probs be `Arc<SpaceTime>`
	type InfoIter = [Self::Info; 1];

	fn protocol_info(&self) -> Self::InfoIter {
		[ThisIsTheProtoOrSomething()]
	}
}

impl<TMetadata, TConnFn> InboundUpgrade<NegotiatedSubstream>
	for ResponseProtocol<TMetadata, TConnFn>
where
	TMetadata: crate::Metadata,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	type Output = ();
	type Error = io::Error;
	type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>; // TODO: Can this be a named future???

	fn upgrade_inbound(self, mut io: NegotiatedSubstream, protocol: Self::Info) -> Self::Future {
		async move {
			// TODO: Connection establishment payload + auth system here

			// TODO: Restrict the size of request that can be read to prevent Dos attacks -> Decide on a logical value for it and timeout clients that keep trying to blow the limit!
			let buf = read_length_prefixed(&mut io, 1_000_000).await?;
			if buf.is_empty() {
				// return Err(io::Error::from(ErrorKind::UnexpectedEof));
				todo!(); // TODO: Error handling
			}
			let request: SpaceTimeMessage = from_slice(&buf).unwrap();

			println!("Request: {:?}", request); // TODO: Tracing

			match request {
				SpaceTimeMessage::Establish => {
					println!("WE ESTBALISHED BI");
					// TODO: Handle authentication here by moving over the old `ConnectionEstablishmentPayload` from `p2p`
				}
				SpaceTimeMessage::Application(data) => {
					let resp = (self.state.fn_on_connect)(
						Connection {
							manager: self.state.manager.clone(),
						},
						data,
					)
					.await // TODO: Should this be spawned onto a separate task or not??? -> Which event loop it running in
					.unwrap(); // TODO: Error handling]

					let write = write_response(&mut io, SpaceTimeMessage::Application(resp));
					write.await?;
					io.close().await?;
				}
			}

			// TODO: Dispatch request to the application and write the response

			// 	match self.request_sender.send((self.request_id, request)) {
			// 		Ok(()) => {}
			// 		Err(_) => {
			// 			panic!("Expect request receiver to be alive i.e. protocol handler to be alive.",)
			// 		}
			// 	}

			// 	if let Ok(response) = self.response_receiver.await {
			// 		let write = write_response(&protocol, &mut io, response);
			// 		write.await?;

			// 		io.close().await?;
			// 		// Response was sent. Indicate to handler to emit a `ResponseSent` event.
			// 		Ok(true)
			// 	} else {
			// 		io.close().await?;
			// 		// No response was sent. Indicate to handler to emit a `ResponseOmission` event.
			// 		Ok(false)
			// 	}

			Ok(())
		}
		.boxed()
	}
}

// TODO: Can this request protocol be removed or refactored

/// Request substream upgrade protocol.
///
/// Sends a request and receives a response.
pub struct RequestProtocol {
	pub(crate) request: SpaceTimeMessage,
}

impl UpgradeInfo for RequestProtocol {
	type Info = ThisIsTheProtoOrSomething; // TODO: This should probs be `Arc<SpaceTime>`
	type InfoIter = [Self::Info; 1]; // smallvec::IntoIter<[Self::Info; 2]>;

	fn protocol_info(&self) -> Self::InfoIter {
		[ThisIsTheProtoOrSomething()]
	}
}

impl OutboundUpgrade<NegotiatedSubstream> for RequestProtocol {
	type Output = SpaceTimeMessage;
	type Error = io::Error;
	type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>; // TODO: Can this be a named future???

	fn upgrade_outbound(self, mut io: NegotiatedSubstream, protocol: Self::Info) -> Self::Future {
		async move {
			let write = write_request(&mut io, self.request);
			write.await?;
			io.close().await?;
			let read = read_response(&mut io);
			let response = read.await?;
			Ok(response)
		}
		.boxed()
	}
}

// TODO: Merge these in above

async fn read_request<T>(io: &mut T) -> io::Result<SpaceTimeMessage>
where
	T: AsyncRead + Unpin + Send,
{
	// TODO: Restrict the size of request that can be read to prevent Dos attacks -> Decide on a logical value for it and timeout clients that keep trying to blow the limit!

	let buf = read_length_prefixed(io, 1_000_000).await?;
	if buf.is_empty() {
		return Err(io::Error::from(ErrorKind::UnexpectedEof));
	}
	// TODO: error handling
	Ok(from_slice(&buf).unwrap())
}

async fn read_response<T>(io: &mut T) -> io::Result<SpaceTimeMessage>
where
	T: AsyncRead + Unpin + Send,
{
	// TODO: Restrict the size of request that can be read to prevent Dos attacks -> Decide on a logical value for it and timeout clients that keep trying to blow the limit!

	let buf = read_length_prefixed(io, 1_000_000).await?;
	if buf.is_empty() {
		return Err(io::Error::from(ErrorKind::UnexpectedEof));
	}
	// TODO: error handling
	Ok(from_slice(&buf).unwrap())
}

async fn write_request<T>(io: &mut T, data: SpaceTimeMessage) -> io::Result<()>
where
	T: AsyncWrite + Unpin + Send,
{
	// TODO: error handling
	write_length_prefixed(io, to_vec_named(&data).unwrap().as_slice()).await?;
	io.close().await?;
	Ok(())
}

async fn write_response<T>(io: &mut T, data: SpaceTimeMessage) -> io::Result<()>
where
	T: AsyncWrite + Unpin + Send,
{
	// TODO: error handling
	write_length_prefixed(io, to_vec_named(&data).unwrap().as_slice()).await?;
	io.close().await?;
	Ok(())
}
