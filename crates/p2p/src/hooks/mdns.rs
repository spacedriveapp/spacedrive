//! mDNS-based service discovery.
//!
//! This uses [mdns-sd](https://docs.rs/mdns-sd) under the hood.

use std::{
	collections::HashMap, net::SocketAddr, pin::Pin, str::FromStr, sync::Arc, time::Duration,
};

use flume::{bounded, Receiver};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::time::{sleep_until, Instant, Sleep};
use tracing::{error, trace, warn};

use crate::{HookEvent, HookId, PeerConnectionCandidate, RemoteIdentity, ShutdownGuard, P2P};

/// The time between re-advertising the mDNS service.
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute re-advertise

/// Multicast DNS (mDNS) is used for discovery of peers over local networks.
#[derive(Debug)]
pub struct Mdns {
	p2p: Arc<P2P>,
	hook_id: HookId,
}

impl Mdns {
	pub fn spawn(p2p: Arc<P2P>) -> Result<Self, mdns_sd::Error> {
		let (tx, rx) = bounded(15);
		let hook_id = p2p.register_hook("mdns", tx);

		start(p2p.clone(), hook_id, rx)?;

		Ok(Self { p2p, hook_id })
	}

	// pub fn is_discovered_by(&self, identity: &RemoteIdentity) -> bool {
	// 	self.p2p
	// 		.peers()
	// 		.get(identity)
	// 		.map(|p| p.discovered_by().contains(&self.hook_id))
	// 		.unwrap_or(false)
	// }

	// pub fn is_connected_with(&self, identity: &RemoteIdentity) -> bool {
	// 	self.p2p
	// 		.peers()
	// 		.get(identity)
	// 		.map(|p| p.is_connected_with_hook(self.hook_id))
	// 		.unwrap_or(false)
	// }

	pub async fn shutdown(self) {
		self.p2p.unregister_hook(self.hook_id).await;
	}
}

struct State {
	hook_id: HookId,
	p2p: Arc<P2P>,
	service_domain: String,
	service_name: String,
	mdns_daemon: ServiceDaemon,
	next_mdns_advertisement: Pin<Box<Sleep>>,
}

fn start(p2p: Arc<P2P>, hook_id: HookId, rx: Receiver<HookEvent>) -> Result<(), mdns_sd::Error> {
	let service_domain = format!("_{}._udp.local.", p2p.app_name());
	let mut state = State {
		hook_id,
		service_name: format!("{}.{service_domain}", p2p.remote_identity()),
		service_domain,
		p2p,
		mdns_daemon: ServiceDaemon::new()?,
		next_mdns_advertisement: Box::pin(sleep_until(
			Instant::now() + MDNS_READVERTISEMENT_INTERVAL,
		)),
	};
	let mdns_service = state.mdns_daemon.browse(&state.service_domain)?;

	tokio::spawn(async move {
		loop {
			tokio::select! {
				Ok(event) = rx.recv_async() => match event {
					HookEvent::MetadataModified | HookEvent::ListenerRegistered(_) | HookEvent::ListenerAddrAdded(_, _) | HookEvent::ListenerAddrRemoved(_, _) | HookEvent::ListenerUnregistered(_)  => advertise(&mut state),
					HookEvent::Shutdown { _guard } => {
						shutdown(_guard, &mut state);
						break;
					},
					_ => continue,
				},
				_ = &mut state.next_mdns_advertisement => advertise(&mut state),
				Ok(event) = mdns_service.recv_async() => on_event(&state, event)
			};
		}
	});

	Ok(())
}

fn advertise(state: &mut State) {
	let mut ports_to_service = HashMap::new();
	for addr in state.p2p.listeners().iter().flat_map(|l| l.addrs.clone()) {
		ports_to_service
			.entry(addr.port())
			.or_insert_with(Vec::new)
			.push(addr.ip());
	}

	let meta = state.p2p.metadata().clone();
	for (port, ips) in ports_to_service {
		let service = ServiceInfo::new(
			&state.service_domain,
			&state.p2p.remote_identity().to_string(),
			&state.service_name,
			&*ips,
			port,
			// TODO: If a piece of metadata overflows a DNS record take care of splitting it across multiple.
			Some(meta.clone()),
		)
		.map(|s| s.enable_addr_auto());

		let service = match service {
			Ok(service) => service,
			Err(err) => {
				warn!("error creating mdns service info: {}", err);
				continue;
			}
		};

		trace!("advertising mdns service: {:?}", service);
		match state.mdns_daemon.register(service) {
			Ok(()) => {}
			Err(err) => warn!("error registering mdns service: {}", err),
		}
	}

	state.next_mdns_advertisement =
		Box::pin(sleep_until(Instant::now() + MDNS_READVERTISEMENT_INTERVAL));
}

fn on_event(state: &State, event: ServiceEvent) {
	match event {
		ServiceEvent::ServiceResolved(info) => {
			let Some(identity) = fullname_to_identity(state, info.get_fullname()) else {
				return;
			};

			state.p2p.clone().discover_peer(
				state.hook_id,
				identity,
				info.get_properties()
					.iter()
					.map(|p| (p.key().to_string(), p.val_str().to_string()))
					.collect(),
				info.get_addresses()
					.iter()
					.map(|addr| {
						PeerConnectionCandidate::SocketAddr(SocketAddr::new(*addr, info.get_port()))
					})
					.collect(),
			);
		}
		ServiceEvent::ServiceRemoved(_, fullname) => {
			let Some(identity) = fullname_to_identity(state, &fullname) else {
				return;
			};

			if let Some(peer) = state.p2p.peers().get(&identity) {
				peer.undiscover_peer(state.hook_id);
			}
		}
		ServiceEvent::SearchStarted(_)
		| ServiceEvent::SearchStopped(_)
		| ServiceEvent::ServiceFound(_, _) => {}
	}
}

fn fullname_to_identity(
	State {
		p2p,
		service_domain,
		..
	}: &State,
	fullname: &str,
) -> Option<RemoteIdentity> {
	let Some(identity) = fullname
		.strip_suffix(service_domain)
		.map(|s| &s[0..s.len() - 1])
	else {
		warn!(
			"resolved peer advertising itself with an invalid fullname '{}'",
			fullname
		);
		return None;
	};

	let Ok(identity) = RemoteIdentity::from_str(identity) else {
		warn!("resolved peer advertising itself with an invalid remote identity '{identity}'");
		return None;
	};

	// Prevent discovery of the current peer.
	if identity == p2p.remote_identity() {
		return None;
	}

	Some(identity)
}

fn shutdown(_guard: ShutdownGuard, state: &mut State) {
	if let Ok(chan) = state
		.mdns_daemon
		.unregister(&state.service_name)
		.map_err(|err| {
			error!(
				"error removing mdns service '{}': {err}",
				state.service_name
			);
		}) {
		let _ = chan.recv();
	};

	// TODO: Without this mDNS is not sending it goodbye packets without a timeout. Try and remove this cause it makes shutdown slow.
	std::thread::sleep(Duration::from_millis(100));

	match state.mdns_daemon.shutdown() {
		Ok(chan) => {
			let _ = chan.recv();
		}
		Err(err) => {
			error!("error shutting down mdns daemon: {err}");
		}
	}
}
