use std::{
	collections::HashMap,
	net::SocketAddr,
	pin::Pin,
	sync::PoisonError,
	task::{Context, Poll},
	time::Duration,
};

use libp2p::{futures::FutureExt, PeerId};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::time::{sleep_until, Instant, Sleep};
use tracing::{trace, warn};

use crate::{ListenAddrs, State};

/// TODO
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute re-advertise

pub(crate) struct Mdns {
	// used to ignore events from our own mdns advertisement
	peer_id: PeerId,
	service_name: String,
	advertised_services: Vec<String>,
	mdns_daemon: ServiceDaemon,
	mdns_rx: flume::Receiver<ServiceEvent>,
	next_mdns_advertisement: Pin<Box<Sleep>>,
}

impl Mdns {
	pub(crate) fn new(
		application_name: &'static str,
		peer_id: PeerId,
	) -> Result<Self, mdns_sd::Error> {
		let mdns_daemon = ServiceDaemon::new()?;
		let service_name = format!("_{}._udp.local.", application_name);
		let mdns_rx = mdns_daemon.browse(&service_name)?;

		Ok(Self {
			peer_id,
			service_name,
			advertised_services: Vec::new(),
			mdns_daemon,
			mdns_rx,
			next_mdns_advertisement: Box::pin(sleep_until(Instant::now())), // Trigger an advertisement immediately
		})
	}

	/// Do an mdns advertisement to the network.
	pub(super) fn do_advertisement(&mut self, listen_addrs: &ListenAddrs, state: &State) {
		trace!("doing mDNS advertisement!");
		// TODO: Second stage rate-limit

		let mut ports_to_service = HashMap::new();
		for addr in listen_addrs.iter() {
			let addr = match addr {
				SocketAddr::V4(addr) => addr,
				// TODO: Our mdns library doesn't support Ipv6. This code has the infra to support it so once this issue is fixed upstream we can just flip it on.
				// Refer to issue: https://github.com/keepsimple1/mdns-sd/issues/61
				SocketAddr::V6(_) => continue,
			};

			ports_to_service
				.entry(addr.port())
				.or_insert_with(Vec::new)
				.push(addr.ip());
		}

		// This method takes `&mut self` so we know we have exclusive access to `advertised_services`
		let mut advertised_services_to_remove = self.advertised_services.clone();

		let state = state.read().unwrap_or_else(PoisonError::into_inner);
		for (port, ips) in ports_to_service.into_iter() {
			for (service_name, (_, metadata)) in &state.services {
				let service = match ServiceInfo::new(
					&self.service_name,
					&self.peer_id.to_string(),
					&format!("{}.{}.", service_name, self.peer_id),
					&*ips, // TODO: &[] as &[Ipv4Addr],
					port,
					Some(metadata.clone()), // TODO: Prevent the user defining a value that overflows a DNS record
				) {
					Ok(service) => service, // TODO: .enable_addr_auto(), // TODO: using autoaddrs or not???
					Err(err) => {
						warn!("error creating mdns service info: {}", err);
						continue;
					}
				};

				let service_name = service.get_fullname().to_string();
				println!("{:?}", service_name); // TODO
				advertised_services_to_remove.retain(|s| *s != service_name);
				self.advertised_services.push(service_name);

				// TODO: Do a proper diff and remove old services
				trace!("advertising mdns service: {:?}", service);
				match self.mdns_daemon.register(service) {
					Ok(_) => {}
					Err(err) => warn!("error registering mdns service: {}", err),
				}
			}
		}

		for service in advertised_services_to_remove {
			println!("REMOVING {service:?}");

			// TODO
			// self.mdns_daemon.unregister(fullname)
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
				Poll::Ready(()) => self.do_advertisement(&listen_addrs, &state),
				Poll::Pending => is_pending = true,
			}

			match self.mdns_rx.recv_async().poll_unpin(cx) {
				Poll::Ready(Ok(event)) => {
					// TODO
					println!("{:?}", event);
				}
				Poll::Ready(Err(err)) => warn!("mDNS reciever error: {err:?}"),
				Poll::Pending => is_pending = true,
			}
		}

		Poll::Pending
	}

	pub(crate) fn shutdown(&self) {
		// TODO: Deregister all services
		// TODO: Shutdown Daemon

		// self.mdns_daemon
		// 	.unregister(&format!("{}.{}", self.peer_id, self.service_name))
		// 	.unwrap();

		// 		match self
		// 			.mdns_daemon
		// 			.unregister(&format!("{}.{}", self.peer_id, self.service_name))
		// 			.map(|chan| chan.recv())
		// 		{
		// 			Ok(Ok(_)) => {}
		// 			Ok(Err(err)) => {
		// 				warn!(
		// 					"shutdown error recieving shutdown status from mdns service: {}",
		// 					err
		// 				);
		// 			}
		// 			Err(err) => {
		// 				warn!("shutdown error unregistering mdns service: {}", err);
		// 			}
		// 		}

		// 		self.mdns_daemon.shutdown().unwrap_or_else(|err| {
		// 			error!("shutdown error shutting down mdns daemon: {}", err);
		// 		});
	}
}
