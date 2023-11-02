use std::sync::Arc;

use rspc::{
	internal::middleware::{ArgMapper, ArgumentMapper, Middleware},
	ErrorCode,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	api::{Ctx, SdError},
	library::Library,
};

/// Can wrap a query argument to require it to contain a `library_id` and provide helpers for working with libraries.
#[derive(Clone, Serialize, Deserialize, Type)]
pub struct LibraryArgs<T> {
	library_id: Uuid,
	arg: T,
}

pub enum LibraryArgsMapper {}

impl ArgumentMapper for LibraryArgsMapper {
	type State = Uuid;
	type Input<T> = LibraryArgs<T>
    where
        T: DeserializeOwned + Type + 'static;

	fn map<T: Serialize + DeserializeOwned + Type + 'static>(
		arg: Self::Input<T>,
	) -> (T, Self::State) {
		(arg.arg, arg.library_id)
	}
}

pub(crate) fn library() -> impl Middleware<Ctx, NewCtx = (Ctx, Arc<Library>)> {
	// TODO: Remove `ctx: Ctx` thing
	ArgMapper::<LibraryArgsMapper>::new(|mw, ctx: Ctx, library_id| async move {
		let library = ctx
			.libraries
			.get_library(&library_id)
			.await
			.ok_or_else(|| {
				rspc::Error::new(
					ErrorCode::BadRequest,
					"You must specify a valid library to use this operation.".to_string(),
				)
			})?; // TODO: Error handling

		Ok::<_, SdError>(mw.next((ctx, library)))
	})
}
