use crate::prisma::location;
use rspc::{alpha::AlphaRouter, ErrorCode};

use sd_prisma::prisma::instance;
use serde::Deserialize;
use specta::Type;
use tracing::error;
use uuid::Uuid;

use super::{locations::ExplorerItem, utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("edit", {
			#[derive(Deserialize, Type)]
			pub struct ChangeNodeNameArgs {
				pub name: Option<String>,
			}
			// TODO: validate name isn't empty or too long

			R.mutation(|node, args: ChangeNodeNameArgs| async move {
				if let Some(name) = args.name {
					if name.is_empty() || name.len() > 32 {
						return Err(rspc::Error::new(
							ErrorCode::BadRequest,
							"invalid node name".into(),
						));
					}

					node.config
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
				// TODO: I don't like this. `node_id` should probs be a machine hash or something cause `node_id` is dynamic in the context of P2P and what does it mean for removable media to be owned by a node?
				.query(|(_, library), node_id: Option<Uuid>| async move {
					// Be aware multiple instances can exist on a single node. This is generally an edge case but it's possible.
					let instances = library
						.db
						.instance()
						.find_many(vec![node_id
							.map(|id| instance::node_id::equals(id.as_bytes().to_vec()))
							.unwrap_or(instance::id::equals(library.config.instance_id))])
						.exec()
						.await?;

					Ok(library
						.db
						.location()
						.find_many(
							instances
								.into_iter()
								.map(|i| location::instance_id::equals(Some(i.id)))
								.collect(),
						)
						.exec()
						.await?
						.into_iter()
						.map(|location| ExplorerItem::Location {
							has_local_thumbnail: false,
							thumbnail_key: None,
							item: location,
						})
						.collect::<Vec<_>>())
				})
		})
}
