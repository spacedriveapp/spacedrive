use std::{
	collections::HashMap,
	net::SocketAddr,
	pin::Pin,
	sync::PoisonError,
	task::{Context, Poll},
	time::Duration,
};

use libp2p::futures::FutureExt;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::{
	sync::mpsc,
	time::{sleep_until, Instant, Sleep},
};
use tracing::{trace, warn};

use crate::{DiscoveryManager, Event, MultiFlume, PeerId};

/// TODO
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute re-advertise

pub(crate) struct Mdns {
	// used to ignore events from our own mdns advertisement
	peer_id: PeerId,
	mdns_daemon: ServiceDaemon,
	services_rx: MultiFlume<ServiceEvent>,
	next_mdns_advertisement: Pin<Box<Sleep>>,
}

impl Mdns {
	pub(crate) fn new(
		application_name: &'static str,
		peer_id: PeerId,
	) -> Result<Self, mdns_sd::Error> {
		let mdns_daemon = ServiceDaemon::new()?;
		// let service_name = format!("_{}._udp.local.", application_name);
		// let mdns_service_receiver = mdns_daemon.browse(&service_name)?;

		todo!();
		// Ok(Self {
		// 	peer_id,
		// 	mdns_daemon,
		// 	mdns_service_receiver,
		// 	// service_name,
		// 	next_mdns_advertisement: Box::pin(sleep_until(Instant::now())), // Trigger an advertisement immediately
		// 	next_allowed_discovery_advertisement: Instant::now(),
		// })
	}

	/// Do an mdns advertisement to the network.
	fn do_advertisement(&mut self, discovery: &DiscoveryManager) {
		let mut ports_to_service = HashMap::new();
		for addr in &discovery.listen_addrs {
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

		let state = discovery
			.state
			.read()
			.unwrap_or_else(PoisonError::into_inner);

		// TODO: Automatically unregister any services that are now missing

		// for (port, ips) in ports_to_service.into_iter() {
		// 	for (service_name, metadata) in &state.services {
		// 		let service = match ServiceInfo::new(
		// 			&self.service_name,
		// 			&self.peer_id.to_string(),
		// 			&format!("{}.", self.peer_id),
		// 			&*ips,
		// 			port,
		// 			Some(metadata.clone()), // TODO: Prevent the user defining a value that overflows a DNS record
		// 		) {
		// 			Ok(service) => service,
		// 			Err(err) => {
		// 				warn!("error creating mdns service info: {}", err);
		// 				continue;
		// 			}
		// 		};

		// 		trace!("advertising mdns service: {:?}", service);
		// 		match self.mdns_daemon.register(service) {
		// 			Ok(_) => {}
		// 			Err(err) => warn!("error registering mdns service: {}", err),
		// 		}
		// 	}
		// }

		// If mDNS advertisement is not queued in future, queue one
		if self.next_mdns_advertisement.is_elapsed() {
			self.next_mdns_advertisement =
				Box::pin(sleep_until(Instant::now() + MDNS_READVERTISEMENT_INTERVAL));
		}
	}

	pub(crate) fn poll(&mut self, cx: &mut Context<'_>, discovery: &DiscoveryManager) -> Poll<()> {
		let mut is_pending = false;
		while !is_pending {
			match self.next_mdns_advertisement.poll_unpin(cx) {
				Poll::Ready(()) => self.do_advertisement(&discovery),
				Poll::Pending => is_pending = true,
			}

			// match self.mdns_service_receiver.recv_async().poll_unpin(cx) {
			// 	Poll::Ready(Ok(event)) => {
			// 		// TODO
			// 	}
			// 	Poll::Ready(Err(err)) => warn!("mDNS reciever error: {err:?}"),
			// 	Poll::Pending => is_pending = true,
			// }
			todo!();
		}

		Poll::Pending
	}

	pub(crate) fn shutdown(&self) {
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
