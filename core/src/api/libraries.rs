use crate::{
	library::LibraryConfig,
	prisma::statistics,
	util::MaybeUndefined,
	volume::{get_volumes, save_volume},
};

use chrono::Utc;
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use specta::Type;
use tracing::debug;
use uuid::Uuid;

use super::{
	utils::{get_size, library},
	Ctx, R,
};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.query(
				|ctx, _: ()| async move { ctx.library_manager.get_all_libraries_config().await },
			)
		})
		.procedure("statistics", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				let _statistics = library
					.db
					.statistics()
					.find_unique(statistics::id::equals(library.node_local_id))
					.exec()
					.await?;

				// TODO: get from database, not sys
				let volumes = get_volumes();
				save_volume(&library).await?;

				let mut available_capacity: u64 = 0;
				let mut total_capacity: u64 = 0;

				if let Ok(volumes) = volumes {
					for volume in volumes {
						total_capacity += volume.total_capacity;
						available_capacity += volume.available_capacity;
					}
				}

				let library_db_size = get_size(
					library
						.config()
						.data_directory()
						.join("libraries")
						.join(&format!("{}.db", library.id)),
				)
				.await
				.unwrap_or(0);

				let thumbnail_folder_size =
					get_size(library.config().data_directory().join("thumbnails"))
						.await
						.unwrap_or(0);

				use statistics::*;
				let params = vec![
					id::set(1), // Each library is a database so only one of these ever exists
					date_captured::set(Utc::now().into()),
					total_object_count::set(0),
					library_db_size::set(library_db_size.to_string()),
					total_bytes_used::set(0.to_string()),
					total_bytes_capacity::set(total_capacity.to_string()),
					total_unique_bytes::set(0.to_string()),
					total_bytes_free::set(available_capacity.to_string()),
					preview_media_bytes::set(thumbnail_folder_size.to_string()),
				];

				Ok(library
					.db
					.statistics()
					.upsert(
						statistics::id::equals(1), // Each library is a database so only one of these ever exists
						statistics::create(params.clone()),
						params,
					)
					.exec()
					.await?)
			})
		})
		.procedure("create", {
			#[derive(Deserialize, Type)]
			pub struct CreateLibraryArgs {
				name: String,
			}

			R.mutation(|ctx, args: CreateLibraryArgs| async move {
				debug!("Creating library");

				let new_library = ctx
					.library_manager
					.create(
						LibraryConfig::new(args.name.to_string(), ctx.config.get().await.id),
						ctx.config.get().await,
					)
					.await?;

				Ok(new_library)
			})
		})
		.procedure("edit", {
			#[derive(Type, Deserialize)]
			pub struct EditLibraryArgs {
				pub id: Uuid,
				pub name: Option<String>,
				pub description: MaybeUndefined<String>,
			}

			R.mutation(|ctx, args: EditLibraryArgs| async move {
				Ok(ctx
					.library_manager
					.edit(args.id, args.name, args.description)
					.await?)
			})
		})
		.procedure(
			"delete",
			R.mutation(|ctx, id: Uuid| async move { Ok(ctx.library_manager.delete(id).await?) }),
		)
	// .yolo_merge("peer.guest.", peer_guest_router())
	// .yolo_merge("peer.host.", peer_host_router())
}

// pub(crate) fn peer_guest_router() -> RouterBuilder {
// 	<RouterBuilder>::new()
// 		.subscription("request_peering", |t| {
// 			t(|node, peer_id: String| {
// 				async_stream::stream! {
//                     let mut rx = node.begin_guest_peer_request(peer_id).await.unwrap();

// 					while let Some(state) = rx.recv().await {
// 						yield state;
// 					}
// 				}
// 			})
// 		})
// 		.mutation("submit_password", |t| {
// 			t(|node, password: String| async move {
// 				let request = node.peer_request.lock().await;
// 				let Some(peer_request::PeerRequest::Guest(request)) = &*request else {
//                     return
//                 };

//                 request.submit_password(password).await;
// 			})
// 		})
// }

// pub(crate) fn peer_host_router() -> RouterBuilder {
// 	<RouterBuilder>::new()
// 		.subscription("request", |t| {
// 			t(|node, _: ()| async_stream::stream! { yield (); })
// 		})
// 		.mutation("accept", |t| t(|node, _: ()| Ok(())))
// }
