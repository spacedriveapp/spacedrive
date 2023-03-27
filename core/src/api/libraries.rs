use crate::{
	crypto::OnboardingConfig,
	invalidate_query,
	library::{Library, LibraryConfig},
	prisma::statistics,
	volume::{get_volumes, save_volume},
};

use sd_crypto::{
	types::{Algorithm, HashingAlgorithm},
	Protected,
};

use chrono::Utc;
use rspc::{Error, ErrorCode, Type};
use serde::Deserialize;
use tracing::debug;
use uuid::Uuid;

use super::{
	utils::{get_size, LibraryRequest},
	RouterBuilder,
};

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.query("list", |t| {
			t(|ctx, _: ()| async move { ctx.library_manager.get_all_libraries_config().await })
		})
		.library_query("getStatistics", |t| {
			t(|_, _: (), library: Library| async move {
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
						params.clone(),
						params,
					)
					.exec()
					.await?)
			})
		})
		.mutation("create", |t| {
			#[derive(Deserialize, Type)]
			#[serde(tag = "type", content = "value")]
			#[specta(inline)]
			enum AuthOption {
				Password(Protected<String>),
				TokenizedPassword(String),
			}

			#[derive(Deserialize, Type)]
			pub struct CreateLibraryArgs {
				name: String,
				auth: AuthOption,
				algorithm: Algorithm,
				hashing_algorithm: HashingAlgorithm,
			}

			t(|ctx, args: CreateLibraryArgs| async move {
				debug!("Creating library");

				let password = match args.auth {
					AuthOption::Password(password) => password,
					AuthOption::TokenizedPassword(tokenized_pw) => {
						let token = Uuid::parse_str(&tokenized_pw).map_err(|err| {
							Error::with_cause(
								ErrorCode::BadRequest,
								"Failed to parse UUID".to_string(),
								err,
							)
						})?;
						Protected::new(ctx.secure_temp_keystore.claim(token).map_err(|err| {
							Error::with_cause(
								ErrorCode::InternalServerError,
								"Failed to claim token from keystore".to_string(),
								err,
							)
						})?)
					}
				};

				let new_library = ctx
					.library_manager
					.create(
						LibraryConfig {
							name: args.name.to_string(),
							..Default::default()
						},
						OnboardingConfig {
							password,
							algorithm: args.algorithm,
							hashing_algorithm: args.hashing_algorithm,
						},
					)
					.await?;

				invalidate_query!(
					// SAFETY: This unwrap is alright as we just created the library
					ctx.library_manager.get_ctx(new_library.uuid).await.unwrap(),
					"library.getStatistics"
				);

				Ok(new_library)
			})
		})
		.mutation("edit", |t| {
			#[derive(Type, Deserialize)]
			pub struct EditLibraryArgs {
				pub id: Uuid,
				pub name: Option<String>,
				pub description: Option<String>,
			}

			t(|node, args: EditLibraryArgs| async move {
				Ok(node
					.library_manager
					.edit(args.id, args.name, args.description)
					.await?)
			})
		})
		.mutation("delete", |t| {
			t(|ctx, id: Uuid| async move { Ok(ctx.library_manager.delete_library(id).await?) })
		})
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
