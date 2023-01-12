// Copyright 2020 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

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

/// A `Codec` defines the request and response types
/// for a request-response [`Behaviour`](crate::Behaviour) protocol or
/// protocol family and how they are encoded / decoded on an I/O stream.
#[async_trait]
pub trait Codec {
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

#[allow(deprecated)]
#[async_trait]
impl<U> Codec for U
where
	U: RequestResponseCodec + Send,
	U::Protocol: Sync,
{
	type Protocol = U::Protocol;

	type Request = U::Request;

	type Response = U::Response;

	async fn read_request<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
	) -> io::Result<Self::Request>
	where
		T: AsyncRead + Unpin + Send,
	{
		self.read_request(protocol, io).await
	}

	async fn read_response<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
	) -> io::Result<Self::Response>
	where
		T: AsyncRead + Unpin + Send,
	{
		self.read_response(protocol, io).await
	}

	async fn write_request<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
		req: Self::Request,
	) -> io::Result<()>
	where
		T: AsyncWrite + Unpin + Send,
	{
		self.write_request(protocol, io, req).await
	}

	async fn write_response<T>(
		&mut self,
		protocol: &Self::Protocol,
		io: &mut T,
		res: Self::Response,
	) -> io::Result<()>
	where
		T: AsyncWrite + Unpin + Send,
	{
		self.write_response(protocol, io, res).await
	}
}
