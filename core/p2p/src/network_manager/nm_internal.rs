use std::{
	net::{IpAddr, Ipv4Addr, SocketAddrV4},
	sync::Arc,
	time::Duration,
};

use futures_util::StreamExt;
use if_watch::{IfEvent, IfWatcher};
use quinn::{ClientConfig, Incoming, NewConnection, VarInt};
use thiserror::Error;
use tokio::{select, sync::mpsc, time::sleep};
use tracing::{debug, error, warn};
use tunnel_utils::{quic::client_config, PeerId};

use crate::{
	ConnectionType, DiscoveryStack, NetworkManager, NetworkManagerError, P2PManager, Peer,
	PeerCandidate,
};

/// Represents an event that should be handled by the [NetworkManager] event loop.
#[derive(Debug, Clone)]
pub(crate) enum NetworkManagerInternalEvent {
	Connect(PeerCandidate),
	NewKnownPeer(PeerId),
}

impl<TP2PManager: P2PManager> NetworkManager<TP2PManager> {
	// this event_loop is run in a tokio task and is responsible for handling events emitted by components of the P2P library.
	pub(crate) async fn event_loop(
		nm: &Arc<Self>,
		mut quic_incoming: Incoming,
		mut internal_channel: mpsc::UnboundedReceiver<NetworkManagerInternalEvent>,
	) -> Result<(), NetworkManagerError> {
		debug!("Starting P2P event loop");
		let mut if_watcher = IfWatcher::new()
			.await
			.map_err(NetworkManagerError::IfWatch)?;
		let discovery = DiscoveryStack::new(nm).await?;
		let (shutdown_signal_tx, mut shutdown_signal_rx) = mpsc::unbounded_channel(); // This should be able to be a oneshot but ctrlc is cringe
		ctrlc::set_handler(move || {
			debug!("Shutdown signal captured. Sending shutdown signal...");
			match shutdown_signal_tx.send(()) {
				Ok(_) => {}
				Err(err) => {
					error!(
						"Failed to send shutdown signal. Falling back to hard shutdown. {:?}",
						err
					);
				}
			}
		})?;

		for iface in if_watcher.iter() {
			Self::handle_ifwatch_event(nm, IfEvent::Up(*iface));
		}

		discovery.register().await;

		debug!(
			"Network adapters discovered on startup: {:?}",
			nm.lan_addrs
				.iter()
				.map(|a| a.to_string())
				.collect::<Vec<_>>()
				.join(",")
		);

		let nm = nm.clone();
		tokio::spawn(async move {
			loop {
				// TODO: Deal with `discovery.register`'s network calls blocking the main event loop
				select! {
					conn = quic_incoming.next() => match conn {
						Some(conn) => {
							debug!("Handling incoming QUIC connection");
							nm.clone().handle_connection(conn)
						},
						None => break,
					},
					event = Pin::new(&mut if_watcher) => {
						match event {
							Ok(event) => {
								debug!("Handling ifwatch event: {:?}", event);
								if Self::handle_ifwatch_event(&nm, event) {
									discovery.register().await;
								}
							},
							Err(_) => break,
						}
					}
					_ = discovery.mdns.handle_mdns_event() => {}
					_ = sleep(Duration::from_secs(15 * 60 /* 15 Minutes */)) => {
						debug!("Discovery service registration timer reached");
						discovery.register().await;
					}
					// TODO: Maybe use subscription system instead of polling or review this timeout!
					_ = sleep(Duration::from_secs(60 /* 1 minute */)) => {
						debug!("Discovery service pool timer reached");
						discovery.global.poll().await; // TODO: this does network calls and blocks. Is this ok?
					}
					event = internal_channel.recv() => {
						debug!("Received internal event: {:?}", event);
						let event = match event {
							Some(event) => event,
							None => {
								error!("internal_channel has been closed, stopping p2p event loop!");
								break;
							},
						};

						match event {
							NetworkManagerInternalEvent::Connect(peer) => {
								Self::connect_to_peer(&nm, peer).await;
							}
							NetworkManagerInternalEvent::NewKnownPeer(peer_id) => {
								if let Some(peer) = nm.get_discovered_peer(&peer_id) {
									Self::connect_to_peer(&nm, peer).await;
								}
							}
						}
					}
					_ = shutdown_signal_rx.recv() => {
						debug!("Event loop received shutdown signal. Shutting down...");
						nm.endpoint.close(VarInt::from_u32(69 /* TODO */), b"BRUH");
						discovery.shutdown();
						debug!("P2P event loop shutdown");
						return; // Shutdown p2p manager thread as program is exitting
					}
				};
			}
		});
		Ok(())
	}

	fn handle_ifwatch_event(nm: &Arc<Self>, event: IfEvent) -> bool {
		match event {
			IfEvent::Up(iface) => {
				let ip = match iface.addr() {
					IpAddr::V4(ip) if ip != Ipv4Addr::LOCALHOST => ip,
					_ => return false, // Currently IPv6 is not supported. Support will likely be added in the future.
				};
				nm.lan_addrs.insert(ip)
			}
			IfEvent::Down(iface) => {
				let ip = match iface.addr() {
					IpAddr::V4(ip) if ip != Ipv4Addr::LOCALHOST => ip,
					_ => return false, // Currently IPv6 is not supported. Support will likely be added in the future.
				};
				nm.lan_addrs.remove(&ip).is_some()
			}
		}
	}

	async fn connect_to_peer(nm: &Arc<Self>, peer: PeerCandidate) {
		tracing::debug!("Connecting to peer: {:?}", peer);
		let metadata = peer.metadata.clone();
		let peer_id = peer.id.clone();
		if nm.is_peer_connected(&peer.id) && nm.peer_id <= peer.id {
			return;
		}

		let NewConnection {
			connection,
			bi_streams,
			..
		} = match Self::connect_to_peer_internal(nm, peer).await {
			Ok(connection) => connection,
			Err(e) => {
				warn!("failed to connect to peer {:?}: {:?}", peer_id, e);
				return;
			}
		};

		if nm.is_peer_connected(&peer_id) && nm.peer_id <= peer_id {
			debug!(
				"Closing new connection to peer '{}' as we are already connect!",
				peer_id
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return;
		}

		match Peer::new(
			ConnectionType::Client,
			peer_id,
			connection,
			metadata,
			nm.clone(),
		)
		.await
		{
			Ok(peer) => {
				tokio::spawn(peer.handler(bi_streams));
			}
			Err(e) => {
				error!("failed to create peer: {:?}", e);
			}
		}
	}

	// TODO: Error type
	pub(crate) async fn connect_to_peer_internal(
		nm: &Arc<Self>,
		peer: PeerCandidate,
	) -> Result<NewConnection, ConnectError> {
		tracing::debug!("Attempting connection to {:?}", peer);
		// TODO: Guess the best default IP.

		let mut i = 0;
		let identity = nm.identity.clone();
		let client_config =
			ClientConfig::new(Arc::new(client_config(vec![identity.0], identity.1)?));
		loop {
			let address = match peer.addresses.get(i) {
				Some(address) => address,
				None => break None,
			};
			debug!(
				"Attempting connection to peer '{}' at address {:?}",
				peer.id, address
			);

			// TODO: Shorter timeout for connections!
			let conn = match nm.endpoint.connect_with(
				client_config.clone(),
				SocketAddrV4::new(*address, peer.port).into(),
				&peer.id.to_string(),
			) {
				Ok(conn) => conn,
				Err(e) => {
					debug!("failed to connect to addr '{:?}': {}", address, e);
					i += 1;
					continue;
				}
			};

			match conn.await {
				Ok(conn) => break Some(conn),
				Err(e) => {
					debug!("failed to connect to addr '{:?}': {}", address, e);
					i += 1;
					continue;
				}
			}
		}
		.ok_or(ConnectError::UnableToConnect)
	}
}

#[derive(Error, Debug)]
pub enum ConnectError {
	#[error("Unable to connect to peer")]
	UnableToConnect,
	#[error("error setting up client TLS")]
	TlsError(#[from] rustls::Error),
}
