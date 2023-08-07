use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_p2p::PeerId;
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

use crate::p2p::{P2PEvent, PairingDecision};

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("events", {
			R.subscription(|node, _: ()| async move {
				let mut rx = node.p2p.subscribe();
				async_stream::stream! {
					// TODO: Don't block subscription start
					for peer in node.p2p.manager.get_discovered_peers().await {
						yield P2PEvent::DiscoveredPeer {
							peer_id: peer.peer_id,
							metadata: peer.metadata,
						};
					}

					// // TODO: Don't block subscription start
					// for peer in ctx.p2p_manager.get_connected_peers().await.unwrap() {
					// 	// TODO: Send to frontend
					// }


					while let Ok(event) = rx.recv().await {
						yield event;
					}
				}
			})
		})
		.procedure("spacedrop", {
			#[derive(Type, Deserialize)]
			pub struct SpacedropArgs {
				peer_id: PeerId,
				file_path: Vec<String>,
			}

			R.mutation(|node, args: SpacedropArgs| async move {
				// TODO: Handle multiple files path and error if zero paths
				node.p2p
					.big_bad_spacedrop(
						args.peer_id,
						PathBuf::from(
							args.file_path
								.first()
								.expect("https://linear.app/spacedriveapp/issue/ENG-625/spacedrop-multiple-files"),
						),
					)
					.await
					.map_err(|_| {
						rspc::Error::new(ErrorCode::InternalServerError, "todo".to_string())
					})
			})
		})
		.procedure("acceptSpacedrop", {
			R.mutation(|node, (id, path): (Uuid, Option<String>)| async move {
				match path {
					Some(path) => node.p2p.accept_spacedrop(id, path).await,
					None => node.p2p.reject_spacedrop(id).await,
				}
			})
		})
		// TODO: Send this over `p2p.events`
		.procedure("spacedropProgress", {
			R.subscription(|node, id: Uuid| async move {
				node.p2p.spacedrop_progress(id).await.ok_or_else(|| {
					rspc::Error::new(ErrorCode::BadRequest, "Spacedrop not found!".into())
				})
			})
		})
		.procedure("pair", {
			R.mutation(|node, id: PeerId| async move {
				node.p2p.pairing.clone().originator(id, node).await
			})
		})
		.procedure("pairingResponse", {
			R.mutation(|node, (pairing_id, decision): (u16, PairingDecision)| {
				node.p2p.pairing.decision(pairing_id, decision);
			})
		})
}
