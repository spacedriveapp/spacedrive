use std::sync::Arc;

use sd_p2p2::{flume::bounded, HookEvent, P2P};

/// A P2P hook which listens for the availability of peers and connects with them.
pub struct ConnectHook {}

impl ConnectHook {
	pub fn spawn(p2p: Arc<P2P>) -> Self {
		let (tx, rx) = bounded(15);
		let _ = p2p.register_hook("sd-connect-hook", tx);

		tokio::spawn(async move {
			while let Ok(event) = rx.recv_async().await {
				match event {
					// TODO: Do the thing. For now we don't need this.
					HookEvent::Shutdown => break,
					_ => continue,
				}
			}
		});

		Self {}
	}
}
