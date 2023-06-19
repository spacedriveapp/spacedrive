use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use tracing::error;

use crate::api::R;

use super::Ctx;

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("changeNodeName", {
		#[derive(Deserialize, Type)]
		pub struct ChangeNodeNameArgs {
			pub name: String,
		}
		// TODO: validate name isn't empty or too long

		R.mutation(|ctx, args: ChangeNodeNameArgs| async move {
			ctx.config
				.write(|mut config| {
					config.name = args.name;
				})
				.await
				.map_err(|err| {
					error!("Failed to write config: {}", err);
					rspc::Error::new(
						ErrorCode::InternalServerError,
						"error updating config".into(),
					)
				})
				.map(|_| ())
		})
	})
}
