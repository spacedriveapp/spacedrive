use crate::{
	invalidate_query,
	library::{update_library_statistics, Library, LibraryConfig, LibraryName},
	location::{scan_location, LocationCreateArgs},
	util::MaybeUndefined,
	Node,
};

use sd_cache::{Model, Normalise, NormalisedResult, NormalisedResults};
use sd_file_ext::kind::ObjectKind;
use sd_p2p::spacetunnel::RemoteIdentity;
use sd_prisma::prisma::{indexer_rule, object, statistics};
use std::{
	convert::identity,
	sync::{Arc, Once},
	time::Duration,
};
use strum::IntoEnumIterator;

use directories::UserDirs;
use futures_concurrency::future::Join;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{spawn, time::sleep};
use tracing::{debug, error};
use uuid::Uuid;

use super::{utils::library, Ctx, R};

static STATISTICS_UPDATER: Once = Once::new();

// TODO(@Oscar): Replace with `specta::json`
#[derive(Serialize, Type)]
pub struct LibraryConfigWrapped {
	pub uuid: Uuid,
	pub instance_id: Uuid,
	pub instance_public_key: RemoteIdentity,
	pub config: LibraryConfig,
}

impl Model for LibraryConfigWrapped {
	fn name() -> &'static str {
		"LibraryConfigWrapped"
	}
}

impl LibraryConfigWrapped {
	pub async fn from_library(library: &Library) -> Self {
		Self {
			uuid: library.id,
			instance_id: library.instance_uuid,
			instance_public_key: library.identity.to_remote_identity(),
			config: library.config().await,
		}
	}
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.query(|node, _: ()| async move {
				let libraries = node
					.libraries
					.get_all()
					.await
					.into_iter()
					.map(|lib| async move {
						LibraryConfigWrapped {
							uuid: lib.id,
							instance_id: lib.instance_uuid,
							instance_public_key: lib.identity.to_remote_identity(),
							config: lib.config().await,
						}
					})
					.collect::<Vec<_>>()
					.join()
					.await;

				let (nodes, items) = libraries.normalise(|i| i.uuid.to_string());

				Ok(NormalisedResults { nodes, items })
			})
		})
		.procedure("statistics", {
			R.with2(library())
				.query(|(node, library), _: ()| async move {
					let statistics = library
						.db
						.statistics()
						.find_unique(statistics::id::equals(1))
						.exec()
						.await?;

					STATISTICS_UPDATER.call_once(move || {
						spawn(async move {
							loop {
								if let Err(e) = update_library_statistics(
									Arc::clone(&node),
									Arc::clone(&library),
								)
								.await
								{
									error!("Failed to update library statistics: {e:#?}");
								} else {
									invalidate_query!(&library, "library.statistics");
								}

								sleep(Duration::from_secs(60)).await;
							}
						});
					});

					Ok(statistics)
				})
		})
		.procedure("kindStatistics", {
			#[derive(Serialize, Deserialize, Type, Default)]
			pub struct KindStatistic {
				kind: i32,
				name: String,
				count: i32,
				total_bytes: String,
			}
			#[derive(Serialize, Deserialize, Type, Default)]
			pub struct KindStatistics {
				statistics: Vec<KindStatistic>,
			}
			R.with2(library()).query(|(_, library), _: ()| async move {
				let mut statistics: Vec<KindStatistic> = vec![];
				for kind in ObjectKind::iter() {
					let count = library
						.db
						.object()
						.count(vec![object::kind::equals(Some(kind as i32))])
						.exec()
						.await?;

					statistics.push(KindStatistic {
						kind: kind as i32,
						name: kind.to_string(),
						count: count as i32,
						total_bytes: "0".to_string(),
					});
				}

				Ok(KindStatistics { statistics })
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

					Ok(NormalisedResult::from(
						LibraryConfigWrapped::from_library(&library).await,
						|l| l.uuid.to_string(),
					))
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

			R.mutation(
				|node,
				 EditLibraryArgs {
				     id,
				     name,
				     description,
				 }: EditLibraryArgs| async move {
					Ok(node.libraries.edit(id, name, description).await?)
				},
			)
		})
		.procedure(
			"delete",
			R.mutation(|node, id: Uuid| async move {
				node.libraries.delete(&id).await.map_err(Into::into)
			}),
		)
		.procedure(
			"actors",
			R.with2(library()).subscription(|(_, library), _: ()| {
				let mut rx = library.actors.invalidate_rx.resubscribe();

				async_stream::stream! {
					let actors = library.actors.get_state().await;
					yield actors;

					while let Ok(()) = rx.recv().await {
						let actors = library.actors.get_state().await;
						yield actors;
					}
				}
			}),
		)
		.procedure(
			"startActor",
			R.with2(library())
				.mutation(|(_, library), name: String| async move {
					library.actors.start(&name).await;

					Ok(())
				}),
		)
		.procedure(
			"stopActor",
			R.with2(library())
				.mutation(|(_, library), name: String| async move {
					library.actors.stop(&name).await;

					Ok(())
				}),
		)
}
