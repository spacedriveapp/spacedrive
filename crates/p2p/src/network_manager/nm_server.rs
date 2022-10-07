use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use quinn::{Connecting, NewConnection, VarInt};
use rustls::Certificate;
use sd_tunnel_utils::{read_value, write_value, PeerId};
use spake2::{Ed25519Group, Password, Spake2};
use tokio::{sync::oneshot, time::sleep};
use tracing::{debug, error, info, warn};

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
				Err(err) => {
					warn!("error accepting connection: {:?}", err);
					return;
				}
			};

			// let handshake_data = connection
			// 	.handshake_data()?
			// 	.downcast::<HandshakeData>()?;

			let peer_id = match connection
				.peer_identity()
				.map(|v| v.downcast::<Vec<Certificate>>())
			{
				Some(Ok(certs)) if certs.len() == 1 => PeerId::from_cert(&certs[0]),
				Some(Ok(_)) => {
					warn!("client presenting an invalid number of certificates!");
					return;
				}
				Some(Err(_)) => {
					warn!("error decoding certificates from connection!");
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
				debug!(
					"Closing new connection to peer '{}' as we are already connect!",
					peer_id
				);
				connection.close(VarInt::from_u32(0), b"DUP_CONN");
				return;
			}

			let stream = tokio::select! {
				stream = bi_streams.next() => {
					match stream {
						Some(stream) => stream,
						None => {
							warn!("connection closed before we could read from it!");
							return;
						}
					}
				}
				_ = sleep(Duration::from_secs(1)) => {
					warn!("Connection create connection establishment stream in expected time.");
					return;
				}

			};

			match stream {
				Ok((mut tx, mut rx)) => {
					let payload = match read_value(&mut rx).await {
						Ok(msg) => msg,
						Err(err) => {
							warn!("error decoding connection establishment payload: {}", err);
							return;
						}
					};

					match payload {
						ConnectionEstablishmentPayload::ConnectionRequest => {
							debug!("ConnectionRequest from peer '{}'", peer_id);
							// TODO: Only allow peers we trust to get pass this point
						}
						ConnectionEstablishmentPayload::PairingRequest {
							pake_msg,
							metadata,
							extra_data,
						} => {
							debug!("PairingRequest from peer '{}'", peer_id);
							// TODO: Ensure we are not already paired with the peer

							let (oneshot_tx, oneshot_rx) = oneshot::channel();
							self.manager.peer_pairing_request(
								&self,
								&peer_id,
								&metadata,
								&extra_data,
								oneshot_tx,
							);

							// TODO: Have a timeout and console warning if the P2PManager doesn't respond
							let preshared_key = match oneshot_rx.await {
								Ok(Ok(preshared_key)) => preshared_key,
								Ok(Err(err)) => {
									warn!("P2PManager reported error pairing: {:?}", err);
									return;
								}
								Err(err) => {
									warn!("error receiving response for P2PManager: {:?}", err);
									return;
								}
							};

							let (spake, outgoing_pake_msg) = Spake2::<Ed25519Group>::start_b(
								&Password::new(preshared_key.as_bytes()),
								&spake2::Identity::new(peer_id.as_bytes()),
								&spake2::Identity::new(self.peer_id.as_bytes()),
							);
							match spake.finish(&pake_msg) {
								Ok(_) => {} // We only use SPAKE2 to ensure the current connection is to the peer we expect, hence we don't use the key which is returned.
								Err(err) => {
									warn!(
										"error pairing with peer. Connection has been tampered with! err: {:?}",
										err
									);
									return;
								}
							};

							let resp = match self
								.manager
								.peer_paired(
									&self,
									PairingParticipantType::Accepter,
									&peer_id,
									&metadata,
									&extra_data,
								)
								.await
							{
								Ok(_) => PairingPayload::PairingAccepted {
									pake_msg: outgoing_pake_msg,
									metadata: self.manager.get_metadata(),
								},
								Err(err) => {
									warn!("p2p manager error: {:?}", err);
									PairingPayload::PairingFailed
								}
							};

							match write_value(&mut tx, &resp).await {
								Ok(_) => {}
								Err(err) => {
									warn!("error encoding and sending pairing response: {}", err);
									return;
								}
							};

							let payload = match read_value(&mut rx).await {
								Ok(payload) => payload,
								Err(err) => {
									warn!("error reading and decoding pairing payload: {}", err);
									return;
								}
							};

							match payload {
								PairingPayload::PairingAccepted { .. } => {
									todo!("invalid") // TODO: Remove this
								}
								PairingPayload::PairingComplete { .. } => {
									info!("Pairing with peer '{}' complete.", peer_id);
								}
								PairingPayload::PairingFailed => {
									error!("Pairing with peer '{}' complete.", peer_id);

									// TODO
									// self.manager
									// 			.peer_paired_rollback(&self, &remote_peer_id, &extra_data)
									// 			.await;

									// TODO: emit event to frontend

									return;
								}
							}

							match Peer::new(
								ConnectionType::Server,
								peer_id.clone(),
								connection,
								metadata,
								self,
							)
							.await
							{
								Ok(peer) => {
									tokio::spawn(peer.handler(bi_streams));
								}
								Err(err) => {
									error!("p2p warning: error creating peer: {:?}", err);
								}
							}
						}
					}
				}
				_ => {
					error!("connection from peer '{}' didn't send establishment payload fast enough. Closing connection", peer_id);
				}
			}
		});
	}
}
