use std::{
	collections::HashMap,
	pin::Pin,
	task::{Context, Poll},
	time::Duration,
};

use mdns_sd::ServiceDaemon;

use crate::{Component, ManagerState};

// TODO: Advertise when another node annonces it has started up

/// The interval between re-advertising the service.
///
/// We continually re-advertise the service to ensure that it is always discoverable.
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute

/// Multicast DNS (mDNS) discovery system.
///
/// By using multicast UDP packets, holding DNS entries, devices on the same local network can discover each other.
///
pub struct Mdns {
	daemon: ServiceDaemon,
	services: HashMap<String, HashMap<String, String>>,
}

impl Mdns {
	pub fn new() -> Result<Self, mdns_sd::Error> {
		let daemon = ServiceDaemon::new()?;
		Ok(Self {
			daemon,
			services: Default::default(),
		})
	}
}

impl Component for Mdns {
	type OutEvent = ();

	fn poll(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		state: &mut ManagerState,
	) -> Poll<Option<Self::OutEvent>> {
		// TODO: MDNS readvertisement every minute

		// TODO: Receiving updates to `services`

		// TODO: Incoming mDNS discoveries
		// TODO: Re-emit if node startup and debounce is happy with it.

		Poll::Ready(None)
	}
}
