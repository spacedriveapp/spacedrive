use rspc::{alpha::AlphaRouter, Type};
use serde::{Deserialize, Serialize};

use super::{t, Ctx};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	t.router().procedure("tokenizeSensitiveKey", {
		#[derive(Deserialize, Type)]
		pub struct TokenizeKeyArgs {
			pub secret_key: String,
		}
		#[derive(Serialize, Type)]
		pub struct TokenizeResponse {
			pub token: String,
		}

		t.mutation(|ctx, args: TokenizeKeyArgs| async move {
			let token = ctx.secure_temp_keystore.tokenize(args.secret_key);

			Ok(TokenizeResponse {
				token: token.to_string(),
			})
		})
	})
}
