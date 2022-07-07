use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use quinn::{crypto::rustls::HandshakeData, Connecting, NewConnection, VarInt};
use rustls::Certificate;
use sd_tunnel_utils::PeerId;
use tokio::time::sleep;

use crate::{
	ConnectionEstablishmentPayload, ConnectionType, NetworkManager, P2PManager, PairingPayload,
	Peer, PeerMetadata,
};

// TODO: move this onto the network_manager and rename the file `nm_server.rs`

/// is called when a new connection is received from the 'quic' listener to handle the connection.
pub(crate) fn handle_connection<TP2PManager: P2PManager>(
	nm: &Arc<NetworkManager<TP2PManager>>,
	conn: Connecting,
) {
	let nm = nm.clone();
	tokio::spawn(async move {
		let NewConnection {
			connection,
			mut bi_streams,
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

		// TODO: Do this check again before adding to array because the `ConnectionEstablishmentPayload` adds delay
		if nm.is_peer_connected(&peer_id) && nm.peer_id > peer_id {
			println!(
				"Closing new connection to peer '{}' as we are already connect!",
				peer_id
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return;
		}

		tokio::select! {
			stream = bi_streams.next() => {
				match stream.unwrap() {
					Ok((mut tx, mut rx)) => {
						// TODO: Get max chunk size from constant.
						let data = rx.read_chunk(64 * 1024, true).await.unwrap().unwrap();
						let payload: ConnectionEstablishmentPayload = rmp_serde::decode::from_read(&data.bytes[..]).unwrap();

						match payload {
							ConnectionEstablishmentPayload::PairingRequest { preshared_key, metadata, extra_data } => {
								// TODO: Ensure we are not already paired with the peer

								// TODO: UI popup that pairing is happening & get password from user
								println!("TEMP: PAIRING REQUEST INCOMING FROM {}", peer_id);
								let expected_preshared_key = "very_secure".to_string(); // TODO: This is hardcoded until the UI is inplace.

								if preshared_key != expected_preshared_key {
									// TODO
									todo!();
								}

								let resp = match nm.manager.peer_paired(&nm, &peer_id, &extra_data).await {
									Ok(_) => {
										PairingPayload::PairingComplete { metadata: nm.manager.get_metadata(), }
									},
									Err(err) => {
										println!("p2p manager error: {:?}", err);
										PairingPayload::PairingFailed
									}
								};

								// rmp_serde doesn't support `AsyncWrite` so we have to allocate buffer here.
								tx.write_all(
									&rmp_serde::encode::to_vec_named(&resp)
									.unwrap(),
								)
								.await
								.unwrap();

								match resp {
									PairingPayload::PairingComplete { .. } => {
										println!("Pairing complete!");

										// TODO: Call self.manager.peer_paired??

										// TODO: This is duplicated with `ConnectionEstablishmentPayload::ConnectionRequest` fix that!
										let peer = Peer::new(
											ConnectionType::Server,
											peer_id.clone(),
											connection,
											metadata,
											nm,
										)
										.await
										.unwrap();
										tokio::spawn(peer.handler(bi_streams));
									},
									_ => {
										tx.finish().await.unwrap();
									}
								}
							}
							ConnectionEstablishmentPayload::ConnectionRequest => {
								// TODO: Only allow peers we trust to get pass this point

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
							}
						}
					},
					_ => {
						println!("p2p warning: Connection didn't send establishment payload.");
						return;
					}
				}
			}
			_ = sleep(Duration::from_secs(1)) => {
				println!("p2p warning: Connection didn't send establishment payload in expected time.");
				return;
			}
		}
	});
}
