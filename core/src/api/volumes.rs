use crate::volume::get_volumes;

use super::{Router, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	<Router>::new().query("list", |_, _: ()| Ok(get_volumes()?))
}
