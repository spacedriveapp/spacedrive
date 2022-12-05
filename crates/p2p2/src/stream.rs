//! Stream: TODO

use futures_util::AsyncWriteExt;
use serde::{Deserialize, Serialize};

use crate::ConnectionStream;

/// TODO
pub struct Stream<TStream> {
    stream: TStream,
    // TODO: Hold tx, rx
    // TODO: Hold stream controller
}

impl<TStream: ConnectionStream> Stream<TStream> {
    pub fn new(stream: TStream) -> Self {
        Self { stream }
    }

    // fn peer_id(&self) -> PeerId {}
    // fn remote_addr(&self) -> SocketAddr {}

    pub async fn send<T: Serialize>(&mut self, t: T) -> Result<(), ()> {
        let bytes = rmp_serde::to_vec_named(&t).unwrap();
        self.stream
            .write(&bytes[..]) // TODO: Error handling
            .await
            .unwrap(); // TODO: Error handling
        Ok(())
    }

    fn close(&self) {
        todo!();
    }
}

// impl AsyncWrite

// impl AsyncRead

// TODO: Helpers for AsyncRead/AsyncWrite + MsgPack
