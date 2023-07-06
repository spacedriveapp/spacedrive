use rspc::alpha::AlphaRouter;

use crate::volume::get_volumes;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("list", {
		R.query(|ctx, _: ()| async move { Ok(get_volumes(&ctx)?) })
	})
}
