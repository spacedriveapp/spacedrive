use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use tracing::error;

use crate::api::R;

use super::Ctx;

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("edit", {
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
}
