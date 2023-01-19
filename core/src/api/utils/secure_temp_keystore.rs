use super::RouterBuilder;

#[derive(Serialize, Type, Object)]
pub struct TokenizeKeyArgs {
	pub secret_key: String,
}
#[derive(Serialize, Type, Object)]
pub struct TokenizeResponse {
	pub token: String,
}

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new().mutation("tokenize", |t| {
		t(|ctx, args: TokenizeKeyArgs| async move {
			let token = ctx.keystore().tokenize(args.secret_key).await?;

			Ok(TokenizeResponse { token })
		})
	})
}
