use std::{future::Future, marker::PhantomData, net::SocketAddr, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};

use crate::{
	ConnectError, Connection, ConnectionManager, Identity, PeerId, State, Stream, Transport,
	TransportConnection,
};

/// TODO
pub struct Endpoint<T, THandlerFn, THandlerFut, TPayload>
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
	peer_id: PeerId,
	listen_addr: SocketAddr,
	manager: Arc<ConnectionManager<T, TPayload, THandlerFn, THandlerFut>>,
	state: Arc<State>,
	phantom: PhantomData<(THandlerFn, THandlerFut, TPayload)>,
}

impl<T, THandlerFn, THandlerFut, TPayload> Endpoint<T, THandlerFn, THandlerFut, TPayload>
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
		transport: T,
		identity: &Identity,
		handler_fn: THandlerFn,
	) -> Result<Self, T::ListenError> {
		let state = State::new();
		let (manager, listen_addr) = ConnectionManager::new(transport, state.clone(), handler_fn)?;
		Ok(Self {
			peer_id: PeerId::from_cert(identity.cert()),
			listen_addr,
			manager,
			state,
			phantom: PhantomData,
		})
	}

	/// returns the peer ID of the current peer. These are unique identifier derived from the peers public key.
	pub fn peer_id(&self) -> &PeerId {
		&self.peer_id
	}

	/// returns the address that the NetworkManager will listen on for incoming connections from other peers.
	pub fn listen_addr(&self) -> SocketAddr {
		self.listen_addr
	}

	/// returns the internal state of the endpoint.
	pub fn state(&self) -> &Arc<State> {
		&self.state
	}

	/// establish a new connection to a remote peer.
	/// Warning: This method does no connection deduplication. For now that's left to the implementer.
	pub async fn connect(
		&self,
		socket_addr: SocketAddr,
	) -> Result<Connection<TPayload, T::Connection>, ConnectError<T>> {
		self.manager.connect(socket_addr).await
	}

	/// close the endpoint and all its connections.
	pub fn close(self) {
		self.manager.close();
	}
}
