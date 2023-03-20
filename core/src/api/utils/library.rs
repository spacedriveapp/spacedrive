use rspc::ErrorCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use rspc::alpha::{MiddlewareArgMapper, Mw};

use crate::{api::Ctx, library::Library};

/// Can wrap a query argument to require it to contain a `library_id` and provide helpers for working with libraries.
#[derive(Clone, Serialize, Deserialize, Type)]
pub struct LibraryArgs<T> {
	pub library_id: Uuid,
	pub arg: T,
}

pub struct LibraryArgsLike;
impl MiddlewareArgMapper for LibraryArgsLike {
	type Input<T> = LibraryArgs<T> where T: Type + DeserializeOwned + 'static;
	type Output<T> = T where T: Serialize;
	type State = Uuid;

	fn map<T: Serialize + DeserializeOwned + Type + 'static>(
		arg: Self::Input<T>,
	) -> (Self::Output<T>, Self::State) {
		(arg.arg, arg.library_id)
	}
}

pub fn library<TPrevMwMapper>() -> impl Mw<Ctx, TPrevMwMapper, NewLayerCtx = (Ctx, Library)>
where
	TPrevMwMapper: MiddlewareArgMapper,
{
	|mw| {
		mw.args::<LibraryArgsLike>()
			.middleware(|mw, library_id| async move {
				let library = mw
					.ctx
					.library_manager
					.get_ctx(library_id)
					.await
					.ok_or_else(|| {
						rspc::Error::new(
							ErrorCode::BadRequest,
							"You must specify a valid library to use this operation.".to_string(),
						)
					})?;

				Ok(mw.map_ctx(|ctx| (ctx, library)))
			})
	}
}
