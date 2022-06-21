use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use futures_util::StreamExt;
use quinn::{Connecting, Endpoint, NewConnection, VarInt};
use rustls::Certificate;
use tokio::sync::mpsc;

use crate::{
	quic, ConnectionType, DiscoveryManager, GlobalDiscovery, Identity, NetworkManagerError,
	NetworkManagerEvent, NetworkManagerState, P2PApplication, Peer, PeerCandidate, PeerId,
};

/// NetworkManager is used to manage the P2P networking between cores. This implementation is completely decoupled from the Spacedrive core to aid future refactoring and unit testing.
pub struct NetworkManager {
	/// TODO
	pub(crate) state: Arc<NetworkManagerState>,
	/// TODO
	pub(crate) discovery: Arc<DiscoveryManager>,
	/// listen_addr is the address that the NetworkManager will listen on for incoming connections.
	listen_addr: SocketAddr,
	/// endpoint is the QUIC endpoint that the NetworkManager will use to listen for incoming connections.
	endpoint: Endpoint,
}

impl NetworkManager {
	/// Create a new NetworkManager. The 'app_name' argument must be alphanumeric.
	pub async fn new<TApplication: P2PApplication + Send + Sync + 'static>(
		app_name: &'static str,
		p2p_application: TApplication,
		identity: Identity,
		application_channel: mpsc::Sender<NetworkManagerEvent>,
	) -> Result<Arc<Self>, NetworkManagerError> {
		if !app_name.chars().all(char::is_alphanumeric) {
			return Err(NetworkManagerError::InvalidAppName);
		}

		let identity = identity.into_rustls();
		let state = Arc::new(NetworkManagerState {
			peer_id: PeerId::from_cert(&identity.0),
			identity: identity.clone(),
			application_channel,
			connected_peers: Default::default(),
			p2p_application: Arc::new(p2p_application),
		});
		let (endpoint, mut incoming, listen_addr) =
			quic::new_server(identity, state.p2p_application.clone())?;

		// TODO
		let config = GlobalDiscovery::load_from_env();
		config
			.do_client_announcement(endpoint.clone())
			.await
			.unwrap();
		unimplemented!();
		// END TODO

		let discovery = DiscoveryManager::new(
			app_name,
			state.clone(),
			listen_addr,
			state.p2p_application.clone(),
		)
		.await?;

		let this = Arc::new(Self {
			state,
			discovery,
			listen_addr,
			endpoint,
		});

		let this2 = this.clone();
		tokio::spawn(async move {
			loop {
				if let Some(conn) = incoming.next().await {
					tokio::spawn(this2.clone().handle_server_connection(conn));
				}
			}
		});

		Ok(this)
	}

	/// handle_server_connection is called when a new connection is received from the 'quic' listener.
	async fn handle_server_connection(self: Arc<Self>, conn: Connecting) {
		let NewConnection {
			connection,
			bi_streams,
			..
		} = conn.await.unwrap();

		//     let y = conn
		//         .handshake_data()
		//         .await
		//         .unwrap()
		//         .downcast::<HandshakeData>()
		//         .unwrap();

		//     println!("{:?}", y.server_name);

		let y = connection
			.peer_identity()
			.unwrap()
			.downcast::<Vec<Certificate>>()
			.unwrap();

		let peer_id = PeerId::from_cert(&y[0]); // TODO: handle missing [0]

		if self
			.state
			.connected_peers
			.read()
			.await
			.contains_key(&peer_id)
			&& self.state.peer_id > peer_id
		{
			println!(
				"Already found connection {:?}",
				self.state.connected_peers.read().await
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return;
		}

		let peer = Peer::new(
			ConnectionType::Server,
			peer_id.clone(),
			connection,
			self.clone(),
		)
		.await
		.unwrap();
		tokio::spawn(peer.handler(bi_streams));
	}

	/// connect will initiate a connection to the given peer if it is available.
	pub async fn connect(self: &Arc<Self>, peer: PeerCandidate) -> Result<(), ()> {
		// TODO: Skip creating new connection if one already exists

		let NewConnection {
			connection,
			bi_streams,
			..
		} = quic::new_client(&self.endpoint, self.state.identity.clone(), peer.clone())
			.await
			.map_err(|err| {
				println!("Error: {:?}", err);
				()
			})?;

		if self
			.state
			.connected_peers
			.read()
			.await
			.contains_key(&peer.id)
			&& self.state.peer_id <= peer.id
		{
			println!(
				"Already found connection {:?}",
				self.state.connected_peers.read().await
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return Ok(());
		}

		let peer = Peer::new(ConnectionType::Client, peer.id, connection, self.clone())
			.await
			.unwrap();
		tokio::spawn(peer.handler(bi_streams));

		Ok(())
	}

	/// peer_id returns the peer ID of the current node. These are unique identifier derived from the nodes public key.
	pub fn peer_id(&self) -> PeerId {
		self.state.peer_id.clone()
	}

	/// listen_addr returns the address that the NetworkManager will listen on for incoming connections from other peers.
	pub fn listen_addr(&self) -> SocketAddr {
		self.listen_addr
	}

	/// connected_peers returns a list of the connected peers.
	pub async fn connected_peers(&self) -> HashMap<PeerId, Peer> {
		self.state.connected_peers.read().await.clone()
	}

	/// discovered_peers returns a list of the discovered peers.
	pub fn discovered_peers(&self) -> HashMap<PeerId, PeerCandidate> {
		self.discovery
			.discovered_peers
			.clone()
			.into_iter()
			.collect()
	}
}
