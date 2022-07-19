use std::path::PathBuf;

use serde::Deserialize;
use ts_rs::TS;

use crate::{encode::ThumbnailJob, file::cas::FileIdentifierJob, job::JobManager};

use super::{LibraryRouter, LibraryRouterBuilder};

#[derive(TS, Deserialize)]
pub struct GenerateThumbsForLocationArgs {
	pub id: i32,
	pub path: PathBuf,
}

#[derive(TS, Deserialize)]
pub struct IdentifyUniqueFilesArgs {
	pub id: i32,
	pub path: PathBuf,
}

pub(crate) fn mount() -> LibraryRouterBuilder {
	<LibraryRouter>::new()
		.query("getRunning", |ctx, _: ()| async move {
			ctx.jobs.get_running().await
		})
		.query("getHistory", |ctx, _: ()| async move {
			JobManager::get_history(&ctx.library).await.unwrap()
		})
		.mutation(
			"generateThumbsForLocation",
			|ctx, args: GenerateThumbsForLocationArgs| async move {
				ctx.library
					.spawn_job(Box::new(ThumbnailJob {
						location_id: args.id,
						path: args.path,
						background: false, // fix
					}))
					.await;
			},
		)
		.mutation(
			"identifyUniqueFiles",
			|ctx, args: IdentifyUniqueFilesArgs| async move {
				ctx.library
					.spawn_job(Box::new(FileIdentifierJob {
						location_id: args.id,
						path: args.path,
					}))
					.await;
			},
		)
}
