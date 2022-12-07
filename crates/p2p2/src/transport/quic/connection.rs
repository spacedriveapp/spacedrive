use std::net::SocketAddr;

use quinn::{
	ConnectionError, IncomingBiStreams, NewConnection, OpenBi, RecvStream, SendStream, VarInt,
};
use rustls::Certificate;

use crate::{PeerId, TransportConnection};

use super::stream::QuicStream;

pub struct QuicConnection(quinn::Connection, Option<IncomingBiStreams>);

impl QuicConnection {
	pub fn new(conn: NewConnection) -> Self {
		Self(conn.connection, Some(conn.bi_streams))
	}
}

impl TransportConnection for QuicConnection {
	type Error = ConnectionError;
	type RawStream = (SendStream, RecvStream);
	type ListenStream = IncomingBiStreams;
	type Stream = QuicStream;
	type StreamFuture = OpenBi;

	fn listen(&mut self) -> Self::ListenStream {
		match self.1.take() {
			Some(v) => v,
			None => unreachable!(),
		}
	}

	fn stream(&self) -> Self::StreamFuture {
		self.0.open_bi()
	}

	fn accept_stream(&self, stream: Self::RawStream) -> Self::Stream {
		QuicStream {
			tx: stream.0,
			rx: stream.1,
		}
	}

	fn peer_id(&self) -> Result<PeerId, String> {
		match self
            .0
            .peer_identity()
            .map(|v| v.downcast::<Vec<Certificate>>())
        {
            Some(Ok(certs)) if certs.len() == 1 => Ok(PeerId::from_cert(&certs[0])),
            Some(Ok(_)) => Err("client presented more than one valid TLS certificate. This is not supported. Rejecting connection.".into()),
            Some(Err(err)) => Err(format!("error decoding TLS certificates from connection. error: {}", err.downcast::<rustls::Error>().expect("Error downcasting to rustls error"))), // TODO: Is this error downcast correct??
            None => unreachable!(),
        }
	}

	fn remote_addr(&self) -> SocketAddr {
		self.0.remote_address()
	}

	fn close(self) {
		self.0.close(VarInt::from_u32(0), b""); // TODO: Custom reason and VarInt here
	}
}
