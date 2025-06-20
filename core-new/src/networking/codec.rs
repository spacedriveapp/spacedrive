use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use libp2p::request_response::Codec;
use libp2p::StreamProtocol;
use std::io;

/// Production codec that exchanges byte arrays for pairing protocol
#[derive(Debug, Clone, Default)]
pub struct PairingCodec;

#[async_trait]
impl Codec for PairingCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        // Read message length first (4 bytes)
        let mut len_buf = [0u8; 4];
        AsyncReadExt::read_exact(io, &mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Prevent DoS attacks with oversized messages
        if len > 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Message too large",
            ));
        }

        // Read the message data
        let mut buf = vec![0u8; len];
        AsyncReadExt::read_exact(io, &mut buf).await?;

        Ok(buf)
    }

    async fn read_response<T>(&mut self, protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        // Response uses same format as request
        self.read_request(protocol, io).await
    }

    async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, req: Self::Request) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // Write length first, then data
        let len = req.len() as u32;
        AsyncWriteExt::write_all(io, &len.to_be_bytes()).await?;
        AsyncWriteExt::write_all(io, &req).await?;
        AsyncWriteExt::flush(io).await?;

        Ok(())
    }

    async fn write_response<T>(&mut self, protocol: &Self::Protocol, io: &mut T, res: Self::Response) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // Response uses same format as request
        self.write_request(protocol, io, res).await
    }
}