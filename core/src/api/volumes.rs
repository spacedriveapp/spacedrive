use crate::volume::get_volumes;

use super::RouterBuilder;

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new().query("list", |t| t(|_, _: ()| Ok(get_volumes()?)))
}
