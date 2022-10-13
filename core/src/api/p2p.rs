// use std::collections::HashMap;

// use p2p::PeerId;
// use rspc::Type;
// use serde::Deserialize;

// use super::{LibraryArgs, RouterBuilder};

// #[derive(Type, Deserialize)]
// pub struct AcceptPairingRequestArgs {
// 	pub peer_id: PeerId,
// 	pub preshared_key: String,
// }

// pub(crate) fn mount() -> RouterBuilder {
// 	RouterBuilder::new()
// 		.query("getNodes", |ctx, arg: LibraryArgs<()>| async move {
// 			let (_, library) = arg.get_library(&ctx).await?;

// 			Ok(
// 				library.db.node().find_many(vec![]).exec().await?, // TODO: Make this work
// 				                                                   // .into_iter()
// 				                                                   // .filter_map(|v| {
// 				                                                   // 	if v.id == ctx.node_local_id {
// 				                                                   // 		None
// 				                                                   // 	} else {
// 				                                                   // 		Some(v.into())
// 				                                                   // 	}
// 				                                                   // })
// 				                                                   // .collect::<Vec<LibraryNode>>()
// 			)
// 		})
// 		.query("connectedPeers", |ctx, _: ()| async move {
// 			ctx.p2p
// 				.nm
// 				.connected_peers()
// 				.into_iter()
// 				.map(|(_, v)| (v.id, v.metadata))
// 				.collect::<HashMap<_, _>>()
// 		})
// 		.query("discoveredPeers", |ctx, _: ()| async move {
// 			ctx.p2p
// 				.nm
// 				.discovered_peers()
// 				.into_iter()
// 				// TODO: Make this better
// 				.map(|(_, v)| v)
// 				.collect::<Vec<_>>()
// 		})
// 		.mutation("pairNode", |ctx, arg: LibraryArgs<PeerId>| async move {
// 			let (peer_id, library) = arg.get_library(&ctx).await?;

// 			let preshared_key = ctx.p2p.pair(&library, peer_id).await.unwrap();

// 			// TODO: These aren't library queries so they can't be invalidated with the current system. We can fix this with the normalised cache!
// 			// invalidate_query!(ctx, "p2p.discoveredPeers": (), ());
// 			// invalidate_query!(ctx, "p2p.connectedPeers": (), ());

// 			Ok(preshared_key)
// 		})
// 		.mutation(
// 			"unpairNode",
// 			|_, _: LibraryArgs<PeerId>| async move { todo!() },
// 		)
// 		.mutation(
// 			"acceptPairingRequest",
// 			|ctx, arg: AcceptPairingRequestArgs| async move {
// 				ctx.p2p
// 					.pairing_requests
// 					.lock()
// 					.unwrap()
// 					.remove(&arg.peer_id)
// 					.unwrap()
// 					.send(Ok(arg.preshared_key))
// 					.unwrap(); // TODO: Remove unwrap
// 			},
// 		)
// }
