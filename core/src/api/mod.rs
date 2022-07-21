use std::{path::PathBuf, sync::Arc};

use rspc::{ActualMiddlewareResult, Config, ErrorCode, ExecError, MiddlewareResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
	job::JobManager,
	library::{LibraryContext, LibraryManager, Statistics},
	node::{NodeConfig, NodeConfigManager},
};

// TODO(Oscar): Allow rspc `mount()` functions to return `impl RouterBuilder` or something so we can remove these cause they are annoying
pub type Router = rspc::Router<Ctx>;
pub(crate) type RouterBuilder = rspc::RouterBuilder<Ctx>;
pub(crate) type LibraryRouter = rspc::Router<LibraryCtx>;
pub(crate) type LibraryRouterBuilder = rspc::RouterBuilder<LibraryCtx>;

// The context which is sent from the frontend to the backend.
#[derive(Deserialize)]
pub struct RequestCtx {
	pub library_id: Option<String>,
}

/// Is provided when executing the router from the request.
pub struct Ctx {
	pub library_id: Option<String>,
	pub library_manager: Arc<LibraryManager>,
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
}

/// Is the context provided to queries scoped to a library. This is done through rspc context switching.
pub(super) struct LibraryCtx {
	pub library: LibraryContext,
	pub library_manager: Arc<LibraryManager>,
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
}

mod files;
mod jobs;
mod libraries;
mod locations;
mod tags;
mod volumes;

// TODO: replace with with the selection macro
#[derive(Serialize, Deserialize, Debug, TS)]
pub struct NodeState {
	#[serde(flatten)]
	pub config: NodeConfig,
	pub data_path: String,
}

pub(crate) fn mount() -> Arc<Router> {
	<Router>::new()
		// This messes with Tauri's hot reload so we can't use it until their is a solution upstream. https://github.com/tauri-apps/tauri/issues/4617
		.config(
			Config::new()
				.export_ts_bindings(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./bindings")),
		) // Note: This is relative to directory the command was executed in. // TODO: Change it to be relative to Cargo.toml by default
		.query("version", |_, _: ()| env!("CARGO_PKG_VERSION"))
		.query("getNode", |ctx, _: ()| async move {
			NodeState {
				config: ctx.config.get().await,
				data_path: ctx.config.data_directory().to_str().unwrap().to_string(),
			}
		})
		.merge("library.", libraries::mount())
		.merge("volumes.", volumes::mount())
		// The middleware gets the library context and changes the router context to be LibraryCtx.
		// All library specific operations should be defined below this middleware and non-library operations should be defined above.
		.middleware(|ctx, next| async move {
			match ctx.library_id {
				Some(library_id) => match ctx.library_manager.get_ctx(library_id).await {
					Some(library) => match next(LibraryCtx {
						library,
						library_manager: ctx.library_manager,
						config: ctx.config,
						jobs: ctx.jobs,
					})? {
						MiddlewareResult::Stream(stream) => Ok(stream.into_middleware_result()),
						result => Ok(result.await?.into_middleware_result()),
					},

					None => Err(ExecError::ErrResolverError(rspc::Error::new(
						ErrorCode::BadRequest,
						"You must specify a library to use this operation.".to_string(),
					))),
				},
				None => Err(ExecError::ErrResolverError(rspc::Error::new(
					ErrorCode::BadRequest,
					"You must specify a library to use this operation.".to_string(),
				))),
			}
		})
		// I hate that this is here. We need something like trpc V10 reusable procedures to work around that. It is here cause we need the ctx returned from the middleware.
		.query("getLibraryStatistics", |ctx, _: ()| async move {
			Statistics::calculate(&ctx.library).await.unwrap()
		})
		.merge("tags.", tags::mount())
		.merge("locations.", locations::mount())
		.merge("files.", files::mount())
		.merge("jobs.", jobs::mount())
		.build()
		.arced()
}
