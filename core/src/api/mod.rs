use chrono::{DateTime, Utc};
use prisma_client_rust::{operator::or, Direction};
use rspc::{Config, Type};
use sd_file_ext::kind::ObjectKind;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
	api::locations::{file_path_with_object, ExplorerItem},
	location::LocationError,
	node::NodeConfig,
	prisma::*,
	Node,
};

use utils::{InvalidRequests, InvalidateOperationEvent};

use self::utils::LibraryRequest;

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;
pub(crate) type RouterBuilder = rspc::RouterBuilder<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail { cas_id: String },
	InvalidateOperation(InvalidateOperationEvent),
}

mod files;
mod jobs;
mod keys;
mod libraries;
mod locations;
mod nodes;
mod p2p;
mod sync;
mod tags;
pub mod utils;
pub mod volumes;

#[derive(Serialize, Deserialize, Debug, Type)]
struct NodeState {
	#[serde(flatten)]
	config: NodeConfig,
	data_path: String,
}

pub(crate) fn mount() -> Arc<Router> {
	let config = Config::new().set_ts_bindings_header("/* eslint-disable */");

	#[cfg(all(debug_assertions, not(feature = "mobile")))]
	let config = config.export_ts_bindings(
		std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packages/client/src/core.ts"),
	);

	let r = <Router>::new()
		.config(config)
		.query("buildInfo", |t| {
			#[derive(Serialize, Type)]
			pub struct BuildInfo {
				version: &'static str,
				commit: &'static str,
			}

			t(|_, _: ()| BuildInfo {
				version: env!("CARGO_PKG_VERSION"),
				commit: env!("GIT_HASH"),
			})
		})
		.query("nodeState", |t| {
			t(|ctx, _: ()| async move {
				Ok(NodeState {
					config: ctx.config.get().await,
					// We are taking the assumption here that this value is only used on the frontend for display purposes
					data_path: ctx
						.config
						.data_directory()
						.to_str()
						.expect("Found non-UTF-8 path")
						.to_string(),
				})
			})
		})
		.library_query("search", |t| {
			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			#[specta(inline)]
			enum Ordering {
				Name(bool),
			}

			impl Ordering {
				fn get_direction(&self) -> Direction {
					match self {
						Self::Name(v) => v,
					}
					.then_some(Direction::Asc)
					.unwrap_or(Direction::Desc)
				}

				fn to_param(self) -> file_path::OrderByParam {
					let dir = self.get_direction();

					use file_path::*;

					match self {
						Self::Name(_) => name::order(dir),
					}
				}
			}

			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			#[specta(inline)]
			struct Args {
				#[specta(optional)]
				location_id: Option<i32>,
				#[specta(optional)]
				after_file_id: Option<(i32, i32)>,
				#[specta(optional)]
				take: Option<i32>,
				#[specta(optional)]
				order: Option<Ordering>,
				#[specta(optional)]
				search: Option<String>,
				#[specta(optional)]
				extension: Option<String>,
				#[specta(optional)]
				kind: Option<i32>,
				#[specta(optional)]
				#[serde(default)]
				tags: Vec<i32>,
				#[specta(optional)]
				created_at_from: Option<DateTime<Utc>>,
				#[specta(optional)]
				created_at_to: Option<DateTime<Utc>>,
				#[specta(optional)]
				path: Option<String>,
			}

			t(|_, args: Args, library| async move {
				let params = args
					.search
					.map(|search| {
						search
							.split(" ")
							.map(str::to_string)
							.map(file_path::materialized_path::contains)
							.map(Some)
							.collect::<Vec<_>>()
					})
					.unwrap_or_default()
					.into_iter()
					.chain([
						args.location_id.map(file_path::location_id::equals),
						args.kind
							.map(|kind| file_path::object::is(vec![object::kind::equals(kind)])),
						args.extension.map(file_path::extension::equals),
						(args.tags.len() > 0).then(|| {
							file_path::object::is(vec![object::tags::some(vec![
								tag_on_object::tag::is(vec![or(args
									.tags
									.into_iter()
									.map(tag::id::equals)
									.collect())]),
							])])
						}),
						args.created_at_from
							.map(|v| file_path::date_created::gte(v.into())),
						args.created_at_to
							.map(|v| file_path::date_created::lte(v.into())),
						args.path.map(file_path::materialized_path::starts_with),
					])
					.flatten()
					.collect();

				let mut query = library.db.file_path().find_many(params);

				if let Some((loc_id, file_id)) = args.after_file_id {
					query = query.cursor(file_path::location_id_id(loc_id, file_id))
				}

				if let Some(order) = args.order {
					query = query.order_by(order.to_param());
				}

				if let Some(take) = args.take {
					query = query.take(take as i64);
				}

				let file_paths = query
					.include(file_path_with_object::include())
					.exec()
					.await?;

				let mut items = Vec::with_capacity(file_paths.len());

				for file_path in file_paths {
					let has_thumbnail = if let Some(cas_id) = &file_path.cas_id {
						library
							.thumbnail_exists(cas_id)
							.await
							.map_err(LocationError::IOError)?
					} else {
						false
					};

					items.push(ExplorerItem::Path {
						has_thumbnail,
						item: file_path,
					})
				}

				Ok(items)
			})
		})
		.yolo_merge("library.", libraries::mount())
		.yolo_merge("volumes.", volumes::mount())
		.yolo_merge("tags.", tags::mount())
		.yolo_merge("keys.", keys::mount())
		.yolo_merge("locations.", locations::mount())
		.yolo_merge("files.", files::mount())
		.yolo_merge("jobs.", jobs::mount())
		.yolo_merge("p2p.", p2p::mount())
		.yolo_merge("sync.", sync::mount())
		.yolo_merge("invalidation.", utils::mount_invalidate())
		.build()
		.arced();
	InvalidRequests::validate(r.clone()); // This validates all invalidation calls.

	r
}

#[cfg(test)]
mod tests {
	/// This test will ensure the rspc router and all calls to `invalidate_query` are valid and also export an updated version of the Typescript bindings.
	#[test]
	fn test_and_export_rspc_bindings() {
		super::mount();
	}
}
