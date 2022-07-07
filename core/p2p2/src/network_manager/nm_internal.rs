use std::{
	net::{IpAddr, Ipv4Addr, SocketAddrV4},
	sync::Arc,
	time::Duration,
};

use futures_util::StreamExt;
use if_watch::{IfEvent, IfWatcher};
use quinn::{ClientConfig, Incoming, NewConnection, VarInt};
use sd_tunnel_utils::{quic::client_config, PeerId};
use tokio::{select, sync::mpsc, time::sleep};

use crate::{
	handle_connection, ConnectionType, GlobalDiscovery, NetworkManager, NetworkManagerError,
	P2PManager, Peer, PeerCandidate, MDNS,
};

/// TODO
#[derive(Debug, Clone)]
pub(crate) enum NetworkManagerInternalEvent {
	Connect(PeerCandidate),
	NewKnownPeer(PeerId),
}

impl<TP2PManager: P2PManager> NetworkManager<TP2PManager> {
	pub(crate) async fn event_loop(
		nm: &Arc<Self>,
		mut quic_incoming: Incoming,
		mut internal_channel: mpsc::UnboundedReceiver<NetworkManagerInternalEvent>,
	) -> Result<(), NetworkManagerError> {
		let mut if_watcher = IfWatcher::new()
			.await
			.map_err(NetworkManagerError::IfWatch)?;
		let mdns = MDNS::init(nm)?;
		let global = GlobalDiscovery::init(nm)?;
		global.poll().await;

		for iface in if_watcher.iter() {
			Self::handle_ifwatch_event(nm, IfEvent::Up(iface.clone()));
		}

		Self::register(&mdns, &global).await; // TODO: Create a discovery stack type to hold them instead of passing them all individually

		let nm = nm.clone();
		tokio::spawn(async move {
			loop {
				// TODO: Deal with `Self::register`'s network calls blocking the main event loop
				select! {
					conn = quic_incoming.next() => match conn {
						Some(conn) => handle_connection(&nm, conn),
						None => break,
					},
					event = Pin::new(&mut if_watcher) => {
						match event {
							Ok(event) => {
								if Self::handle_ifwatch_event(&nm, event) {
									Self::register(&mdns, &global).await;
								}
							},
							Err(_) => break,
						}
					}
					_ = mdns.handle_mdns_event() => {}
					_ = sleep(Duration::from_secs(15 * 60 /* 15 Minutes */)) => {
						Self::register(&mdns, &global).await;
					}
					// TODO: Maybe use subscription system instead of polling or review this timeout!
					_ = sleep(Duration::from_secs(30 /* 30 Seconds */)) => {
						global.poll().await; // TODO: this does network calls and blocks. Is this ok?
					}
					event = internal_channel.recv() => {
						let event = match event {
							Some(event) => event,
							None => {
								println!("p2p error: internal_channel has been closed, stopping p2p event loop!");
								break;
							},
						};

						match event {
							NetworkManagerInternalEvent::Connect(peer) => {
								Self::connect_to_peer(&nm, peer).await.unwrap();
							}
							NetworkManagerInternalEvent::NewKnownPeer(peer_id) => {
								if let Some(peer) = nm.get_discovered_peer(&peer_id) {
									Self::connect_to_peer(&nm, peer).await.unwrap();
								}
							}
						}
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

	pub(crate) async fn register(mdns: &MDNS<TP2PManager>, global: &GlobalDiscovery<TP2PManager>) {
		mdns.register().await;
		global.register().await;
	}

	// TODO: Error type
	async fn connect_to_peer(nm: &Arc<Self>, peer: PeerCandidate) -> Result<(), ()> {
		let metadata = peer.metadata.clone();
		let peer_id = peer.id.clone();
		if nm.is_peer_connected(&peer.id) && nm.peer_id <= peer.id {
			return Ok(());
		}

		let NewConnection {
			connection,
			bi_streams,
			..
		} = Self::connect_to_peer_internal(nm, peer).await?;

		if nm.is_peer_connected(&peer_id) && nm.peer_id <= peer_id {
			println!(
				"Closing new connection to peer '{}' as we are already connect!",
				peer_id
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return Ok(());
		}

		let peer = Peer::new(
			ConnectionType::Client,
			peer_id,
			connection,
			metadata,
			nm.clone(),
		)
		.await
		.unwrap();
		tokio::spawn(peer.handler(bi_streams));
		Ok(())
	}

	// TODO: Error type
	pub(crate) async fn connect_to_peer_internal(
		nm: &Arc<Self>,
		peer: PeerCandidate,
	) -> Result<NewConnection, ()> {
		// TODO: Guess the best default IP.

		let mut i = 0;
		let conn = loop {
			let address = match peer.addresses.get(i) {
				Some(address) => address,
				None => break None,
			};

			// TODO: Shorter timeout for connections!
			let identity = nm.identity.clone();
			let conn = match nm.endpoint.connect_with(
				ClientConfig::new(Arc::new(
					client_config(vec![identity.0], identity.1).unwrap(),
				)),
				SocketAddrV4::new(*address, peer.port).into(),
				&peer.id.to_string(),
			) {
				Ok(conn) => conn,
				Err(e) => {
					println!("p2p error: failed to connect to peer: {}", e);
					i += 1;
					continue;
				}
			};

			match conn.await {
				Ok(conn) => break Some(conn),
				Err(e) => {
					println!("p2p error: failed to connect to peer: {}", e);
					i += 1;
					continue;
				}
			}
		}
		.unwrap();

		Ok(conn)
	}

	fn shutdown() {
		// TODO: Trigger this function
		// TODO: Deannounce MDNS + Global Discovery
	}
}
