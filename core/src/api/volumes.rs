use crate::volume::get_volumes;

use super::{Router, R};

pub(crate) fn mount() -> Router {
	R.router().procedure("list", {
		R.query(|_, _: ()| async move { Ok(get_volumes().await) })
	})
}
