use crate::{invalidate_query, library::Library};

use sd_prisma::prisma::{label, label_on_object, object};

use std::collections::BTreeMap;

use rspc::alpha::AlphaRouter;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(library.db.label().find_many(vec![]).exec().await?)
			})
		})
		.procedure("getForObject", {
			R.with2(library())
				.query(|(_, library), object_id: i32| async move {
					Ok(library
						.db
						.label()
						.find_many(vec![label::label_objects::some(vec![
							label_on_object::object_id::equals(object_id),
						])])
						.exec()
						.await?)
				})
		})
		.procedure("getWithObjects", {
			R.with2(library()).query(
				|(_, library), object_ids: Vec<object::id::Type>| async move {
					let Library { db, .. } = library.as_ref();
					let labels_with_objects = db
						.label()
						.find_many(vec![label::label_objects::some(vec![
							label_on_object::object_id::in_vec(object_ids.clone()),
						])])
						.select(label::select!({
							id
							label_objects(vec![label_on_object::object_id::in_vec(object_ids.clone())]): select {
								date_created
								object: select {
									id
								}
							}
						}))
						.exec()
						.await?;
					Ok(labels_with_objects
						.into_iter()
						.map(|label| (label.id, label.label_objects))
						.collect::<BTreeMap<_, _>>())
				},
			)
		})
		.procedure("get", {
			R.with2(library())
				.query(|(_, library), label_id: i32| async move {
					Ok(library
						.db
						.label()
						.find_unique(label::id::equals(label_id))
						.exec()
						.await?)
				})
		})
		.procedure(
			"delete",
			R.with2(library())
				.mutation(|(_, library), label_id: i32| async move {
					library
						.db
						.label()
						.delete(label::id::equals(label_id))
						.exec()
						.await?;

					invalidate_query!(library, "labels.list");

					Ok(())
				}),
		)
}
