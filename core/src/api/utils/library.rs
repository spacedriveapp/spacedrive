use futures::{Future, Stream};
use rspc::{internal::specta, ErrorCode, IntoLayerResult, Type};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

use crate::{api::Ctx, library::LibraryContext};

/// Can wrap a query argument to require it to contain a `library_id` and provide helpers for working with libraries.
#[derive(Clone, Serialize, Deserialize, Type)]
pub(crate) struct LibraryArgs<T> {
	pub library_id: Uuid,
	pub arg: T,
}

pub trait LibraryRequest {
	fn library_query<T, TArg, TResult, TResultMarker>(
		self,
		key: &'static str,
		resolver: fn(Ctx, TArg, LibraryContext) -> TResult,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + Send + 'static,
		TResult: Future<Output = Result<T, rspc::Error>> + Send + 'static,
		T: IntoLayerResult<TResultMarker> + Send + Serialize + specta::Type;

	fn library_mutation<T, TArg, TResult, TResultMarker>(
		self,
		key: &'static str,
		resolver: fn(Ctx, TArg, LibraryContext) -> TResult,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + Send + 'static,
		TResult: Future<Output = Result<T, rspc::Error>> + Send + 'static,
		T: IntoLayerResult<TResultMarker> + Send + Serialize + specta::Type;

	fn library_subscription<T, TArg, TResult>(
		self,
		key: &'static str,
		resolver: fn(Ctx, TArg, Uuid) -> T,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + Send + 'static,
		T: Stream<Item = TResult> + Send + 'static,
		TResult: Serialize + specta::Type;
}

// Note: This will break with middleware context switching but that's fine for now
impl LibraryRequest for rspc::RouterBuilder<Ctx, (), Ctx> {
	fn library_query<T, TArg, TResult, TResultMarker>(
		self,
		key: &'static str,
		resolver: fn(Ctx, TArg, LibraryContext) -> TResult,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + Send + 'static,
		TResult: Future<Output = Result<T, rspc::Error>> + Send + 'static,
		T: IntoLayerResult<TResultMarker> + Send + Serialize + specta::Type,
	{
		self.query(key, move |ctx, arg: LibraryArgs<TArg>| async move {
			let library = ctx
				.library_manager
				.get_ctx(arg.library_id)
				.await
				.ok_or_else(|| {
					rspc::Error::new(
						ErrorCode::BadRequest,
						"You must specify a valid library to use this operation.".to_string(),
					)
				})?;

			resolver(ctx, arg.arg, library).await
		})
	}

	fn library_mutation<T, TArg, TResult, TResultMarker>(
		self,
		key: &'static str,
		resolver: fn(Ctx, TArg, LibraryContext) -> TResult,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + Send + 'static,
		TResult: Future<Output = Result<T, rspc::Error>> + Send + 'static,
		T: IntoLayerResult<TResultMarker> + Send + Serialize + specta::Type,
	{
		self.mutation(key, move |ctx, arg: LibraryArgs<TArg>| async move {
			let library = ctx
				.library_manager
				.get_ctx(arg.library_id)
				.await
				.ok_or_else(|| {
					rspc::Error::new(
						ErrorCode::BadRequest,
						"You must specify a valid library to use this operation.".to_string(),
					)
				})?;

			resolver(ctx, arg.arg, library).await
		})
	}

	fn library_subscription<T, TArg, TResult>(
		self,
		key: &'static str,
		resolver: fn(Ctx, TArg, Uuid) -> T,
	) -> Self
	where
		TArg: DeserializeOwned + specta::Type + Send + 'static,
		T: Stream<Item = TResult> + Send + 'static,
		TResult: Serialize + specta::Type,
	{
		self.subscription(key, move |ctx, arg: LibraryArgs<TArg>| {
			// TODO: Make this fetch the library like the other functions. This needs upstream rspc work to be supported.
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
	}
}
