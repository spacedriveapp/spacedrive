use std::{fmt, future::Future, marker::PhantomData, net::SocketAddr, sync::Arc, time::Duration};

use futures_util::{AsyncReadExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::time::sleep;
use tracing::warn;

use crate::{Connection, PeerId, State, Stream, Transport, TransportConnection};

pub struct ConnectionManager<T, TPayload, THandlerFn, THandlerFut>
where
	T: Transport,
	TPayload: Serialize + DeserializeOwned + Send + Sync + 'static,
	THandlerFn: Fn(TPayload, Stream<<T::Connection as TransportConnection>::Stream>) -> THandlerFut
		+ Clone
		+ Send
		+ Sync
		+ 'static,
	THandlerFut: Future<Output = ()> + Send + Sync + 'static,
{
	transport: T,
	endpoint_state: Arc<State>,
	state: T::State,
	phantom: PhantomData<(TPayload, THandlerFn, THandlerFut)>,
}

impl<T, TPayload, THandlerFn, THandlerFut> ConnectionManager<T, TPayload, THandlerFn, THandlerFut>
where
	T: Transport,
	TPayload: Serialize + DeserializeOwned + Send + Sync + 'static,
	THandlerFn: Fn(TPayload, Stream<<T::Connection as TransportConnection>::Stream>) -> THandlerFut
		+ Clone
		+ Send
		+ Sync
		+ 'static,
	THandlerFut: Future<Output = ()> + Send + Sync + 'static,
{
	pub fn new(
		mut transport: T,
		endpoint_state: Arc<State>,
		handler_fn: THandlerFn,
	) -> Result<(Arc<Self>, SocketAddr), T::ListenError> {
		let (stream, state) = transport.listen(endpoint_state.clone())?;
		let listen_addr = transport.listen_addr(state.clone());
		let this = Arc::new(Self {
			transport,
			endpoint_state,
			state,
			phantom: PhantomData,
		});

		// TODO: Invert it so the user spawns this for us???? -> Could make runtime agnostic?
		tokio::spawn(this.clone().event_loop(handler_fn.clone(), stream));

		Ok((this, listen_addr))
	}

	// TODO: Drop thread when `ConnectionManager` is dropped
	async fn event_loop(self: Arc<Self>, handler_fn: THandlerFn, mut stream: T::ListenStream) {
		loop {
			while let Some(conn) = stream.next().await {
				tokio::spawn(self.clone().handle_connection(handler_fn.clone(), conn));
			}
		}
	}

	// TODO: Drop thread when `ConnectionManager` is dropped
	async fn handle_connection(self: Arc<Self>, handler_fn: THandlerFn, conn: T::ListenStreamItem) {
		let mut conn = match conn.await {
			Ok(conn) => self.transport.accept(self.state.clone(), conn),
			Err(err) => {
				warn!("Failed to accept incoming connection: {:?}", err);
				return;
			}
		};

		let mut peer_id = match conn.peer_id() {
			Ok(v) => v,
			Err(err) => {
				warn!(
					"Failed to determine peer id of incoming connection: {:?}",
					err
				);
				return;
			}
		};

		let mut stream = conn.listen();
		let conn = Arc::new(conn);

		while let Some(stream) = stream.next().await {
			let stream = match stream {
				Ok(stream) => conn.accept_stream(stream),
				Err(err) => {
					warn!("Failed to accept incoming stream: {:?}", err);
					continue;
				}
			};

			// TODO: Reenable this
			// if let Some(server_name) = handshake_data.server_name {
			// 	if server_name != peer_id.to_string() {
			// 		println!("{} {}", server_name, peer_id.to_string()); // TODO: BRUH
			// 		println!(
			// 			"p2p warning: client presented a certificate and servername which don't match!"
			// 		);
			// 		return;
			// 	}
			// } else {
			// 	println!(
			// 		"p2p warning: client presented a certificate and servername which don't match!"
			// 	);
			// 	return;
			// }

			// // TODO: Do this check again before adding to array because the `ConnectionEstablishmentPayload` adds delay
			// if self.is_peer_connected(&peer_id) && self.peer_id > peer_id {
			//     debug!(
			//         "Closing new connection to peer '{}' as we are already connect!",
			//         peer_id
			//     );
			//     connection.close(VarInt::from_u32(0), b"DUP_CONN");
			//     return;
			// }

			if !self.endpoint_state.on_incoming_connection(&peer_id) {
				todo!(); // TODO
			}

			// TODO: A connection can be created and then left idle to waste resources. Should we require them to spawn a stream to verify identity?

			tokio::spawn(self.clone().handle_stream(
				handler_fn.clone(),
				peer_id.clone(),
				conn.clone(),
				stream,
			));
		}
	}

	// TODO: Drop thread when `ConnectionManager` is dropped
	async fn handle_stream(
		self: Arc<Self>,
		handler_fn: THandlerFn,
		peer_id: PeerId,
		conn: Arc<T::Connection>,
		mut stream: <T::Connection as TransportConnection>::Stream,
	) {
		// TODO: Fire off preconnect behavour hook
		if !self.endpoint_state.on_incoming_stream(&peer_id) {
			todo!(); // TODO
		}

		let mut output = [0u8; 100]; // TODO: What should this value be because it leaks to userspace with their `TPayload`
		tokio::select! {
			biased;
			bytes = stream.read(&mut output) => {
				match bytes {
					Ok(_) => {},
					Err(err) => {
						warn!("Error reading connection establishment payload: {:?}", err);
						return;
					}
				}
			}
			_ = sleep(Duration::from_secs(1 /* TODO: What should this be? */)) => {
				warn!("Timeout reading connection establishment payload");
				return;
			}
		};

		let payload = match rmp_serde::from_slice(&output) {
			Ok(v) => v,
			Err(err) => {
				warn!("Error decoding connection establishment payload: {:?}", err);
				return;
			}
		};

		// TODO: Add connection into `active_conn` map

		// We pass off control of this stream to the root application
		handler_fn(payload, Stream::new(stream)).await;
	}

	/// TODO
	// TODO: Accept `PeerId`, `SocketAddr`, `Vec<SocketAddr>`, etc -> Dealing with priority of SocketAddrs???
	pub async fn connect(
		&self,
		socket_addr: SocketAddr,
	) -> Result<Connection<TPayload, T::Connection>, ConnectError<T>> {
		Ok(Connection::new(
			self.transport.accept(
				self.state.clone(),
				self.transport
					.establish(self.state.clone(), socket_addr)
					.map_err(ConnectError::EstablishError)?
					.await
					.map_err(ConnectError::ListenStreamError)?,
			),
		))
	}

	/// TODO
	pub(crate) fn close(&self) {
		// self.end
		todo!(); // TODO: Pass off to transport
	}
}

#[derive(Error)]
pub enum ConnectError<T: Transport> {
	#[error("transport error establishing connection: {0}")]
	EstablishError(T::EstablishError),
	#[error("transport error creating stream listener: {0}")]
	ListenStreamError(T::ListenStreamError),
}

// Using derive for this impl will force the bound `T: Debug` which as shown here is unnecessary
impl<T: Transport> fmt::Debug for ConnectError<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::EstablishError(err) => write!(f, "EstablishError({:?})", err),
			Self::ListenStreamError(err) => write!(f, "ListenStreamError({:?})", err),
		}
	}
}
