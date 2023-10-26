use crate::{
	library::{Library, LibraryConfig, LibraryName},
	location::{scan_location, LocationCreateArgs},
	util::MaybeUndefined,
	volume::get_volumes,
	Node,
};

use sd_p2p::spacetunnel::RemoteIdentity;
use sd_prisma::prisma::{indexer_rule, statistics};

use std::{convert::identity, sync::Arc};

use chrono::Utc;
use directories::UserDirs;
use futures_concurrency::future::Join;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::spawn;
use tracing::{debug, error};
use uuid::Uuid;

use super::{
	utils::{get_size, library},
	Ctx, R,
};

// TODO(@Oscar): Replace with `specta::json`
#[derive(Serialize, Type)]
pub struct LibraryConfigWrapped {
	pub uuid: Uuid,
	pub instance_id: Uuid,
	pub instance_public_key: RemoteIdentity,
	pub config: LibraryConfig,
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.query(|node, _: ()| async move {
				node.libraries
					.get_all()
					.await
					.into_iter()
					.map(|lib| LibraryConfigWrapped {
						uuid: lib.id,
						instance_id: lib.instance_uuid,
						instance_public_key: lib.identity.to_remote_identity(),
						config: lib.config(),
					})
					.collect::<Vec<_>>()
			})
		})
		.procedure("statistics", {
			R.with2(library())
				.query(|(node, library), _: ()| async move {
					// TODO: get from database if library is offline
					// let _statistics = library
					// 	.db
					// 	.statistics()
					// 	.find_unique(statistics::id::equals(library.node_local_id))
					// 	.exec()
					// 	.await?;

					let volumes = get_volumes().await;
					// save_volume(&library).await?;

					let mut total_capacity: u64 = 0;
					let mut available_capacity: u64 = 0;
					for volume in volumes {
						total_capacity += volume.total_capacity;
						available_capacity += volume.available_capacity;
					}

					let library_db_size = get_size(
						node.config
							.data_directory()
							.join("libraries")
							.join(&format!("{}.db", library.id)),
					)
					.await
					.unwrap_or(0);

					let thumbnail_folder_size =
						get_size(node.config.data_directory().join("thumbnails"))
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
			#[derive(Deserialize, Type, Default)]
			pub struct DefaultLocations {
				desktop: bool,
				documents: bool,
				downloads: bool,
				pictures: bool,
				music: bool,
				videos: bool,
			}

			#[derive(Deserialize, Type)]
			pub struct CreateLibraryArgs {
				name: LibraryName,
				default_locations: Option<DefaultLocations>,
			}

			async fn create_default_locations_on_library_creation(
				DefaultLocations {
					desktop,
					documents,
					downloads,
					pictures,
					music,
					videos,
				}: DefaultLocations,
				node: Arc<Node>,
				library: Arc<Library>,
			) -> Result<(), rspc::Error> {
				// If all of them are false, we skip
				if [!desktop, !documents, !downloads, !pictures, !music, !videos]
					.into_iter()
					.all(identity)
				{
					return Ok(());
				}

				let Some(default_locations_paths) = UserDirs::new() else {
					return Err(rspc::Error::new(
						ErrorCode::NotFound,
						"Didn't find any system locations".to_string(),
					));
				};

				let default_rules_ids = library
					.db
					.indexer_rule()
					.find_many(vec![indexer_rule::default::equals(Some(true))])
					.select(indexer_rule::select!({ id }))
					.exec()
					.await
					.map_err(|e| {
						rspc::Error::with_cause(
							ErrorCode::InternalServerError,
							"Failed to get default indexer rules for default locations".to_string(),
							e,
						)
					})?
					.into_iter()
					.map(|rule| rule.id)
					.collect::<Vec<_>>();

				let mut maybe_error = None;

				[
					(desktop, default_locations_paths.desktop_dir()),
					(documents, default_locations_paths.document_dir()),
					(downloads, default_locations_paths.download_dir()),
					(pictures, default_locations_paths.picture_dir()),
					(music, default_locations_paths.audio_dir()),
					(videos, default_locations_paths.video_dir()),
				]
				.into_iter()
				.filter_map(|entry| {
					if let (true, Some(path)) = entry {
						let node = Arc::clone(&node);
						let library = Arc::clone(&library);
						let indexer_rules_ids = default_rules_ids.clone();
						let path = path.to_path_buf();
						Some(spawn(async move {
							let Some(location) = LocationCreateArgs {
								path,
								dry_run: false,
								indexer_rules_ids,
							}
							.create(&node, &library)
							.await
							.map_err(rspc::Error::from)?
							else {
								return Ok(());
							};

							scan_location(&node, &library, location)
								.await
								.map_err(rspc::Error::from)
						}))
					} else {
						None
					}
				})
				.collect::<Vec<_>>()
				.join()
				.await
				.into_iter()
				.map(|spawn_res| {
					spawn_res
						.map_err(|_| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"A task to create a default location failed".to_string(),
							)
						})
						.and_then(identity)
				})
				.fold(&mut maybe_error, |maybe_error, res| {
					if let Err(e) = res {
						error!("Failed to create default location: {e:#?}");
						*maybe_error = Some(e);
					}
					maybe_error
				});

				if let Some(e) = maybe_error {
					return Err(e);
				}

				debug!("Created default locations");

				Ok(())
			}

			R.mutation(
				|node,
				 CreateLibraryArgs {
				     name,
				     default_locations,
				 }: CreateLibraryArgs| async move {
					debug!("Creating library");

					let library = node.libraries.create(name, None, &node).await?;

					debug!("Created library {}", library.id);

					if let Some(locations) = default_locations {
						create_default_locations_on_library_creation(
							locations,
							node,
							Arc::clone(&library),
						)
						.await?;
					}

					Ok(LibraryConfigWrapped {
						uuid: library.id,
						instance_id: library.instance_uuid,
						instance_public_key: library.identity.to_remote_identity(),
						config: library.config(),
					})
				},
			)
		})
		.procedure("edit", {
			#[derive(Type, Deserialize)]
			pub struct EditLibraryArgs {
				pub id: Uuid,
				pub name: Option<LibraryName>,
				pub description: MaybeUndefined<String>,
			}

			R.mutation(|node, args: EditLibraryArgs| async move {
				Ok(node
					.libraries
					.edit(args.id, args.name, args.description)
					.await?)
			})
		})
		.procedure(
			"delete",
			R.mutation(|node, id: Uuid| async move {
				node.libraries.delete(&id).await.map_err(Into::into)
			}),
		)
}
