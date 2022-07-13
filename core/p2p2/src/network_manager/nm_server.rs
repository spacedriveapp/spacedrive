use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use quinn::{crypto::rustls::HandshakeData, Connecting, NewConnection, VarInt};
use rustls::Certificate;
use sd_tunnel_utils::PeerId;
use spake2::{Ed25519Group, Password, Spake2};
use tokio::{sync::oneshot, time::sleep};

use crate::{
	ConnectionEstablishmentPayload, ConnectionType, NetworkManager, P2PManager,
	PairingParticipantType, PairingPayload, Peer,
};

impl<TP2PManager: P2PManager> NetworkManager<TP2PManager> {
	/// is called when a new connection is received from the 'QUIC' server listener to handle the connection.
	pub(crate) fn handle_connection(self: Arc<Self>, conn: Connecting) {
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
			if self.is_peer_connected(&peer_id) && self.peer_id > peer_id {
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
								// _ => todo!(), // TODO: Remove this TODO
								ConnectionEstablishmentPayload::ConnectionRequest => {
																	// TODO: Only allow peers we trust to get pass this point
								},
								ConnectionEstablishmentPayload::PairingRequest { pake_msg, metadata, extra_data } => {
									// TODO: Ensure we are not already paired with the peer

									let (oneshot_tx, oneshot_rx) = oneshot::channel();
									self.manager.peer_pairing_request(&self, &peer_id, &metadata, &extra_data, oneshot_tx);
									let preshared_key = oneshot_rx.await.unwrap().unwrap();

									let (spake, outgoing_pake_msg) = Spake2::<Ed25519Group>::start_b(
										&Password::new(preshared_key.as_bytes()),
										&spake2::Identity::new(peer_id.as_bytes()),
										&spake2::Identity::new(self.peer_id.as_bytes())
									);
									let _spake_key = spake.finish(&pake_msg).unwrap(); // We don't use the key because this is only used to verify we can trust the connection.

									let resp = match self.manager.peer_paired(&self, PairingParticipantType::Accepter, &peer_id, &metadata, &extra_data).await {
										Ok(_) => {
											PairingPayload::PairingAccepted { pake_msg: outgoing_pake_msg, metadata: self.manager.get_metadata(), }
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

									// TODO: Get max chunk size from constant.
									let data = rx.read_chunk(64 * 1024, true).await.unwrap().unwrap();
									let payload: PairingPayload = rmp_serde::decode::from_read(&data.bytes[..]).unwrap();


									match payload {
										PairingPayload::PairingAccepted { .. } => todo!("invalid"),
										PairingPayload::PairingComplete { .. } => {
											println!("PAIRING COMPLETE");
										}
										PairingPayload::PairingFailed => {
											println!("p2p warning: pairing failed!");

											// TODO
											// self.manager
											// 			.peer_paired_rollback(&self, &remote_peer_id, &extra_data)
											// 			.await;

											// TODO: emit event to frontend

											return;
										}
									}

									let peer = Peer::new(
										ConnectionType::Server,
										peer_id.clone(),
										connection,
										metadata,
										self,
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
}
