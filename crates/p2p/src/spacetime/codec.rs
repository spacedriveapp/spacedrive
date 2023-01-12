use async_trait::async_trait;
use futures::prelude::*;
use libp2p::core::ProtocolName;
use std::io;

/// A `RequestResponseCodec` defines the request and response types
/// for a request-response `Behaviour` protocol or
/// protocol family and how they are encoded / decoded on an I/O stream.
#[async_trait]
pub trait RequestResponseCodec {
	/// The type of protocol(s) or protocol versions being negotiated.
	type Protocol: ProtocolName + Send + Clone;
	/// The type of inbound and outbound requests.
	type Request: Send;
	/// The type of inbound and outbound responses.
	type Response: Send;

	/// Reads a request from the given I/O stream according to the
	/// negotiated protocol.
	async fn read_request<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
	) -> io::Result<Self::Request>
	where
		T: AsyncRead + Unpin + Send;

	/// Reads a response from the given I/O stream according to the
	/// negotiated protocol.
	async fn read_response<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
	) -> io::Result<Self::Response>
	where
		T: AsyncRead + Unpin + Send;

	/// Writes a request to the given I/O stream according to the
	/// negotiated protocol.
	async fn write_request<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
		req: Self::Request,
	) -> io::Result<()>
	where
		T: AsyncWrite + Unpin + Send;

	/// Writes a response to the given I/O stream according to the
	/// negotiated protocol.
	async fn write_response<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
		res: Self::Response,
	) -> io::Result<()>
	where
		T: AsyncWrite + Unpin + Send;
}
