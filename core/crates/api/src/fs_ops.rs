use crate::{
    api::utils::library,
    invalidate_query,
    library::Library,
    location::LocationError,
};

use sd_file_actions::{
    copier::CopyJob,
    deleter::{MoveToTrashJob, RemoveJob},
    mover::CutJob,
};

use sd_prisma::prisma::{location, PrismaClient};
use sd_utils::error::FileIOError;

use std::path::{Path, PathBuf};

use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::error;

use super::{Ctx, R};

#[derive(Type, Deserialize)]
pub struct FileOperationArgs {
    sources: Vec<PathBuf>,
    target_dir: PathBuf,
    location_id: Option<location::id::Type>,
}

#[derive(Type, Deserialize)]
pub struct DeleteArgs {
    paths: Vec<PathBuf>,
    location_id: Option<location::id::Type>,
    use_trash: bool,
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
    R.router()
        .procedure("copy", {
            R.with2(library()).mutation(|(ctx, library), args: FileOperationArgs| async move {
                let job = if let Some(location_id) = args.location_id {
                    // Location-bound copy
                    CopyJob::new_in_location(
                        args.sources,
                        args.target_dir,
                        location_id,
                        true,
                    )
                } else {
                    // Ephemeral copy
                    CopyJob::new_ephemeral(
                        args.sources,
                        args.target_dir,
                    )
                };

                library
                    .job_manager
                    .dispatch(job, args.location_id)
                    .await
                    .map_err(|e| {
                        error!(?e, "Failed to dispatch copy job");
                        rspc::Error::with_cause(
                            ErrorCode::InternalServerError,
                            "Failed to dispatch copy job".to_string(),
                            e,
                        )
                    })?;

                invalidate_query!(library, "search.paths");
                if args.location_id.is_none() {
                    invalidate_query!(library, "search.ephemeralPaths");
                }

                Ok(())
            })
        })
        .procedure("cut", {
            R.with2(library()).mutation(|(ctx, library), args: FileOperationArgs| async move {
                let job = if let Some(location_id) = args.location_id {
                    // Location-bound cut
                    CutJob::new_in_location(
                        args.sources,
                        args.target_dir,
                        location_id,
                        true,
                    )
                } else {
                    // Ephemeral cut
                    CutJob::new_ephemeral(
                        args.sources,
                        args.target_dir,
                    )
                };

                library
                    .job_manager
                    .dispatch(job, args.location_id)
                    .await
                    .map_err(|e| {
                        error!(?e, "Failed to dispatch cut job");
                        rspc::Error::with_cause(
                            ErrorCode::InternalServerError,
                            "Failed to dispatch cut job".to_string(),
                            e,
                        )
                    })?;

                invalidate_query!(library, "search.paths");
                if args.location_id.is_none() {
                    invalidate_query!(library, "search.ephemeralPaths");
                }

                Ok(())
            })
        })
        .procedure("delete", {
            R.with2(library()).mutation(|(ctx, library), args: DeleteArgs| async move {
                let job = if args.use_trash {
                    // Move to trash
                    MoveToTrashJob::new(args.paths, args.location_id)
                } else {
                    // Permanent delete
                    RemoveJob::new(args.paths, args.location_id)
                };

                library
                    .job_manager
                    .dispatch(job, args.location_id)
                    .await
                    .map_err(|e| {
                        error!(?e, "Failed to dispatch delete job");
                        rspc::Error::with_cause(
                            ErrorCode::InternalServerError,
                            "Failed to dispatch delete job".to_string(),
                            e,
                        )
                    })?;

                invalidate_query!(library, "search.paths");
                if args.location_id.is_none() {
                    invalidate_query!(library, "search.ephemeralPaths");
                }

                Ok(())
            })
        })
}
