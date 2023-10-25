use std::sync::Arc;

use rspc::{internal::middleware::ConstrainedMiddleware, ErrorCode};
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

pub(crate) fn library() -> impl ConstrainedMiddleware<Ctx, NewCtx = (Ctx, Arc<Library>)> {
	|mw, ctx: Ctx| async move {
		// 	let library = ctx
		// 		.libraries
		// 		.get_library(&library_id)
		// 		.await
		// 		.ok_or_else(|| {
		// 			rspc::Error::new(
		// 				ErrorCode::BadRequest,
		// 				"You must specify a valid library to use this operation.".to_string(),
		// 			)
		// 		})?;

		// 	Ok(mw.next((ctx, library)))

		let library = todo!(); // TODO
		mw.next((ctx, library))
	}
}
