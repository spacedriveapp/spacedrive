use sd_p2p::Event;

use super::RouterBuilder;

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new().subscription("discovery", |t| {
		t(|ctx, _: ()| {
			let mut rx = ctx.p2p_manager.events();
			async_stream::stream! {
				ctx.p2p_manager.temp_emit_discovered_peers().await; // TODO: This causes an emit to all clients. Only emit to the client that requested it

				while let Ok(event) = rx.recv().await {
					if let Event::EmitDiscoveredClients = event {
						continue;
					}

					yield event;
				}
			}
		})
	})
}
