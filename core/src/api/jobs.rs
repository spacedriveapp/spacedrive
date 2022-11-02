use crate::{
	job::{Job, JobManager},
	location::{fetch_location, LocationError},
	object::{
		identifier_job::full_identifier_job::{FullFileIdentifierJob, FullFileIdentifierJobInit},
		preview::{ThumbnailJob, ThumbnailJobInit},
		validation::validator_job::{ObjectValidatorJob, ObjectValidatorJobInit},
	},
	prisma::location,
};

use rspc::{ErrorCode, Type};
use serde::Deserialize;
use std::path::PathBuf;

use super::{utils::LibraryRequest, CoreEvent, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_query("getRunning", |t| {
			t(|ctx, _: (), _| async move { Ok(ctx.jobs.get_running().await) })
		})
		.library_query("isRunning", |t| {
			t(|ctx, _: (), _| async move { Ok(!ctx.jobs.get_running().await.is_empty()) })
		})
		.library_query("getHistory", |t| {
			t(|_, _: (), library| async move { Ok(JobManager::get_history(&library).await?) })
		})
		.library_mutation("clearAll", |t| {
			t(|_, _: (), library| async move {
				JobManager::clear_all_jobs(&library).await?;
				Ok(())
			})
		})
		.library_mutation("generateThumbsForLocation", |t| {
			#[derive(Type, Deserialize)]
			pub struct GenerateThumbsForLocationArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			t(
				|_, args: GenerateThumbsForLocationArgs, library| async move {
					if library
						.db
						.location()
						.count(vec![location::id::equals(args.id)])
						.exec()
						.await? == 0
					{
						return Err(LocationError::IdNotFound(args.id).into());
					}

					library
						.spawn_job(Job::new(
							ThumbnailJobInit {
								location_id: args.id,
								root_path: PathBuf::new(),
								background: true,
							},
							ThumbnailJob {},
						))
						.await;

					Ok(())
				},
			)
		})
		.library_mutation("objectValidator", |t| {
			#[derive(Type, Deserialize)]
			pub struct ObjectValidatorArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			t(|_, args: ObjectValidatorArgs, library| async move {
				if fetch_location(&library, args.id).exec().await?.is_none() {
					return Err(rspc::Error::new(
						ErrorCode::NotFound,
						"Location not found".into(),
					));
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
		.library_mutation("identifyUniqueFiles", |t| {
			#[derive(Type, Deserialize)]
			pub struct IdentifyUniqueFilesArgs {
				pub id: i32,
				pub path: PathBuf,
			}

			t(|_, args: IdentifyUniqueFilesArgs, library| async move {
				if fetch_location(&library, args.id).exec().await?.is_none() {
					return Err(rspc::Error::new(
						ErrorCode::NotFound,
						"Location not found".into(),
					));
				}

				library
					.spawn_job(Job::new(
						FullFileIdentifierJobInit {
							location_id: args.id,
							sub_path: Some(args.path),
						},
						FullFileIdentifierJob {},
					))
					.await;

				Ok(())
			})
		})
		.library_subscription("newThumbnail", |t| {
			t(|ctx, _: (), _| {
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
			})
		})
}
