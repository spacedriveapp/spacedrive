use crate::p2p::{operations, P2PEvent, PeerMetadata};

use sd_p2p2::RemoteIdentity;

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
				let mut rx = node.p2p.events.subscribe();

				let mut queued = Vec::new();

				for (identity, peer, metadata) in
					node.p2p.p2p.peers().iter().filter_map(|(i, p)| {
						PeerMetadata::from_hashmap(&*p.metadata())
							.ok()
							.map(|m| (i, p, m))
					}) {
					let identity = *identity;
					match peer.is_connected() {
						true => queued.push(P2PEvent::ConnectedPeer { identity }),
						false => queued.push(P2PEvent::DiscoveredPeer { identity, metadata }),
					}
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
			R.query(|node, _: ()| async move { Ok(node.p2p.state()) })
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
