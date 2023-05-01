use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use specta::Type;

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
				.unwrap();

			Ok(())
		})
	})
}
