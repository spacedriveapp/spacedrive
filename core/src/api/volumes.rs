use rspc::alpha::AlphaRouter;

use crate::volume::get_volumes;

use super::{t, Ctx};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	t.router()
		.procedure("list", t.query(|_, _: ()| Ok(get_volumes()?)))
}
