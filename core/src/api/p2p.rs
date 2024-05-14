use crate::p2p::{operations, ConnectionMethod, DiscoveryMethod, Header, P2PEvent, PeerMetadata};

use sd_p2p::{PeerConnectionCandidate, RemoteIdentity};

use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
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
						connection: if peer.is_connected_with_hook(node.p2p.libraries_hook_id) {
							ConnectionMethod::Relay
						} else if peer.is_connected() {
							ConnectionMethod::Local
						} else {
							ConnectionMethod::Disconnected
						},
						discovery: match peer
							.connection_candidates()
							.contains(&PeerConnectionCandidate::Relay)
						{
							true => DiscoveryMethod::Relay,
							false => DiscoveryMethod::Local,
						},
						metadata,
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
			#[derive(Serialize, Type)]
			#[serde(tag = "type")]
			pub enum ListenerState {
				Listening,
				Error { error: String },
				Disabled,
			}

			#[derive(Serialize, Type)]
			pub struct Listeners {
				ipv4: ListenerState,
				ipv6: ListenerState,
			}

			R.query(|node, _: ()| async move {
				let addrs = node
					.p2p
					.p2p
					.listeners()
					.iter()
					.flat_map(|l| l.addrs.clone())
					.collect::<Vec<_>>();

				let errors = node
					.p2p
					.listener_errors
					.lock()
					.unwrap_or_else(PoisonError::into_inner);

				Ok(Listeners {
					ipv4: match errors.ipv4 {
						Some(ref err) => ListenerState::Error { error: err.clone() },
						None => match addrs.iter().any(|f| f.is_ipv4()) {
							true => ListenerState::Listening,
							false => ListenerState::Disabled,
						},
					},
					ipv6: match errors.ipv6 {
						Some(ref err) => ListenerState::Error { error: err.clone() },
						None => match addrs.iter().any(|f| f.is_ipv6()) {
							true => ListenerState::Listening,
							false => ListenerState::Disabled,
						},
					},
				})
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
