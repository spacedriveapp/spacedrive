//! `Spacetime` is just a fancy name for the protocol which sits between libp2p and the application build on this library.

use std::io::ErrorKind;

use async_trait::async_trait;
use futures::{io, prelude::*};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    request_response::{ProtocolName, RequestResponseCodec},
};
use rmp_serde::{from_slice, to_vec, to_vec_named};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceTimeMessage {
    /// Establish the connection
    Establish,

    /// Send data on behalf of application
    Application(Vec<u8>),
}

#[derive(Clone)]
pub struct SpaceTimeProtocol();

impl ProtocolName for SpaceTimeProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/spacetime/1".as_bytes()
    }
}

#[derive(Clone)]
pub struct SpaceTimeCodec();

#[async_trait]
impl RequestResponseCodec for SpaceTimeCodec {
    type Protocol = SpaceTimeProtocol;
    type Request = SpaceTimeMessage;
    type Response = SpaceTimeMessage;

    async fn read_request<T>(
        &mut self,
        _: &SpaceTimeProtocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
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

    async fn read_response<T>(
        &mut self,
        _: &SpaceTimeProtocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
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

    async fn write_request<T>(
        &mut self,
        _: &SpaceTimeProtocol,
        io: &mut T,
        data: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // TODO: error handling
        write_length_prefixed(io, to_vec_named(&data).unwrap().as_slice()).await?;
        io.close().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &SpaceTimeProtocol,
        io: &mut T,
        data: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // TODO: error handling
        write_length_prefixed(io, to_vec_named(&data).unwrap().as_slice()).await?;
        io.close().await?;
        Ok(())
    }
}
