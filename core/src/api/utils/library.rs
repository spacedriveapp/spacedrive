use std::{borrow::Cow, marker::PhantomData, panic::Location, process, sync::Arc};

use rspc::{
	internal::{
		BuiltProcedureBuilder, LayerResult, MiddlewareBuilderLike, ResolverLayer,
		UnbuiltProcedureBuilder,
	},
	is_invalid_procedure_name, typedef, ErrorCode, ExecError, RequestLayer, StreamRequestLayer,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{api::Ctx, library::Library};

/// Can wrap a query argument to require it to contain a `library_id` and provide helpers for working with libraries.
#[derive(Clone, Serialize, Deserialize, Type)]
pub(crate) struct LibraryArgs<T> {
	pub library_id: Uuid,
	pub arg: T,
}

// WARNING: This is system is using internal API's which means it will break between rspc release. I would avoid copying it unless you understand the cost of maintaining it!
pub trait LibraryRequest {
	fn library_query<TResolver, TArg, TResult, TResultMarker>(
		self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TResolver>,
		) -> BuiltProcedureBuilder<TResolver>,
	) -> Self
	where
		TArg: DeserializeOwned + Type + Send + 'static,
		TResult: RequestLayer<TResultMarker> + Send,
		TResolver: Fn(Ctx, TArg, Library) -> TResult + Send + Sync + 'static;

	fn library_mutation<TResolver, TArg, TResult, TResultMarker>(
		self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TResolver>,
		) -> BuiltProcedureBuilder<TResolver>,
	) -> Self
	where
		TArg: DeserializeOwned + Type + Send + 'static,
		TResult: RequestLayer<TResultMarker> + Send,
		TResolver: Fn(Ctx, TArg, Library) -> TResult + Send + Sync + 'static;

	fn library_subscription<F, TArg, TResult, TResultMarker>(
		self,
		key: &'static str,
		builder: impl FnOnce(UnbuiltProcedureBuilder<Ctx, F>) -> BuiltProcedureBuilder<F>,
	) -> Self
	where
		F: Fn(Ctx, TArg, Library) -> TResult + Send + Sync + 'static,
		TArg: DeserializeOwned + Type + Send + 'static,
		TResult: StreamRequestLayer<TResultMarker> + Send;
}

// Note: This will break with middleware context switching but that's fine for now
impl<TMiddleware> LibraryRequest for rspc::RouterBuilder<Ctx, (), TMiddleware>
where
	TMiddleware: MiddlewareBuilderLike<Ctx, LayerContext = Ctx> + Send + 'static,
{
	fn library_query<TResolver, TArg, TResult, TResultMarker>(
		mut self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TResolver>,
		) -> BuiltProcedureBuilder<TResolver>,
	) -> Self
	where
		TArg: DeserializeOwned + Type + Send + 'static,
		TResult: RequestLayer<TResultMarker> + Send,
		TResolver: Fn(Ctx, TArg, Library) -> TResult + Send + Sync + 'static,
	{
		if is_invalid_procedure_name(key) {
			eprintln!(
                "{}: rspc error: attempted to attach a query with the key '{}', however this name is not allowed. ",
                Location::caller(),
                key
            );
			process::exit(1);
		}

		let resolver = Arc::new(builder(UnbuiltProcedureBuilder::default()).resolver);
		let ty =
			typedef::<LibraryArgs<TArg>, TResult::Result>(Cow::Borrowed(key), self.typ_store())
				.unwrap();
		let layer = self.prev_middleware().build(ResolverLayer {
			func: move |ctx: Ctx, input, _| {
				let resolver = resolver.clone();
				Ok(LayerResult::FutureValueOrStream(Box::pin(async move {
					let args: LibraryArgs<TArg> =
						serde_json::from_value(input).map_err(ExecError::DeserializingArgErr)?;

					let library = ctx
						.library_manager
						.get_ctx(args.library_id)
						.await
						.ok_or_else(|| {
							rspc::Error::new(
								ErrorCode::BadRequest,
								"You must specify a valid library to use this operation."
									.to_string(),
							)
						})?;

					Ok(resolver(ctx, args.arg, library)
						.into_layer_result()?
						.into_value_or_stream()
						.await?)
				})))
			},
			phantom: PhantomData,
		});
		self.queries().append(key.into(), layer, ty);
		self
	}

	fn library_mutation<TResolver, TArg, TResult, TResultMarker>(
		mut self,
		key: &'static str,
		builder: impl FnOnce(
			UnbuiltProcedureBuilder<Ctx, TResolver>,
		) -> BuiltProcedureBuilder<TResolver>,
	) -> Self
	where
		TArg: DeserializeOwned + Type + Send + 'static,
		TResult: RequestLayer<TResultMarker> + Send,
		TResolver: Fn(Ctx, TArg, Library) -> TResult + Send + Sync + 'static,
	{
		if is_invalid_procedure_name(key) {
			eprintln!(
                "{}: rspc error: attempted to attach a mutation with the key '{}', however this name is not allowed. ",
                Location::caller(),
                key
            );
			process::exit(1);
		}

		let resolver = Arc::new(builder(UnbuiltProcedureBuilder::default()).resolver);
		let ty =
			typedef::<LibraryArgs<TArg>, TResult::Result>(Cow::Borrowed(key), self.typ_store())
				.unwrap();
		let layer = self.prev_middleware().build(ResolverLayer {
			func: move |ctx: Ctx, input, _| {
				let resolver = resolver.clone();
				Ok(LayerResult::FutureValueOrStream(Box::pin(async move {
					let args: LibraryArgs<TArg> =
						serde_json::from_value(input).map_err(ExecError::DeserializingArgErr)?;

					let library = ctx
						.library_manager
						.get_ctx(args.library_id)
						.await
						.ok_or_else(|| {
							rspc::Error::new(
								ErrorCode::BadRequest,
								"You must specify a valid library to use this operation."
									.to_string(),
							)
						})?;

					Ok(resolver(ctx, args.arg, library)
						.into_layer_result()?
						.into_value_or_stream()
						.await?)
				})))
			},
			phantom: PhantomData,
		});
		self.mutations().append(key.into(), layer, ty);
		self
	}

	fn library_subscription<F, TArg, TResult, TResultMarker>(
		mut self,
		key: &'static str,
		builder: impl FnOnce(UnbuiltProcedureBuilder<Ctx, F>) -> BuiltProcedureBuilder<F>,
	) -> Self
	where
		F: Fn(Ctx, TArg, Library) -> TResult + Send + Sync + 'static,
		TArg: DeserializeOwned + Type + Send + 'static,
		TResult: StreamRequestLayer<TResultMarker> + Send,
	{
		if is_invalid_procedure_name(key) {
			eprintln!(
                "{}: rspc error: attempted to attach a subscription with the key '{}', however this name is not allowed. ",
                Location::caller(),
                key
            );
			process::exit(1);
		}

		let resolver = Arc::new(builder(UnbuiltProcedureBuilder::default()).resolver);
		let ty =
			typedef::<LibraryArgs<TArg>, TResult::Result>(Cow::Borrowed(key), self.typ_store())
				.unwrap();
		let layer = self.prev_middleware().build(ResolverLayer {
			func: move |ctx: Ctx, input, _| {
				let resolver = resolver.clone();
				Ok(LayerResult::FutureValueOrStream(Box::pin(async move {
					let args: LibraryArgs<TArg> =
						serde_json::from_value(input).map_err(ExecError::DeserializingArgErr)?;

					let library = ctx
						.library_manager
						.get_ctx(args.library_id)
						.await
						.ok_or_else(|| {
							rspc::Error::new(
								ErrorCode::BadRequest,
								"You must specify a valid library to use this operation."
									.to_string(),
							)
						})?;

					Ok(resolver(ctx, args.arg, library)
						.into_layer_result()?
						.into_value_or_stream()
						.await?)
				})))
			},
			phantom: PhantomData,
		});
		self.subscriptions().append(key.into(), layer, ty);
		self
	}
}
