use super::{utils::LibraryRequest, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.library_query("list", |t| {
			t(
				|_, _: (), library| async move { Ok(library.db.key().find_many(vec![]).exec().await?) },
			)
		})
		.library_query("listMounted", |t| {
			t(
				|_, _: (), library| async move { Ok(library.key_manager.lock().await.get_mounted_uuids()) },
			)
		})
		.library_query("mount", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library.key_manager.lock().await.mount(key_uuid).unwrap();
				// we also need to dispatch jobs that automatically decrypt preview media and metadata here

				Ok(())
			})
		})
}
