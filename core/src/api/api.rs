use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::util::http::ensure_response;

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

			|node, args: Feedback| async move {
				dbg!(&args);

				node.http
					.post(&format!("{}/api/v1/feedback", &node.env.api_url))
					.json(&args)
					.send()
					.await
					.map_err(|_| {
						rspc::Error::new(
							rspc::ErrorCode::InternalServerError,
							"Request failed".to_string(),
						)
					})
					.and_then(ensure_response)?;

				Ok(())
			}
		}),
	)
}
