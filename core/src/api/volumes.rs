use crate::sys;

use super::{Router, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	<Router>::new().query("get", |ctx, _: ()| sys::Volume::get_volumes().unwrap())
}
