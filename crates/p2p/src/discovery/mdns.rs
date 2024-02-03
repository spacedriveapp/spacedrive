use std::{
	collections::HashMap,
	net::SocketAddr,
	pin::Pin,
	str::FromStr,
	sync::PoisonError,
	task::{Context, Poll},
	thread::sleep,
	time::Duration,
};

use futures_core::Stream;
use libp2p::{
	futures::{FutureExt, StreamExt},
	PeerId,
};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use streamunordered::{StreamUnordered, StreamYield};
use tokio::time::{sleep_until, Instant, Sleep};
use tracing::{error, trace, warn};

use crate::{
	spacetunnel::RemoteIdentity, DiscoveredPeerCandidate, ListenAddrs, ServiceEventInternal, State,
};

/// TODO
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute re-advertise

pub struct Mdns {
	identity: RemoteIdentity,
	peer_id: PeerId,
	service_name: String,
	advertised_services: Vec<String>,
	mdns_daemon: ServiceDaemon,
	next_mdns_advertisement: Pin<Box<Sleep>>,
	// This is an ugly workaround for: https://github.com/keepsimple1/mdns-sd/issues/145
	mdns_rx: StreamUnordered<MdnsRecv>,
	// This is hacky but it lets us go from service name back to `RemoteIdentity` when removing the service.
	// During service removal we only have the service name (not metadata) but during service discovery we insert into this map.
	tracked_services: HashMap<String /* Service FQDN */, TrackedService>,
}

struct TrackedService {
	service_name: String,
	identity: RemoteIdentity,
}

impl Mdns {
	pub(crate) fn new(
		application_name: &'static str,
		identity: RemoteIdentity,
		peer_id: PeerId,
	) -> Result<Self, mdns_sd::Error> {
		let mdns_daemon = ServiceDaemon::new()?;

		Ok(Self {
			identity,
			peer_id,
			service_name: format!("_{application_name}._udp.local."),
			advertised_services: Vec::new(),
			mdns_daemon,
			next_mdns_advertisement: Box::pin(sleep_until(Instant::now())), // Trigger an advertisement immediately
			mdns_rx: StreamUnordered::new(),
			tracked_services: HashMap::new(),
		})
	}

	/// Do an mdns advertisement to the network.
	pub(super) fn do_advertisement(&mut self, listen_addrs: &ListenAddrs, state: &State) {
		trace!("doing mDNS advertisement!");

		// TODO: Second stage rate-limit

		let mut ports_to_service = HashMap::new();
		for addr in listen_addrs {
			ports_to_service
				.entry(addr.port())
				.or_insert_with(Vec::new)
				.push(addr.ip());
		}

		// This method takes `&mut self` so we know we have exclusive access to `advertised_services`
		let mut advertised_services_to_remove = self.advertised_services.clone();

		let state = state.read().unwrap_or_else(PoisonError::into_inner);
		for (port, ips) in ports_to_service {
			for (service_name, (_, metadata)) in &state.services {
				let Some(metadata) = metadata else {
					continue;
				};

				let mut meta = metadata.clone();
				meta.insert("__peer_id".into(), self.peer_id.to_string());
				meta.insert("__service".into(), service_name.to_string());
				meta.insert("__identity".into(), self.identity.to_string());

				// The max length of an MDNS record is painful so we just hash the data to come up with a pseudo-random but deterministic value.
				// The full values are stored within TXT records.
				let my_name = String::from_utf8_lossy(&base91::slice_encode(
					&sha256::digest(format!("{}_{}", service_name, self.identity)).as_bytes(),
				))[..63]
					.to_string();

				let service_domain = format!("_{service_name}._sub.{}", self.service_name);
				let service = match ServiceInfo::new(
					&service_domain,
					&my_name[..63], // 63 as long as the mDNS spec will allow us
					&format!("{}.{}.", service_name, self.identity), // TODO: Should this change???
					&*ips,
					port,
					Some(meta.clone()), // TODO: Prevent the user defining a value that overflows a DNS record
				) {
					Ok(service) => service, // TODO: .enable_addr_auto(), // TODO: using autoaddrs or not???
					Err(err) => {
						warn!("error creating mdns service info: {}", err);
						continue;
					}
				};

				let service_name = service.get_fullname().to_string();
				advertised_services_to_remove.retain(|s| *s != service_name);
				self.advertised_services.push(service_name);

				if !self
					.mdns_rx
					.iter_with_token()
					.any(|(s, _)| s.1 == service_domain)
				{
					let service = match self.mdns_daemon.browse(&service_domain) {
						Ok(v) => v,
						Err(err) => {
							error!("error browsing mdns service: {}", err);
							return;
						}
					};
					self.mdns_rx
						.insert(MdnsRecv(service.into_stream(), service_domain));
				}

				// TODO: Do a proper diff and remove old services
				trace!("advertising mdns service: {:?}", service);
				match self.mdns_daemon.register(service) {
					Ok(()) => {}
					Err(err) => warn!("error registering mdns service: {}", err),
				}
			}
		}

		for service_domain in advertised_services_to_remove {
			if let Some((_, token)) = self
				.mdns_rx
				.iter_with_token()
				.find(|(s, _)| s.1 == service_domain)
			{
				Pin::new(&mut self.mdns_rx).remove(token);
			}
			if let Err(err) = self.mdns_daemon.unregister(&service_domain) {
				warn!("error unregistering mdns service: {}", err);
			}
		}

		// If mDNS advertisement is not queued in future, queue one
		if self.next_mdns_advertisement.is_elapsed() {
			self.next_mdns_advertisement =
				Box::pin(sleep_until(Instant::now() + MDNS_READVERTISEMENT_INTERVAL));
		}
	}

	pub(crate) fn poll(
		&mut self,
		cx: &mut Context<'_>,
		listen_addrs: &ListenAddrs,
		state: &State,
	) -> Poll<()> {
		let mut is_pending = false;
		while !is_pending {
			match self.next_mdns_advertisement.poll_unpin(cx) {
				Poll::Ready(()) => self.do_advertisement(listen_addrs, state),
				Poll::Pending => is_pending = true,
			}

			match self.mdns_rx.poll_next_unpin(cx) {
				Poll::Ready(Some((result, _))) => match result {
					StreamYield::Item(event) => self.on_event(event, state),
					StreamYield::Finished(_) => {}
				},
				Poll::Ready(None) => {}
				Poll::Pending => is_pending = true,
			}
		}

		Poll::Pending
	}

	fn on_event(&mut self, event: ServiceEvent, state: &State) {
		match event {
			ServiceEvent::SearchStarted(_) => {}
			ServiceEvent::ServiceFound(_, _) => {}
			ServiceEvent::ServiceResolved(info) => {
				let Some(service_name) = info.get_properties().get("__service") else {
					warn!(
						"resolved mDNS peer advertising itself with missing '__service' metadata"
					);
					return;
				};
				let service_name = service_name.val_str();

				let Some(identity) = info.get_properties().get("__identity") else {
					warn!(
						"resolved mDNS peer advertising itself with missing '__identity' metadata"
					);
					return;
				};
				let identity = identity.val_str();

				println!("\t {:?} {:?}", info.get_fullname(), self.service_name); // TODO

				// if !service_type.ends_with(&self.service_name) {
				// 	warn!(
				// 		"resolved mDNS peer advertising itself with invalid service type '{service_type}'"
				// 	);
				// 	return;
				// }

				let Ok(identity) = RemoteIdentity::from_str(identity) else {
					warn!("resolved peer advertising itself with an invalid RemoteIdentity('{identity}')");
					return;
				};

				// Prevent discovery of the current peer.
				if identity == self.identity {
					return;
				}

				self.tracked_services.insert(
					info.get_fullname().to_string(),
					TrackedService {
						service_name: service_name.to_string(),
						identity: identity.clone(),
					},
				);

				let mut meta = info
					.get_properties()
					.iter()
					.map(|v| (v.key().to_owned(), v.val_str().to_owned()))
					.collect::<HashMap<_, _>>();

				let Some(peer_id) = meta.remove("__peer_id") else {
					warn!(
						"resolved mDNS peer advertising itself with missing '__peer_id' metadata"
					);
					return;
				};
				let Ok(peer_id) = PeerId::from_str(&peer_id) else {
					warn!(
						"resolved mDNS peer advertising itself with invalid '__peer_id' metadata"
					);
					return;
				};

				let mut state = state.write().unwrap_or_else(PoisonError::into_inner);

				if let Some((tx, _)) = state.services.get_mut(service_name) {
					if let Err(err) = tx.send((
						service_name.to_string(),
						ServiceEventInternal::Discovered {
							identity,
							metadata: meta.clone(),
						},
					)) {
						warn!(
							"error sending mDNS service event to '{service_name}' channel: {err}"
						);
					}
				} else {
					warn!(
						"mDNS service '{service_name}' is missing from 'state.services'. This is likely a bug!"
					);
				}

				if let Some(discovered) = state.discovered.get_mut(service_name) {
					discovered.insert(
						identity,
						DiscoveredPeerCandidate {
							peer_id,
							meta,
							addresses: info
								.get_addresses()
								.iter()
								.map(|addr| SocketAddr::new(*addr, info.get_port()))
								.collect(),
						},
					);
				} else {
					warn!("mDNS service '{service_name}' is missing from 'state.discovered'. This is likely a bug!");
				}
			}
			ServiceEvent::ServiceRemoved(_, fullname) => {
				let Some(TrackedService {
					service_name,
					identity,
				}) = self.tracked_services.remove(&fullname)
				else {
					warn!(
						"resolved mDNS peer deadvertising itself without having been discovered!"
					);
					return;
				};
				let mut state = state.write().unwrap_or_else(PoisonError::into_inner);

				if let Some((tx, _)) = state.services.get_mut(&service_name) {
					if let Err(err) = tx.send((
						service_name.to_string(),
						ServiceEventInternal::Expired { identity },
					)) {
						warn!("error sending mDNS service event '{service_name}': {err}");
					}
				} else {
					warn!(
						"mDNS service '{service_name}' is missing from 'state.services'. This is likely a bug!"
					);
				}

				if let Some(discovered) = state.discovered.get_mut(&service_name) {
					discovered.remove(&identity);
				} else {
					warn!("mDNS service '{service_name}' is missing from 'state.discovered'. This is likely a bug!");
				}
			}
			ServiceEvent::SearchStopped(_) => {}
		}
	}

	pub(crate) fn shutdown(&self) {
		for service in &self.advertised_services {
			self.mdns_daemon
				.unregister(service)
				.map_err(|err| {
					error!("error removing mdns service '{service}': {err}");
				})
				.ok();
		}

		// TODO: Without this mDNS is not sending it goodbye packets without a timeout. Try and remove this cause it makes shutdown slow.
		sleep(Duration::from_millis(100));

		match self.mdns_daemon.shutdown() {
			Ok(chan) => {
				let _ = chan.recv();
			}
			Err(err) => {
				error!("error shutting down mdns daemon: {err}");
			}
		}
	}
}

struct MdnsRecv(flume::r#async::RecvStream<'static, ServiceEvent>, String);

impl Stream for MdnsRecv {
	type Item = ServiceEvent;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		self.0.poll_next_unpin(cx)
	}
}
