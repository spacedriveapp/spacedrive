use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use futures_util::StreamExt;
use quinn::{Connecting, Endpoint, NewConnection, VarInt};
use rustls::{Certificate, PrivateKey};
use tokio::sync::{mpsc, oneshot, RwLock};

use crate::{
	quic, ConnectionType, NetworkManagerError, NetworkManagerEvent, P2PApplication, Peer, PeerId,
};

/// TODO
///
/// TODO: move data off this struct cause it's name is weird for the way it is used and the data it holds.
pub(crate) struct Server {
	/// PeerId is the unique identifier of the current node.
	pub(crate) peer_id: PeerId,
	/// identity is the TLS identity of the current node.
	pub(crate) identity: (Certificate, PrivateKey),
	/// listen_addr is the address that the NetworkManager will listen on for incoming connections.
	pub(crate) listen_addr: SocketAddr,
	/// application_channel is the channel that the NetworkManager will send events to so the application embedded the networking layer can react.
	pub(crate) application_channel: mpsc::Sender<NetworkManagerEvent>,
	/// connected_peers is a map of all the peers that have an established connection with the current node.
	pub(crate) connected_peers: RwLock<HashMap<PeerId, Peer>>, // TODO: Move back to DashMap????
	/// endpoint is the QUIC endpoint that the NetworkManager will use to listen for incoming connections.
	pub(crate) endpoint: Endpoint,
	// p2p_application is a trait implemented by the application embedded the network manager. This allows the application to take control of the actions of the network manager.
	pub(crate) p2p_application: Arc<dyn P2PApplication + Send + Sync>,
}

impl Server {
	pub(crate) fn new(
		identity: (Certificate, PrivateKey),
		application_channel: mpsc::Sender<NetworkManagerEvent>,
		p2p_application: Arc<dyn P2PApplication + Send + Sync>,
	) -> Result<Arc<Self>, NetworkManagerError> {
		let peer_id = PeerId::from_cert(&identity.0);
		let (endpoint, mut incoming, listen_addr) =
			quic::new_server(identity.clone(), p2p_application.clone())?;

		let this = Arc::new(Self {
			peer_id,
			identity,
			listen_addr,
			application_channel,
			connected_peers: Default::default(),
			endpoint,
			p2p_application,
		});

		let this2 = this.clone();
		tokio::spawn(async move {
			loop {
				if let Some(conn) = incoming.next().await {
					tokio::spawn(this2.clone().handle_connection(conn));
				}
			}
		});

		Ok(this)
	}

	/// handle_connection is called when a new connection is received from the 'quic' listener.
	async fn handle_connection(self: Arc<Self>, mut conn: Connecting) {
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

		// TODO: Remove this event
		// let (tx, rx) = oneshot::channel();
		// self.application_channel
		// 	.send(NetworkManagerEvent::ConnectionRequest {
		// 		peer_id: peer_id.clone(),
		// 		resp: tx,
		// 	})
		// 	.await
		// 	.unwrap();

		// if !rx.await.unwrap() {
		// 	panic!("TODO");
		// }

		// self.connected_peers.insert();
		//     self.application_channel
		//         .send(NetworkManagerEvent::ConnectionEstablished { peer })
		//         .await
		//         .unwrap(); // TODO: Use oneshot channel to get response

		if self.connected_peers.read().await.contains_key(&peer_id) && self.peer_id > peer_id {
			println!(
				"Already found connection {:?}",
				self.connected_peers.read().await
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return;
		}

		let peer = Peer::new(ConnectionType::Server, peer_id, connection).unwrap();
		tokio::spawn(peer.handler(bi_streams, self.clone()));
	}
}
