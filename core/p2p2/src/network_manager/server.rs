use std::sync::Arc;

use quinn::{crypto::rustls::HandshakeData, Connecting, NewConnection, VarInt};
use rustls::Certificate;
use sd_tunnel_utils::PeerId;

use crate::{ConnectionType, NetworkManager, P2PManager, Peer, PeerMetadata};

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
		} = match conn.await {
			Ok(conn) => conn,
			Err(e) => {
				println!("p2p warning: error accepting connection");
				return;
			}
		};

		let handshake_data = connection
			.handshake_data()
			.unwrap()
			.downcast::<HandshakeData>()
			.unwrap();

		let peer_id = match connection
			.peer_identity()
			.map(|v| v.downcast::<Vec<Certificate>>())
		{
			Some(Ok(certs)) if certs.len() == 1 => PeerId::from_cert(&certs[0]),
			Some(Ok(_)) => {
				println!("p2p warning: client presenting an invalid number of certificates!");
				return;
			}
			Some(Err(_)) => {
				println!("p2p warning: error decoding certificates from connection!");
				return;
			}
			_ => unimplemented!(),
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

		if nm.is_peer_connected(&peer_id) && nm.peer_id > peer_id {
			println!(
				"Closing new connection to peer '{}' as we are already connect!",
				peer_id
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return;
		}

		let peer = Peer::new(
			ConnectionType::Server,
			peer_id.clone(),
			connection,
			PeerMetadata {
				/* TODO: Negotiate this with remote client */
				name: "todo".into(),
				version: None,
			},
			nm,
		)
		.await
		.unwrap();
		tokio::spawn(peer.handler(bi_streams));
	});
}
