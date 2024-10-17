use crate::volume::os::get_volumes;

use rspc::alpha::AlphaRouter;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("list", {
		R.query(|_, _: ()| async move { Ok(get_volumes().await) })
	})
}
