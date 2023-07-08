use async_stream::stream;
use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_p2p::PeerId;
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

use crate::p2p::P2PEvent;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("events", {
			R.subscription(|ctx, _: ()| async move {
				let mut rx = ctx.p2p.subscribe();
				async_stream::stream! {
					// TODO: Don't block subscription start
					for peer in ctx.p2p.manager.get_discovered_peers().await {
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

			R.mutation(|ctx, args: SpacedropArgs| async move {
				// TODO: Handle multiple files path and error if zero paths
				ctx.p2p
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
			R.mutation(|ctx, (id, path): (Uuid, Option<String>)| async move {
				match path {
					Some(path) => ctx.p2p.accept_spacedrop(id, path).await,
					None => ctx.p2p.reject_spacedrop(id).await,
				}
			})
		})
		.procedure("spacedropProgress", {
			R.subscription(|ctx, id: Uuid| async move {
				ctx.p2p.spacedrop_progress(id).await.ok_or_else(|| {
					rspc::Error::new(ErrorCode::BadRequest, "Spacedrop not found!".into())
				})
			})
		})
		.procedure("pair", {
			R.mutation(|ctx, id: PeerId| async move {
				ctx.p2p.pairing.originator(id, ctx.config.get().await).await
			})
		})
		.procedure("pairingProgress", {
			R.subscription(|ctx, pairing_id: u16| async move {
				if let Some(progress) = ctx.p2p.pairing.progress(pairing_id).await {
					Ok(stream! {
						loop {
							let v = progress.wait().await;
							yield v;
						}
					})
				} else {
					Err(rspc::Error::new(
						ErrorCode::BadRequest,
						"Pairing not found!".into(),
					))
				}
			})
		})
}
