use crate::p2p::{operations, ConnectionMethod, DiscoveryMethod, Header, P2PEvent, PeerMetadata};

use sd_p2p::{PeerConnectionCandidate, RemoteIdentity};

use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use std::{path::PathBuf, sync::PoisonError};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("events", {
			R.subscription(|node, _: ()| async move {
				let mut rx = node.p2p.events.subscribe();

				let mut queued = Vec::new();

				for (_, peer, metadata) in node.p2p.p2p.peers().iter().filter_map(|(i, p)| {
					PeerMetadata::from_hashmap(&p.metadata())
						.ok()
						.map(|m| (i, p, m))
				}) {
					queued.push(P2PEvent::PeerChange {
						identity: peer.identity(),
						connection: if peer.is_connected() {
							if node.p2p.quic.is_relayed(peer.identity()) {
								ConnectionMethod::Relay
							} else {
								ConnectionMethod::Local
							}
						} else {
							ConnectionMethod::Disconnected
						},
						discovery: if peer
							.connection_candidates()
							.iter()
							.any(|c| matches!(c, PeerConnectionCandidate::Manual(_)))
						{
							DiscoveryMethod::Manual
						} else if peer
							.connection_candidates()
							.iter()
							.all(|c| *c == PeerConnectionCandidate::Relay)
						{
							DiscoveryMethod::Relay
						} else {
							DiscoveryMethod::Local
						},
						metadata,
						addrs: peer.addrs(),
					});
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
		.procedure("listeners", {
			R.query(|node, _: ()| async move {
				Ok(node
					.p2p
					.listeners
					.lock()
					.unwrap_or_else(PoisonError::into_inner)
					.clone())
			})
		})
		.procedure("debugConnect", {
			R.mutation(|node, identity: RemoteIdentity| async move {
				let peer = { node.p2p.p2p.peers().get(&identity).cloned() };
				let mut stream = peer
					.ok_or(rspc::Error::new(
						ErrorCode::InternalServerError,
						"big man, not found".into(),
					))?
					.new_stream()
					.await
					.map_err(|e| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error in peer.new_stream: {:?}", e),
						)
					})?;

				stream
					.write_all(&Header::Ping.to_bytes())
					.await
					.map_err(|e| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error sending ping header: {:?}", e),
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
				.map_err(|spacedrop_err| {
					rspc::Error::new(ErrorCode::InternalServerError, spacedrop_err.to_string())
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
