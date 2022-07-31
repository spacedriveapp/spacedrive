use std::path::PathBuf;

use rspc::Type;
use serde::Deserialize;

use crate::{
	encode::{ThumbnailJob, ThumbnailJobInit},
	file::cas::{FileIdentifierJob, FileIdentifierJobInit},
	job::{Job, JobManager},
};

use super::{CoreEvent, LibraryArgs, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct GenerateThumbsForLocationArgs {
	pub id: i32,
	pub path: PathBuf,
}

#[derive(Type, Deserialize)]
pub struct IdentifyUniqueFilesArgs {
	pub id: i32,
	pub path: PathBuf,
}

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.query("getRunning", |ctx, arg: LibraryArgs<()>| async move {
			let (_, _) = arg.get_library(&ctx).await?;

			Ok(ctx.jobs.get_running().await)
		})
		.query("getHistory", |ctx, arg: LibraryArgs<()>| async move {
			let (_, library) = arg.get_library(&ctx).await?;

			Ok(JobManager::get_history(&library).await?)
		})
		.mutation(
			"generateThumbsForLocation",
			|ctx, arg: LibraryArgs<GenerateThumbsForLocationArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				library
					.spawn_job(Job::new(
						ThumbnailJobInit {
							location_id: args.id,
							path: args.path,
							background: false, // fix
						},
						Box::new(ThumbnailJob {}),
					))
					.await;

				Ok(())
			},
		)
		.mutation(
			"identifyUniqueFiles",
			|ctx, arg: LibraryArgs<IdentifyUniqueFilesArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				library
					.spawn_job(Job::new(
						FileIdentifierJobInit {
							location_id: args.id,
							path: args.path,
						},
						Box::new(FileIdentifierJob {}),
					))
					.await;

				Ok(())
			},
		)
		.subscription("newThumbnail", |ctx, _: LibraryArgs<()>| {
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
}
