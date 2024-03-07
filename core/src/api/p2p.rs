use crate::p2p::{operations, Header, P2PEvent, PeerMetadata};

use futures::future::join_all;
use sd_p2p2::{IdentityOrRemoteIdentity, RemoteIdentity};

use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
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
						PeerMetadata::from_hashmap(&p.metadata())
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
			R.query(|node, _: ()| async move { Ok(node.p2p.state().await) })
		})
		.procedure("debugGetLibraryPeers", {
			R.query(|node, _: ()| async move {
				Ok(join_all(
					node.libraries
						.get_all()
						.await
						.into_iter()
						.map(|l| async move {
							let library_id = l.id.to_string();

							let instances =
								l.db.instance()
									.find_many(vec![])
									.exec()
									.await
									.expect("we don't care")
									.into_iter()
									.map(|i| {
										IdentityOrRemoteIdentity::from_bytes(&i.identity)
											.expect("lol: invalid DB entry")
											.remote_identity()
									})
									.collect::<Vec<_>>();

							(library_id, instances)
						})
						.collect::<Vec<_>>(),
				)
				.await)
			})
		})
		.procedure("debugConnect", {
			R.mutation(|node, identity: RemoteIdentity| async move {
				let peer = { node.p2p.p2p.peers().get(&identity).cloned() };
				let mut stream = peer
					.ok_or(rspc::Error::new(
						ErrorCode::InternalServerError,
						"big man, offline".into(),
					))?
					.new_stream()
					.await
					.map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error in peer.new_stream: {:?}", err),
						)
					})?;

				stream
					.write_all(&Header::Ping.to_bytes())
					.await
					.map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error sending ping header: {:?}", err),
						)
					})?;

				Ok("connected")
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
