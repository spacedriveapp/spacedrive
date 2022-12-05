use std::{
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures_util::{AsyncRead, AsyncWrite};

use crate::{ConnectionStream, PeerId, State, TransportConnection};

use super::Transport;

/// will log the data flowing through it. This is designed to demonstrate the transport API and also to be useful for debugging.
pub struct LogTransport<T: Transport> {
    next: T,
}

impl<T: Transport> LogTransport<T> {
    /// Create a new `LogTransport` that wraps the given `Transport`.
    pub fn new(next: T) -> Self {
        Self { next }
    }
}

impl<T: Transport> Transport for LogTransport<T> {
    type State = T::State;
    type RawConn = T::RawConn;
    type EstablishError = T::EstablishError;
    type ListenStreamError = T::ListenStreamError;
    type ListenStreamItem = T::ListenStreamItem;
    type ListenStream = T::ListenStream;
    type Connection = LogConnection<T::Connection>;

    fn listen(&mut self, state: Arc<State>) -> (Self::ListenStream, Self::State) {
        self.next.listen(state)
    }

    fn listen_addr(&self, state: Self::State) -> SocketAddr {
        self.next.listen_addr(state)
    }

    fn establish(
        &self,
        state: Self::State,
        addr: SocketAddr,
    ) -> Result<Self::ListenStreamItem, Self::EstablishError> {
        self.next.establish(state, addr)
    }

    fn accept(&self, state: Self::State, conn: Self::RawConn) -> Self::Connection {
        LogConnection {
            next: self.next.accept(state, conn),
        }
    }
}

pub struct LogConnection<T: TransportConnection> {
    next: T,
}

impl<T: TransportConnection> TransportConnection for LogConnection<T> {
    type Error = T::Error;
    type RawStream = T::RawStream;
    type ListenStream = T::ListenStream;
    type Stream = LogStream<T::Stream>;
    type StreamFuture = T::StreamFuture;

    fn listen(&mut self) -> Self::ListenStream {
        self.next.listen()
    }

    fn stream(&self) -> Self::StreamFuture {
        self.next.stream()
    }

    fn accept_stream(&self, stream: Self::RawStream) -> Self::Stream {
        LogStream {
            next: self.next.accept_stream(stream),
        }
    }

    fn peer_id(&self) -> Result<PeerId, String> {
        self.next.peer_id()
    }

    fn remote_addr(&self) -> SocketAddr {
        self.next.remote_addr()
    }

    fn close(self) {
        self.next.close()
    }
}

pub struct LogStream<T: AsyncWrite + AsyncRead> {
    next: T,
}

impl<T: ConnectionStream> AsyncRead for LogStream<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let res = Pin::new(&mut self.next).poll_read(cx, buf);
        if let Poll::Ready(Ok(n)) = res {
            println!("Read {} bytes", n);
        }
        res
    }
}

impl<T: ConnectionStream> AsyncWrite for LogStream<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        println!("Writing {} bytes", buf.len());
        Pin::new(&mut self.next).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.next).poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.next).poll_close(cx)
    }
}
