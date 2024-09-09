use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure(
		"sendFeedback",
		R.mutation({
			#[derive(Debug, Type, Serialize, Deserialize)]
			struct Feedback {
				message: String,
				emoji: u8,
			}

			|_node, _args: Feedback| async move {
				// sd_cloud_api::feedback::send(
				// 	node.cloud_api_config().await,
				// 	args.message,
				// 	args.emoji,
				// )
				// .await?;

				Ok(())
			}
		}),
	)
}
