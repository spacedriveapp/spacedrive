use crate::{
	job::{Job, JobManager},
	location::{find_location, LocationError},
	object::{
		file_identifier::file_identifier_job::{FileIdentifierJob, FileIdentifierJobInit},
		preview::thumbnailer_job::{ThumbnailerJob, ThumbnailerJobInit},
		validation::validator_job::{ObjectValidatorJob, ObjectValidatorJobInit},
	},
};

use rspc::{alpha::AlphaRouter, Type};
use serde::Deserialize;
use std::path::PathBuf;

use super::{t, utils::library, CoreEvent, Ctx};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	t.router()
		.procedure("getRunning", {
			t.with(library())
				.query(|(ctx, _), _: ()| async move { Ok(ctx.jobs.get_running().await) })
		})
		.procedure("isRunning", {
			t.with(library()).query(|(ctx, _), _: ()| async move {
				Ok(!ctx.jobs.get_running().await.is_empty())
			})
		})
		.procedure("getHistory", {
			t.with(library()).query(|(_, library), _: ()| async move {
				Ok(JobManager::get_history(&library).await?)
			})
		})
		.procedure("clearAll", {
			t.with(library())
				.mutation(|(_, library), _: ()| async move {
					JobManager::clear_all_jobs(&library).await?;
					Ok(())
				})
		})
		.procedure("generateThumbsForLocation", {
			#[derive(Type, Deserialize)]
			pub struct GenerateThumbsForLocationArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			t.with(library()).mutation(
				|(_, library), args: GenerateThumbsForLocationArgs| async move {
					let Some(location) = find_location(&library, args.id).exec().await? else {
						return Err(LocationError::IdNotFound(args.id).into());
					};

					library
						.spawn_job(Job::new(
							ThumbnailerJobInit {
								location,
								sub_path: Some(args.path),
								background: false,
							},
							ThumbnailerJob {},
						))
						.await;

					Ok(())
				},
			)
		})
		.procedure("objectValidator", {
			#[derive(Type, Deserialize)]
			pub struct ObjectValidatorArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			t.with(library())
				.mutation(|(_, library), args: ObjectValidatorArgs| async move {
					if find_location(&library, args.id).exec().await?.is_none() {
						return Err(LocationError::IdNotFound(args.id).into());
					}

					library
						.spawn_job(Job::new(
							ObjectValidatorJobInit {
								location_id: args.id,
								path: args.path,
								background: true,
							},
							ObjectValidatorJob {},
						))
						.await;

					Ok(())
				})
		})
		.procedure("identifyUniqueFiles", {
			#[derive(Type, Deserialize)]
			pub struct IdentifyUniqueFilesArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			t.with(library())
				.mutation(|(_, library), args: IdentifyUniqueFilesArgs| async move {
					let Some(location) = find_location(&library, args.id).exec().await? else {
					return Err(LocationError::IdNotFound(args.id).into());
				};

					library
						.spawn_job(Job::new(
							FileIdentifierJobInit {
								location,
								sub_path: Some(args.path),
							},
							FileIdentifierJob {},
						))
						.await;

					Ok(())
				})
		})
		.procedure(
			"newThumbnail",
			t.with(library()).subscription(|(ctx, _), _: ()| {
				// TODO: Only return event for the library that was subscribed to

				let mut event_bus_rx = ctx.event_bus.subscribe();
				async_stream::stream! {
					while let Ok(event) = event_bus_rx.recv().await {
						match event {
							CoreEvent::NewThumbnail { cas_id } => yield cas_id,
							_ => {}
						}
					}
				}
			}),
		)
}
