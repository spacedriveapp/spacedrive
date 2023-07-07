use crate::prisma::{location, node};
use rspc::{alpha::AlphaRouter, ErrorCode};

use serde::Deserialize;
use specta::Type;
use tracing::error;

use super::{locations::ExplorerItem, utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("edit", {
			#[derive(Deserialize, Type)]
			pub struct ChangeNodeNameArgs {
				pub name: Option<String>,
			}
			// TODO: validate name isn't empty or too long

			R.mutation(|ctx, args: ChangeNodeNameArgs| async move {
				if let Some(name) = args.name {
					if name.is_empty() || name.len() > 32 {
						return Err(rspc::Error::new(
							ErrorCode::BadRequest,
							"invalid node name".into(),
						));
					}

					ctx.config
						.write(|mut config| {
							config.name = name;
						})
						.await
						.map_err(|err| {
							error!("Failed to write config: {}", err);
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"error updating config".into(),
							)
						})?;
				}

				Ok(())
			})
		})
		// TODO: add pagination!! and maybe ordering etc
		.procedure("listLocations", {
			R.with2(library())
				.query(|(ctx, library), _node_id: Option<String>| async move {
					// 1. grab currently active node
					let node_config = ctx.config.get().await;
					let node_pub_id = node_config.id.as_bytes().to_vec();
					// 2. get node from database
					// TODO: Nodes table is being deprecated so this will be broken for now
					let node = library
						.db
						.node()
						.find_unique(node::pub_id::equals(node_pub_id))
						.exec()
						.await?;

					if let Some(node) = node {
						// query for locations with that node id
						let locations: Vec<ExplorerItem> = library
							.db
							.location()
							.find_many(vec![location::node_id::equals(Some(node.id))])
							.exec()
							.await?
							.into_iter()
							.map(|location| ExplorerItem::Location {
								has_local_thumbnail: false,
								thumbnail_key: None,
								item: location,
							})
							.collect();

						return Ok(locations);
					}

					Ok(vec![])
				})
		})
}
