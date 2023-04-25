use crate::{
	job::JobManager,
	location::{find_location, LocationError},
	object::{
		file_identifier::file_identifier_job::FileIdentifierJobInit,
		preview::thumbnailer_job::ThumbnailerJobInit,
		validation::validator_job::ObjectValidatorJobInit,
	},
};

use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

use super::{utils::library, CoreEvent, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("getRunning", {
			R.with2(library())
				.query(|(ctx, _), _: ()| async move { Ok(ctx.jobs.get_running().await) })
		})
		.procedure("getHistory", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				JobManager::get_history(&library).await.map_err(Into::into)
			})
		})
		.procedure("clear", {
			R.with2(library())
				.mutation(|(_, library), id: Uuid| async move {
					JobManager::clear_job(id, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("clearAll", {
			R.with2(library())
				.mutation(|(_, library), _: ()| async move {
					JobManager::clear_all_jobs(&library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("generateThumbsForLocation", {
			#[derive(Type, Deserialize)]
			pub struct GenerateThumbsForLocationArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			R.with2(library()).mutation(
				|(_, library), args: GenerateThumbsForLocationArgs| async move {
					let Some(location) = find_location(&library, args.id).exec().await? else {
						return Err(LocationError::IdNotFound(args.id).into());
					};

					library
						.spawn_job(ThumbnailerJobInit {
							location,
							sub_path: Some(args.path),
							background: false,
						})
						.await
						.map_err(Into::into)
				},
			)
		})
		.procedure("objectValidator", {
			#[derive(Type, Deserialize)]
			pub struct ObjectValidatorArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			R.with2(library())
				.mutation(|(_, library), args: ObjectValidatorArgs| async move {
					if find_location(&library, args.id).exec().await?.is_none() {
						return Err(LocationError::IdNotFound(args.id).into());
					}

					library
						.spawn_job(ObjectValidatorJobInit {
							location_id: args.id,
							path: args.path,
							background: true,
						})
						.await
						.map_err(Into::into)
				})
		})
		.procedure("identifyUniqueFiles", {
			#[derive(Type, Deserialize)]
			pub struct IdentifyUniqueFilesArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			R.with2(library())
				.mutation(|(_, library), args: IdentifyUniqueFilesArgs| async move {
					let Some(location) = find_location(&library, args.id).exec().await? else {
						return Err(LocationError::IdNotFound(args.id).into());
					};

					library
						.spawn_job(FileIdentifierJobInit {
							location,
							sub_path: Some(args.path),
						})
						.await
						.map_err(Into::into)
				})
		})
		.procedure("newThumbnail", {
			R.with2(library())
				.subscription(|(ctx, _), _: ()| async move {
					// TODO: Only return event for the library that was subscribed to

					let mut event_bus_rx = ctx.event_bus.0.subscribe();
					async_stream::stream! {
						while let Ok(event) = event_bus_rx.recv().await {
							match event {
								CoreEvent::NewThumbnail { cas_id } => yield cas_id,
								_ => {}
							}
						}
					}
				})
		})
}
