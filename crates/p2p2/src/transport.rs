//! Transport: TODO

use std::{future::Future, net::SocketAddr, sync::Arc};

use crate::{PeerId, State};
use futures_util::{AsyncRead, AsyncWrite, Stream};
use std::fmt::Debug;

mod log;
mod quic;

pub use log::*;
pub use quic::*;

/// TODO
pub trait Transport: Sized + Send + Sync + 'static {
    type State: Clone + Send + Sync;
    type RawConn;
    type EstablishError: Debug;
    type ListenStreamError: Debug;
    type ListenStreamItem: Future<Output = Result<Self::RawConn, Self::ListenStreamError>> + Send;
    type ListenStream: Stream<Item = Self::ListenStreamItem> + Unpin + Send;
    type Connection: TransportConnection + Send + Sync;

    fn listen(&mut self, state: Arc<State>) -> (Self::ListenStream, Self::State);

    fn listen_addr(&self, state: Self::State) -> SocketAddr;

    fn establish(
        &self,
        state: Self::State,
        addr: SocketAddr,
    ) -> Result<Self::ListenStreamItem, Self::EstablishError>;

    fn accept(&self, state: Self::State, conn: Self::RawConn) -> Self::Connection;
}

/// TODO
pub trait ConnectionStream: AsyncWrite + AsyncRead + Unpin {}

impl<T: AsyncWrite + AsyncRead + Unpin> ConnectionStream for T {}

/// TODO
pub trait TransportConnection {
    type Error: Debug;
    type RawStream;
    type ListenStream: Stream<Item = Result<Self::RawStream, Self::Error>> + Unpin + Send;
    type Stream: ConnectionStream + Send;
    type StreamFuture: Future<Output = Result<Self::RawStream, Self::Error>>;

    fn listen(&mut self) -> Self::ListenStream;

    fn stream(&self) -> Self::StreamFuture;

    fn accept_stream(&self, stream: Self::RawStream) -> Self::Stream;

    fn peer_id(&self) -> Result<PeerId, String>;

    fn remote_addr(&self) -> SocketAddr;

    fn close(self);
}

// impl<T: Connection> Connection for Arc<T> {}
