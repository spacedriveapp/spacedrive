use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::util::http::ensure_response;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure(
		"sendFeedback",
		R.mutation({
			#[derive(Type, Serialize, Deserialize)]
			struct Feedback {
				message: String,
				emoji: u8,
			}

			|node, args: Feedback| async move {
				node.authed_api_request(
					node.http
						.post(&format!("{}/api/v1/feedback", &node.env.api_url))
						.json(&args),
				)
				.await
				.and_then(ensure_response)?;

				Ok(())
			}
		}),
	)
}
