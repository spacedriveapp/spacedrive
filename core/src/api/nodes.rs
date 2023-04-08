use super::RouterBuilder;
use rspc::Type;
use serde::{Deserialize, Serialize};

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.mutation("tokenizeSensitiveKey", |t| {
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
		// change node name
		.mutation("changeNodeName", |t| {
			#[derive(Deserialize, Type)]
			pub struct ChangeNodeNameArgs {
				pub name: String,
			}
			// TODO: validate name isn't empty or too long

			t(|ctx, args: ChangeNodeNameArgs| async move {
				ctx.config
					.write(|mut config| {
						config.name = args.name;
					})
					.await;

				Ok(())
			})
		})
}
