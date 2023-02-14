use std::sync::Arc;

use futures::Stream;
use rspc::{
	internal::{
		specta, BuiltProcedureBuilder, MiddlewareBuilderLike, RequestResult,
		UnbuiltProcedureBuilder,
	},
	ErrorCode, Type,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

use crate::{api::Ctx, library::LibraryContext};

/// Can wrap a query argument to require it to contain a `library_id` and provide helpers for working with libraries.
#[derive(Clone, Serialize, Deserialize, Type)]
pub(crate) struct LibraryArgs<T> {
	pub library_id: Uuid,
	pub arg: T,
}

// WARNING: This is system is using internal API's which means it will break between rspc release. I would avoid copying it unless you understand the cost of maintaining it!
pub trait LibraryRequest {
	fn library_query<TUnbuiltResolver, TUnbuiltResult, TUnbuiltResultMarker, TBuiltResolver, TArg>(
		self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TUnbuiltResolver>,
		) -> BuiltProcedureBuilder<TBuiltResolver>,
	) -> Self
	where
		TUnbuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send,
		TBuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send + Sync + 'static,
		TUnbuiltResult: RequestResult<TUnbuiltResultMarker> + Send,
		TArg: DeserializeOwned + specta::Type + Send + 'static;

	fn library_mutation<
		TUnbuiltResolver,
		TUnbuiltResult,
		TUnbuiltResultMarker,
		TBuiltResolver,
		TArg,
	>(
		self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TUnbuiltResolver>,
		) -> BuiltProcedureBuilder<TBuiltResolver>,
	) -> Self
	where
		TUnbuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send,
		TBuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send + Sync + 'static,
		TUnbuiltResult: RequestResult<TUnbuiltResultMarker> + Send,
		TArg: DeserializeOwned + specta::Type + Send + 'static;

	fn library_subscription<TResolver, TArg, TStream, TResult>(
		self,
		key: &'static str,
		builder: impl Fn(UnbuiltProcedureBuilder<Ctx, TResolver>) -> BuiltProcedureBuilder<TResolver>,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + 'static,
		TStream: Stream<Item = TResult> + Send + Sync + 'static,
		TResult: Serialize + specta::Type,
		TResolver: Fn(Ctx, TArg, Uuid) -> TStream + Send + Sync + 'static;
}

// Note: This will break with middleware context switching but that's fine for now
impl<TMiddleware> LibraryRequest for rspc::RouterBuilder<Ctx, TMiddleware>
where
	TMiddleware: MiddlewareBuilderLike<Ctx, LayerContext = Ctx> + Send + 'static,
{
	fn library_query<TUnbuiltResolver, TUnbuiltResult, TUnbuiltResultMarker, TBuiltResolver, TArg>(
		self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TUnbuiltResolver>,
		) -> BuiltProcedureBuilder<TBuiltResolver>,
	) -> Self
	where
		TUnbuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send,
		TBuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send + Sync + 'static,
		TUnbuiltResult: RequestResult<TUnbuiltResultMarker> + Send,
		TArg: DeserializeOwned + specta::Type + Send + 'static,
	{
		self.query(key, move |t| {
			let resolver = Arc::new(builder(UnbuiltProcedureBuilder::from_builder(&t)).resolver);

			t(move |ctx, arg: LibraryArgs<TArg>| {
				let resolver = resolver.clone();
				async move {
					let library = ctx
						.library_manager
						.get_ctx(arg.library_id)
						.await
						.ok_or_else(|| {
							rspc::Error::new(
								ErrorCode::BadRequest,
								"You must specify a valid library to use this operation."
									.to_string(),
							)
						})?;

					Ok(resolver(ctx, arg.arg, library)
						.into_request_future()?
						.exec()
						.await?)
				}
			})
		})
	}

	fn library_mutation<
		TUnbuiltResolver,
		TUnbuiltResult,
		TUnbuiltResultMarker,
		TBuiltResolver,
		TArg,
	>(
		self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TUnbuiltResolver>,
		) -> BuiltProcedureBuilder<TBuiltResolver>,
	) -> Self
	where
		TUnbuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send,
		TBuiltResolver: Fn(Ctx, TArg, LibraryContext) -> TUnbuiltResult + Send + Sync + 'static,
		TUnbuiltResult: RequestResult<TUnbuiltResultMarker> + Send,
		TArg: DeserializeOwned + specta::Type + Send + 'static,
	{
		self.mutation(key, move |t| {
			let resolver = Arc::new(builder(UnbuiltProcedureBuilder::from_builder(&t)).resolver);

			t(move |ctx, arg: LibraryArgs<TArg>| {
				let resolver = resolver.clone();
				async move {
					let library = ctx
						.library_manager
						.get_ctx(arg.library_id)
						.await
						.ok_or_else(|| {
							rspc::Error::new(
								ErrorCode::BadRequest,
								"You must specify a valid library to use this operation."
									.to_string(),
							)
						})?;

					Ok(resolver(ctx, arg.arg, library)
						.into_request_future()?
						.exec()
						.await?)
				}
			})
		})
	}

	fn library_subscription<TResolver, TArg, TStream, TResult>(
		self,
		key: &'static str,
		builder: impl Fn(UnbuiltProcedureBuilder<Ctx, TResolver>) -> BuiltProcedureBuilder<TResolver>,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + 'static,
		TStream: Stream<Item = TResult> + Send + Sync + 'static,
		TResult: Serialize + specta::Type,
		TResolver: Fn(Ctx, TArg, Uuid) -> TStream + Send + Sync + 'static,
	{
		self.subscription(key, |t| {
			let resolver = Arc::new(builder(UnbuiltProcedureBuilder::from_builder(&t)).resolver);

			t(move |ctx, arg: LibraryArgs<TArg>| {
				// TODO(@Oscar): Upstream rspc work to allow this to work
				// let library = ctx
				// 	.library_manager
				// 	.get_ctx(arg.library_id)
				// 	.await
				// 	.ok_or_else(|| {
				// 		rspc::Error::new(
				// 			ErrorCode::BadRequest,
				// 			"You must specify a valid library to use this operation.".to_string(),
				// 		)
				// 	})?;

				resolver(ctx, arg.arg, arg.library_id)
			})
		})
	}
}
