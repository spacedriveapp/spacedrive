use super::{utils::LibraryRequest, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.library_query("list", |t| {
			t(
				|_, _: (), library| async move { Ok(library.key_manager.lock().await.dump_keystore()) },
			)
		})
}
