use crate::{
	api::CoreEvent,
	invalidate_query,
	library::{Library, LibraryConfig, LibraryName},
	location::{scan_location, LocationCreateArgs, ScanState},
	util::MaybeUndefined,
	Node,
};

use sd_core_heavy_lifting::JobId;

use sd_file_ext::kind::ObjectKind;
use sd_p2p::RemoteIdentity;
use sd_prisma::prisma::{file_path, indexer_rule, object, object_kind_statistics, statistics};
use sd_utils::{db::size_in_bytes_from_db, u64_to_frontend};

use std::{
	collections::{hash_map::Entry, HashMap},
	convert::identity,
	pin::pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, LazyLock,
	},
	time::Duration,
};

use async_channel as chan;
use directories::UserDirs;
use futures::{FutureExt, StreamExt};
use futures_concurrency::{
	future::{Join, TryJoin},
	stream::Merge,
};
use prisma_client_rust::{raw, QueryError};
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use strum::IntoEnumIterator;
use tokio::{
	spawn,
	sync::{Mutex, RwLock},
	time::{interval, Instant},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{utils::library, Ctx, R};

const ONE_MINUTE: Duration = Duration::from_secs(60);
const TWO_MINUTES: Duration = Duration::from_secs(60 * 2);
const FIVE_MINUTES: Duration = Duration::from_secs(60 * 5);

static IS_COMPUTING_KIND_STATISTICS: AtomicBool = AtomicBool::new(false);
static LAST_KIND_STATISTICS_UPDATE: LazyLock<RwLock<TotalFilesStatistics>> =
	LazyLock::new(RwLock::default);

static STATISTICS_UPDATERS: LazyLock<Mutex<HashMap<Uuid, chan::Sender<Instant>>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

struct TotalFilesStatistics {
	updated_at: Instant,
	total_identified_files: u64,
	total_unidentified_files: u64,
}

impl Default for TotalFilesStatistics {
	fn default() -> Self {
		Self {
			updated_at: Instant::now() - FIVE_MINUTES,
			total_identified_files: 0,
			total_unidentified_files: 0,
		}
	}
}

// TODO(@Oscar): Replace with `specta::json`
#[derive(Serialize, Type)]
pub struct LibraryConfigWrapped {
	pub uuid: Uuid,
	pub instance_id: Uuid,
	pub instance_public_key: RemoteIdentity,
	pub config: LibraryConfig,
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

#[derive(Debug, Serialize, Deserialize, Type, Default, Clone)]
pub struct KindStatistic {
	kind: i32,
	name: String,
	count: (u32, u32),
	total_bytes: (u32, u32),
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.query(|node, _: ()| async move {
				Ok(node
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
					.await)
			})
		})
		.procedure("statistics", {
			#[derive(Serialize, Deserialize, Type)]
			pub struct StatisticsResponse {
				statistics: Option<statistics::Data>,
			}
			R.with2(library())
				.query(|(node, library), _: ()| async move {
					let statistics = library
						.db
						.statistics()
						.find_unique(statistics::id::equals(1))
						.exec()
						.await?;

					match STATISTICS_UPDATERS.lock().await.entry(library.id) {
						Entry::Occupied(entry) => {
							if entry.get().send(Instant::now()).await.is_err() {
								error!("Failed to send statistics update request;");
							}
						}
						Entry::Vacant(entry) => {
							let (tx, rx) = chan::bounded(1);
							entry.insert(tx);

							spawn(update_statistics_loop(node, library, rx));
						}
					}

					Ok(StatisticsResponse { statistics })
				})
		})
		.procedure("kindStatistics", {
			#[derive(Debug, Serialize, Deserialize, Type, Default)]
			pub struct KindStatistics {
				statistics: HashMap<i32, KindStatistic>,
				total_identified_files: i32,
				total_unidentified_files: i32,
			}

			R.with2(library())
				.query(|(node, library), _: ()| async move {
					let last_kind_statistics = LAST_KIND_STATISTICS_UPDATE.read().await;

					let (total_unidentified_files, total_identified_files) = if last_kind_statistics
						.updated_at
						.elapsed()
						< ONE_MINUTE
					{
						(
							last_kind_statistics.total_unidentified_files,
							last_kind_statistics.total_identified_files,
						)
					} else {
						drop(last_kind_statistics);
						let mut last_kind_statistics = LAST_KIND_STATISTICS_UPDATE.write().await;

						if let Ok(false) = IS_COMPUTING_KIND_STATISTICS.compare_exchange(
							false,
							true,
							Ordering::Release,
							Ordering::Relaxed,
						) {
							spawn({
								let library = Arc::clone(&library);
								async move {
									if let Err(e) = update_kind_statistics(node, library).await {
										error!(?e, "Failed to update kind statistics;");
									}
								}
							});
						}

						let (total_unidentified_files, total_identified_files) = (
							library
								.db
								.file_path()
								.count(vec![file_path::object_id::equals(None)])
								.exec()
								.map(|count_res| count_res.map(|count| count as u64)),
							library
								.db
								.file_path()
								.count(vec![file_path::object_id::not(None)])
								.exec()
								.map(|count_res| count_res.map(|count| count as u64)),
						)
							.try_join()
							.await?;

						last_kind_statistics.total_unidentified_files = total_unidentified_files;
						last_kind_statistics.total_identified_files = total_identified_files;
						last_kind_statistics.updated_at = Instant::now();

						(total_unidentified_files, total_identified_files)
					};

					let statistics = ObjectKind::iter()
						.map(|kind| {
							let library = Arc::clone(&library);
							async move {
								let int_kind = kind as i32;
								library
									.db
									.object_kind_statistics()
									.find_unique(object_kind_statistics::kind::equals(int_kind))
									.select(
										object_kind_statistics::select!({ total_bytes files_count }),
									)
									.exec()
									.await
									.map(|maybe_data| {
										let (files_count, total_bytes) =
											maybe_data.map_or((0, 0), |data| {
												(data.files_count as u64, data.total_bytes as u64)
											});

										(
											int_kind,
											KindStatistic {
												kind: int_kind,
												name: kind.to_string(),
												count: u64_to_frontend(files_count),
												total_bytes: u64_to_frontend(total_bytes),
											},
										)
									})
							}
						})
						.collect::<Vec<_>>()
						.try_join()
						.await?
						.into_iter()
						.collect();

					Ok(KindStatistics {
						statistics,
						total_identified_files: total_identified_files as i32,
						total_unidentified_files: total_unidentified_files as i32,
					})
				})
		})
		.procedure("updatedKindStatistic", {
			R.with2(library())
				.subscription(|(node, library), _: ()| async move {
					let mut event_bus_rx = node.event_bus.0.subscribe();
					async_stream::stream! {
						while let Ok(event) = event_bus_rx.recv().await {
							match event {
								CoreEvent::UpdatedKindStatistic(stat, library_id) if library_id == library.id => {
									yield stat
								},
								_ => {}
							}
						}
					}
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
			) -> Result<Option<JobId>, rspc::Error> {
				// If all of them are false, we skip
				if [!desktop, !documents, !downloads, !pictures, !music, !videos]
					.into_iter()
					.all(identity)
				{
					return Ok(None);
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
								return Ok(None);
							};

							let scan_state = ScanState::try_from(location.scan_state)?;

							scan_location(&node, &library, location, scan_state)
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
						error!(?e, "Failed to create default location;");
						*maybe_error = Some(e);
					}
					maybe_error
				});

				if let Some(e) = maybe_error {
					return Err(e);
				}

				debug!("Created default locations");

				Ok(None)
			}

			R.mutation(
				|node,
				 CreateLibraryArgs {
				     name,
				     default_locations,
				 }: CreateLibraryArgs| async move {
					debug!("Creating library");

					let library = node.libraries.create(name, None, &node).await?;

					debug!(%library.id, "Created library;");

					if let Some(locations) = default_locations {
						create_default_locations_on_library_creation(
							locations,
							node,
							Arc::clone(&library),
						)
						.await?;
					}

					Ok(LibraryConfigWrapped::from_library(&library).await)
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
					Ok(node
						.libraries
						.edit(id, name, description, MaybeUndefined::Undefined, None)
						.await?)
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
				let mut rx = library.cloud_sync_actors.invalidate_rx.resubscribe();

				async_stream::stream! {
					let actors = library.cloud_sync_actors.get_state().await;
					yield actors;

					while let Ok(()) = rx.recv().await {
						let actors = library.cloud_sync_actors.get_state().await;
						yield actors;
					}
				}
			}),
		)
		.procedure(
			"vacuumDb",
			R.with2(library())
				.mutation(|(_, library), _: ()| async move {
					// We retry a few times because if the DB is being actively used, the vacuum will fail
					for _ in 0..5 {
						match library.db._execute_raw(raw!("VACUUM;")).exec().await {
							Ok(_) => break,
							Err(e) => {
								warn!(
									%library.id,
									?e,
									"Failed to vacuum DB for library, retrying...;",
								);
								tokio::time::sleep(Duration::from_millis(500)).await;
							}
						}
					}

					info!(%library.id, "Successfully vacuumed DB;");

					Ok(())
				}),
		)
}

async fn update_kind_statistics(node: Arc<Node>, library: Arc<Library>) -> Result<(), QueryError> {
	async fn inner(node: Arc<Node>, library: Arc<Library>) -> Result<(), QueryError> {
		// We're processing each kind in a sequentially manner, so we can update the frontend step by step
		for kind in ObjectKind::iter() {
			let int_kind = kind as i32;
			let mut last_object_id = 0;

			let mut total_size = 0;
			let mut files_count = 0;

			loop {
				let objects = library
					.db
					.object()
					.find_many(vec![
						object::id::gt(last_object_id),
						object::kind::equals(Some(int_kind)),
					])
					.take(10_000)
					.select(object::select!({ id file_paths: select { size_in_bytes_bytes } }))
					.exec()
					.await?;

				if let Some(last) = objects.last() {
					last_object_id = last.id;
				} else {
					break; // No more objects
				}

				for object in objects {
					files_count += object.file_paths.len() as u64;
					if kind != ObjectKind::Folder {
						// We're skipping folders because their size is the sum of all their children and
						// we don't want to count them twice
						total_size += object
							.file_paths
							.into_iter()
							.map(|file_path| {
								file_path
									.size_in_bytes_bytes
									.map(|size| size_in_bytes_from_db(&size))
									.unwrap_or(0)
							})
							.sum::<u64>();
					}
				}
			}

			let (old_files_count, old_total_bytes) = library
				.db
				.object_kind_statistics()
				.find_unique(object_kind_statistics::kind::equals(int_kind))
				.select(object_kind_statistics::select!({ files_count total_bytes }))
				.exec()
				.await?
				.map_or((0, 0), |stats| {
					(stats.files_count as u64, stats.total_bytes as u64)
				});

			// Only update if the statistics changed
			if files_count != old_files_count || total_size != old_total_bytes {
				let update_params = {
					#[allow(clippy::cast_possible_wrap)]
					{
						// SAFETY: we had to store using i64 due to SQLite limitations
						vec![
							object_kind_statistics::files_count::set(files_count as i64),
							object_kind_statistics::total_bytes::set(total_size as i64),
						]
					}
				};

				library
					.db
					.object_kind_statistics()
					.upsert(
						object_kind_statistics::kind::equals(int_kind),
						object_kind_statistics::Create {
							kind: int_kind,
							_params: update_params.clone(),
						},
						update_params,
					)
					.select(object_kind_statistics::select!({ kind }))
					.exec()
					.await?;

				// Sending back the updated statistics to the frontend
				node.emit(CoreEvent::UpdatedKindStatistic(
					KindStatistic {
						kind: int_kind,
						name: kind.to_string(),
						count: u64_to_frontend(files_count),
						total_bytes: u64_to_frontend(total_size),
					},
					library.id,
				));
			}
		}

		Ok(())
	}

	let res = inner(node, library).await;

	// We finished and we can reset the flag
	IS_COMPUTING_KIND_STATISTICS.store(false, Ordering::Release);

	res
}

async fn update_statistics_loop(
	node: Arc<Node>,
	library: Arc<Library>,
	last_requested_rx: chan::Receiver<Instant>,
) {
	let mut last_received_at = Instant::now();

	let tick = interval(ONE_MINUTE);

	enum Message {
		Tick,
		Requested(Instant),
	}

	let mut msg_stream = pin!((
		IntervalStream::new(tick).map(|_| Message::Tick),
		last_requested_rx.map(Message::Requested)
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			Message::Tick => {
				// if last_received_at.elapsed() < FIVE_MINUTES {
				// 	if let Err(e) = update_library_statistics(&node, &library).await {
				// 		error!(?e, "Failed to update library statistics;");
				// 	} else {
				// 		invalidate_query!(&library, "library.statistics");
				// 	}
				// }
			}
			Message::Requested(instant) => {
				if instant - last_received_at > TWO_MINUTES {
					debug!("Updating last received at");
					last_received_at = instant;
				}
			}
		}
	}
}
