use super::RouterBuilder;
use rspc::Type;
use serde::{Deserialize, Serialize};

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new().mutation("tokenizeSensitiveKey", |t| {
		#[derive(Deserialize, Type)]
		pub struct TokenizeKeyArgs {
			pub secret_key: String,
		}
		#[derive(Serialize, Type)]
		pub struct TokenizeResponse {
			pub token: String,
		}

		t(|ctx, args: TokenizeKeyArgs| async move {
			let token = ctx.secure_temp_keystore.tokenize(args.secret_key);

			Ok(TokenizeResponse {
				token: token.to_string(),
			})
		})
	})
}
