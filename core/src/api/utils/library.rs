use rspc::{
	internal::middleware::{Middleware, SealedMiddleware},
	unstable::{MwArgMapper, MwArgMapperMiddleware},
	ErrorCode,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{api::Ctx, library::Library};

/// Can wrap a query argument to require it to contain a `library_id` and provide helpers for working with libraries.
#[derive(Clone, Serialize, Deserialize, Type)]
pub(crate) struct LibraryArgs<T> {
	library_id: Uuid,
	arg: T,
}

pub(crate) struct LibraryArgsLike;
impl MwArgMapper for LibraryArgsLike {
	type Input<T> = LibraryArgs<T> where T: Type + DeserializeOwned + 'static;
	type State = Uuid;

	fn map<T: Serialize + DeserializeOwned + Type + 'static>(
		arg: Self::Input<T>,
	) -> (T, Self::State) {
		(arg.arg, arg.library_id)
	}
}
pub(crate) fn library() -> impl Middleware<Ctx> + SealedMiddleware<Ctx, NewCtx = (Ctx, Library)> {
	MwArgMapperMiddleware::<LibraryArgsLike>::new().mount(|mw, ctx: Ctx, library_id| async move {
		let library = ctx
			.library_manager
			.get_library(library_id)
			.await
			.ok_or_else(|| {
				rspc::Error::new(
					ErrorCode::BadRequest,
					"You must specify a valid library to use this operation.".to_string(),
				)
			})?;

		// let library_id = library_id.to_string();
		// let span = match mw.req.kind {
		// 	ProcedureKind::Query => {
		// 		let query = mw.req.path.as_ref();
		// 		tracing::info_span!("rspc", query, library_id)
		// 	}
		// 	ProcedureKind::Mutation => {
		// 		let mutation = mw.req.path.as_ref();
		// 		tracing::info_span!("rspc", mutation, library_id)
		// 	}
		// 	ProcedureKind::Subscription => {
		// 		let subscription = mw.req.path.as_ref();
		// 		tracing::info_span!("rspc", subscription, library_id)
		// 	}
		// };

		// .with_span(Some(span)) // TODO: Reenable this once we move off the tracing fork.

		Ok(mw.next((ctx, library)))
	})
}
