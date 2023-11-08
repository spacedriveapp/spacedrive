use rspc::ErrorCode;
use sd_p2p::spacetunnel::RemoteIdentity;
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

use crate::p2p::{operations, P2PEvent, PairingDecision};

use super::{RouterBuilder, R};

pub(crate) fn mount() -> RouterBuilder {
	R.router()
		.procedure("events", {
			R.subscription(|node, _: ()| async move {
				let mut rx = node.p2p.subscribe();
				async_stream::stream! {
					// TODO: Don't block subscription start
					for peer in node.p2p.node.get_discovered() {
						 yield Ok(P2PEvent::DiscoveredPeer {
							identity: peer.identity,
							metadata: peer.metadata,
						});
					}

					// TODO: Don't block subscription start
					#[allow(clippy::unwrap_used)] // TODO: P2P isn't stable yet lol
					for identity in node.p2p.manager.get_connected_peers().await.unwrap() {
						yield Ok(P2PEvent::ConnectedPeer {
							identity,
						});
					}

					while let Ok(event) = rx.recv().await {
						yield Ok(event);
					}
				}
			})
		})
		.procedure("state", {
			R.query(|node, _: ()| async move { Ok(node.p2p.state()) })
		})
		.procedure("spacedrop", {
			#[derive(Type, Deserialize)]
			pub struct SpacedropArgs {
				identity: RemoteIdentity,
				file_path: Vec<String>,
			}

			R.mutation(|node, args: SpacedropArgs| async move {
				Ok(operations::spacedrop(
					node.p2p.clone(),
					args.identity,
					args.file_path
						.into_iter()
						.map(PathBuf::from)
						.collect::<Vec<_>>(),
				)
				.await
				.map_err(|_err| {
					rspc::Error::new(ErrorCode::InternalServerError, "todo: error".into())
				})?)
			})
		})
		.procedure("acceptSpacedrop", {
			R.mutation(|node, (id, path): (Uuid, Option<String>)| async move {
				Ok(match path {
					Some(path) => node.p2p.accept_spacedrop(id, path).await,
					None => node.p2p.reject_spacedrop(id).await,
				})
			})
		})
		.procedure("cancelSpacedrop", {
			R.mutation(|node, id: Uuid| async move { Ok(node.p2p.cancel_spacedrop(id).await) })
		})
		.procedure("pair", {
			R.mutation(|node, id: RemoteIdentity| async move {
				Ok(node.p2p.pairing.clone().originator(id, node).await)
			})
		})
		.procedure("pairingResponse", {
			R.mutation(|node, (pairing_id, decision): (u16, PairingDecision)| {
				Ok(node.p2p.pairing.decision(pairing_id, decision))
			})
		})
}
