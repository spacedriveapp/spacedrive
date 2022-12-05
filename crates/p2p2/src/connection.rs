//! Connection: TODO

use std::{marker::PhantomData, net::SocketAddr};

use serde::Serialize;

use crate::{PeerId, Stream, TransportConnection};

pub struct Connection<TPayload, T> {
    conn: T,
    phantom: PhantomData<TPayload>,
}

impl<TPayload, T> Connection<TPayload, T>
where
    TPayload: Serialize,
    T: TransportConnection,
{
    pub(crate) fn new(conn: T) -> Self {
        Self {
            conn,
            phantom: PhantomData,
        }
    }

    /// TODO
    pub fn peer_id(&self) -> Result<PeerId, String> {
        self.conn.peer_id()
    }

    /// TODO
    pub fn remote_addr(&self) -> SocketAddr {
        self.conn.remote_addr()
    }

    /// TODO
    pub async fn stream(&self, payload: TPayload) -> Result<Stream<T::Stream>, ()> {
        let mut stream = Stream::new(self.conn.accept_stream(self.conn.stream().await.unwrap())); // TODO: Error handling
        stream.send(payload).await.unwrap(); // TODO: Error handling
        Ok(stream)
    }

    /// TODO
    pub fn close(self) {
        self.conn.close();
    }
}
