use crate::volume::get_volumes;

use super::{RouterBuilder, R};

pub(crate) fn mount() -> RouterBuilder {
	R.router().procedure("list", {
		R.query(|_, _: ()| async move { Ok(get_volumes().await) })
	})
}
