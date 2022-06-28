use std::sync::Arc;

use quinn::{Connecting, NewConnection};

use crate::{NetworkManager, P2PManager};

/// is called when a new connection is received from the 'quic' listener to handle the connection.
pub(crate) fn handle_connection<TP2PManager: P2PManager>(
	nm: &Arc<NetworkManager<TP2PManager>>,
	conn: Connecting,
) {
	let nm = nm.clone();
	tokio::spawn(async move {
		let NewConnection {
			connection,
			bi_streams,
			..
		} = conn.await.unwrap();

		// //     let y = conn
		// //         .handshake_data()
		// //         .await
		// //         .unwrap()
		// //         .downcast::<HandshakeData>()
		// //         .unwrap();

		// //     println!("{:?}", y.server_name);

		// let y = connection
		// 	.peer_identity()
		// 	.unwrap()
		// 	.downcast::<Vec<Certificate>>()
		// 	.unwrap();

		// let peer_id = PeerId::from_cert(&y[0]); // TODO: handle missing [0]

		// if self
		// 	.state
		// 	.connected_peers
		// 	.read()
		// 	.await
		// 	.contains_key(&peer_id)
		// 	&& self.state.peer_id > peer_id
		// {
		// 	println!(
		// 		"Already found connection {:?}",
		// 		self.state.connected_peers.read().await
		// 	);
		// 	connection.close(VarInt::from_u32(0), b"DUP_CONN");
		// 	return;
		// }

		// let peer = Peer::new(
		// 	ConnectionType::Server,
		// 	peer_id.clone(),
		// 	connection,
		// 	self.clone(),
		// )
		// .await
		// .unwrap();
		// tokio::spawn(peer.handler(bi_streams));
	});
}

// async fn handle_stream<TP2PManager: P2PManager>(nm: Arc<NetworkManager<TP2PManager>>) {}
