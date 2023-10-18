use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_p2p::{spacetunnel::RemoteIdentity, PeerId};
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::p2p::{operations, P2PEvent, PairingDecision};

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("events", {
			R.subscription(|node, _: ()| async move {
				let mut rx = node.p2p.subscribe();
				async_stream::stream! {
					// TODO: Account for library-services cause this doesn't & that will cuase issues

					// TODO: Don't block subscription start
					for peer in node.p2p.node.get_discovered() {
						 yield P2PEvent::DiscoveredPeer {
							peer_id: peer.peer_id,
							metadata: peer.metadata,
						};
					}

					// TODO: Don't block subscription start
					#[allow(clippy::unwrap_used)] // TODO: P2P isn't stable yet lol
					for peer_id in node.p2p.manager.get_connected_peers().await.unwrap() {
						yield P2PEvent::ConnectedPeer {
							peer_id,
						};
					}

					while let Ok(event) = rx.recv().await {
						yield event;
					}
				}
			})
		})
		// TODO: Bring this back
		// .procedure("state", {
		// 	pub struct P2PState {
		// 		nodes: Vec<RemoteIdentity, PeerId>,
		// 		libs: Vec<
		// 	}
		// 	R.query(|node, _: ()| async move {})
		// })
		.procedure("spacedrop", {
			#[derive(Type, Deserialize)]
			pub struct SpacedropArgs {
				peer_id: PeerId,
				file_path: Vec<String>,
			}

			R.mutation(|node, args: SpacedropArgs| async move {
				operations::spacedrop(
					node.p2p.clone(),
					args.peer_id,
					args.file_path
						.into_iter()
						.map(PathBuf::from)
						.collect::<Vec<_>>(),
				)
				.await
				.map_err(|_err| {
					rspc::Error::new(ErrorCode::InternalServerError, "todo: error".into())
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
		.procedure("cancelSpacedrop", {
			R.mutation(|node, id: Uuid| async move { node.p2p.cancel_spacedrop(id).await })
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
