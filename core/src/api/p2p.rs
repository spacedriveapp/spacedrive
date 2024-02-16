use crate::p2p::{operations, P2PEvent};

use sd_p2p::spacetunnel::RemoteIdentity;

use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("events", {
			R.subscription(|node, _: ()| async move {
				let mut rx = node.p2p.subscribe();

				let mut queued = Vec::new();

				// TODO: Don't block subscription start
				for peer in node.p2p.node.get_discovered() {
					queued.push(P2PEvent::DiscoveredPeer {
						identity: peer.identity,
						metadata: peer.metadata,
					});
				}

				// TODO: Don't block subscription start
				for identity in node.p2p.manager.get_connected_peers().await.map_err(|_| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						"todo: error getting connected peers".into(),
					)
				})? {
					queued.push(P2PEvent::ConnectedPeer { identity });
				}

				Ok(async_stream::stream! {
					for event in queued.drain(..queued.len()) {
						yield event;
					}

					while let Ok(event) = rx.recv().await {
						yield event;
					}
				})
			})
		})
		.procedure("state", {
			R.query(|node, _: ()| async move {
				// TODO: This has a potentially invalid map key and Specta don't like that.
				// TODO: This will bypass that check and for an debug route that's fine.
				Ok(serde_json::to_value(node.p2p.state()).unwrap())
			})
		})
		.procedure("spacedrop", {
			#[derive(Type, Deserialize)]
			pub struct SpacedropArgs {
				identity: RemoteIdentity,
				file_path: Vec<String>,
			}

			R.mutation(|node, args: SpacedropArgs| async move {
				operations::spacedrop(
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
				})
			})
		})
		.procedure("acceptSpacedrop", {
			R.mutation(|node, (id, path): (Uuid, Option<String>)| async move {
				match path {
					Some(path) => node.p2p.accept_spacedrop(id, path).await,
					None => node.p2p.reject_spacedrop(id).await,
				};

				Ok(())
			})
		})
		.procedure("cancelSpacedrop", {
			R.mutation(|node, id: Uuid| async move {
				node.p2p.cancel_spacedrop(id).await;

				Ok(())
			})
		})
}
