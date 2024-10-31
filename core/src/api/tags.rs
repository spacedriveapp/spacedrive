use crate::{invalidate_query, library::Library, object::tag::TagCreateArgs};

use sd_prisma::{
	prisma::{device, file_path, object, tag, tag_on_object},
	prisma_sync,
};
use sd_sync::{option_sync_db_entry, sync_db_entry, sync_entry, OperationFactory};

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use itertools::{Either, Itertools};
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(library.db.tag().find_many(vec![]).exec().await?)
			})
		})
		.procedure("getForObject", {
			R.with2(library())
				.query(|(_, library), object_id: i32| async move {
					Ok(library
						.db
						.tag()
						.find_many(vec![tag::tag_objects::some(vec![
							tag_on_object::object_id::equals(object_id),
						])])
						.exec()
						.await?)
				})
		})
		.procedure("getWithObjects", {
			#[derive(Serialize, Type)]
			pub struct ObjectWithDateCreated {
				object: object::Data,
				date_created: DateTime<Utc>,
			}

			R.with2(library()).query(
				|(_, library), object_ids: Vec<object::id::Type>| async move {
					let Library { db, .. } = library.as_ref();

					let tags_with_objects = db
						.tag()
						.find_many(vec![tag::tag_objects::some(vec![
							tag_on_object::object_id::in_vec(object_ids.clone()),
						])])
						.select(tag::select!({
							id
							tag_objects(vec![tag_on_object::object_id::in_vec(object_ids.clone())]): select {
								date_created
								object: select {
									id
								}
							}
						}))
						.exec()
						.await?;

					// This doesn't need normalised caching because it doesn't return whole models.
					Ok(tags_with_objects
						.into_iter()
						.map(|tag| (tag.id, tag.tag_objects))
						.collect::<BTreeMap<_, _>>())
				},
			)
		})
		.procedure("get", {
			R.with2(library())
				.query(|(_, library), tag_id: i32| async move {
					Ok(library
						.db
						.tag()
						.find_unique(tag::id::equals(tag_id))
						.exec()
						.await?)
				})
		})
		.procedure("create", {
			R.with2(library())
				.mutation(|(_, library), args: TagCreateArgs| async move {
					// Check if tag with the same name already exists
					let existing_tag = library
						.db
						.tag()
						.find_many(vec![tag::name::equals(Some(args.name.clone()))])
						.select(tag::select!({ id }))
						.exec()
						.await?;

					if !existing_tag.is_empty() {
						return Err(rspc::Error::new(
							ErrorCode::Conflict,
							"Tag with the same name already exists".to_string(),
						));
					}

					let created_tag = args.exec(&library).await?;

					invalidate_query!(library, "tags.list");

					Ok(created_tag)
				})
		})
		.procedure("assign", {
			#[derive(Debug, Type, Deserialize)]
			#[specta(inline)]
			enum Target {
				Object(object::id::Type),
				FilePath(file_path::id::Type),
			}

			#[derive(Debug, Type, Deserialize)]
			#[specta(inline)]
			struct TagAssignArgs {
				targets: Vec<Target>,
				tag_id: i32,
				unassign: bool,
			}

			R.with2(library())
				.mutation(|(_, library), args: TagAssignArgs| async move {
					let Library { db, sync, .. } = library.as_ref();

					let device_id = library
						.db
						.device()
						.find_unique(device::pub_id::equals(sync.device_pub_id.to_db()))
						.select(device::select!({ id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(
								ErrorCode::NotFound,
								"Local device not found".to_string(),
							)
						})?
						.id;

					let tag = db
						.tag()
						.find_unique(tag::id::equals(args.tag_id))
						.select(tag::select!({ pub_id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(ErrorCode::NotFound, "Tag not found".to_string())
						})?;

					let (objects, file_paths) = db
						._batch({
							let (objects, file_paths): (Vec<_>, Vec<_>) = args
								.targets
								.into_iter()
								.partition_map(|target| match target {
									Target::Object(id) => Either::Left(id),
									Target::FilePath(id) => Either::Right(id),
								});

							(
								db.object()
									.find_many(vec![object::id::in_vec(objects)])
									.select(object::select!({
										id
										pub_id
									})),
								db.file_path()
									.find_many(vec![file_path::id::in_vec(file_paths)])
									.select(file_path::select!({
										id
										pub_id
										is_dir
										object: select { id pub_id }
									})),
							)
						})
						.await?;

					if args.unassign {
						let query = db.tag_on_object().delete_many(vec![
							tag_on_object::tag_id::equals(args.tag_id),
							tag_on_object::object_id::in_vec(
								objects
									.iter()
									.map(|o| o.id)
									.chain(
										file_paths
											.iter()
											.filter_map(|fp| fp.object.as_ref().map(|o| o.id)),
									)
									.collect(),
							),
						]);

						let ops = objects
							.into_iter()
							.map(|o| o.pub_id)
							.chain(
								file_paths
									.into_iter()
									.filter_map(|fp| fp.object.map(|o| o.pub_id)),
							)
							.map(|pub_id| {
								sync.relation_delete(prisma_sync::tag_on_object::SyncId {
									tag: prisma_sync::tag::SyncId {
										pub_id: tag.pub_id.clone(),
									},
									object: prisma_sync::object::SyncId { pub_id },
								})
							})
							.collect::<Vec<_>>();

						if !ops.is_empty() {
							sync.write_ops(db, (ops, query)).await?;
						}
					} else {
						let (sync_ops, db_creates) = objects
							.into_iter()
							.map(|o| (o.id, o.pub_id))
							.chain(
								file_paths
									.into_iter()
									.filter_map(|fp| fp.object.map(|o| (o.id, o.pub_id))),
							)
							.map(|(id, pub_id)| {
								(
									sync.relation_create(
										prisma_sync::tag_on_object::SyncId {
											tag: prisma_sync::tag::SyncId {
												pub_id: tag.pub_id.clone(),
											},
											object: prisma_sync::object::SyncId { pub_id },
										},
										[sync_entry!(
											prisma_sync::device::SyncId {
												pub_id: sync.device_pub_id.to_db(),
											},
											tag_on_object::device
										)],
									),
									tag_on_object::CreateUnchecked {
										tag_id: args.tag_id,
										object_id: id,
										_params: vec![
											tag_on_object::date_created::set(Some(
												Utc::now().into(),
											)),
											tag_on_object::device_id::set(Some(device_id)),
										],
									},
								)
							})
							.unzip::<_, _, Vec<_>, Vec<_>>();

						if !sync_ops.is_empty() && !db_creates.is_empty() {
							sync.write_ops(
								db,
								(
									sync_ops,
									db.tag_on_object().create_many(db_creates).skip_duplicates(),
								),
							)
							.await?;
						}
					}

					invalidate_query!(library, "tags.getForObject");
					invalidate_query!(library, "tags.getWithObjects");
					invalidate_query!(library, "search.objects");

					Ok(())
				})
		})
		.procedure("update", {
			#[derive(Type, Deserialize)]
			pub struct TagUpdateArgs {
				pub id: i32,
				pub name: Option<String>,
				pub color: Option<String>,
			}

			R.with2(library()).mutation(
				|(_, library), TagUpdateArgs { id, name, color }: TagUpdateArgs| async move {
					if name.is_none() && color.is_none() {
						return Ok(());
					}

					let Library { sync, db, .. } = library.as_ref();

					let tag = db
						.tag()
						.find_unique(tag::id::equals(id))
						.select(tag::select!({ pub_id }))
						.exec()
						.await?
						.ok_or(rspc::Error::new(
							ErrorCode::NotFound,
							"Error finding tag in db".into(),
						))?;

					let (sync_params, db_params) = [
						option_sync_db_entry!(name, tag::name),
						option_sync_db_entry!(color, tag::color),
						Some(sync_db_entry!(Utc::now(), tag::date_modified)),
					]
					.into_iter()
					.flatten()
					.unzip::<_, _, Vec<_>, Vec<_>>();

					sync.write_op(
						db,
						sync.shared_update(
							prisma_sync::tag::SyncId {
								pub_id: tag.pub_id.clone(),
							},
							sync_params,
						),
						db.tag()
							.update(tag::id::equals(id), db_params)
							.select(tag::select!({ id })),
					)
					.await?;

					invalidate_query!(library, "tags.list");

					Ok(())
				},
			)
		})
		.procedure(
			"delete",
			R.with2(library())
				.mutation(|(_, library), tag_id: tag::id::Type| async move {
					let Library { sync, db, .. } = &*library;

					let tag_pub_id = db
						.tag()
						.find_unique(tag::id::equals(tag_id))
						.select(tag::select!({ pub_id }))
						.exec()
						.await?
						.ok_or(rspc::Error::new(
							rspc::ErrorCode::NotFound,
							"Tag not found".to_string(),
						))?
						.pub_id;

					let delete_ops = db
						.tag_on_object()
						.find_many(vec![tag_on_object::tag_id::equals(tag_id)])
						.select(tag_on_object::select!({ object: select { pub_id } }))
						.exec()
						.await?
						.into_iter()
						.map(|tag_on_object| {
							sync.relation_delete(prisma_sync::tag_on_object::SyncId {
								tag: prisma_sync::tag::SyncId {
									pub_id: tag_pub_id.clone(),
								},
								object: prisma_sync::object::SyncId {
									pub_id: tag_on_object.object.pub_id,
								},
							})
						})
						.collect::<Vec<_>>();

					sync.write_ops(
						db,
						(
							delete_ops,
							db.tag_on_object()
								.delete_many(vec![tag_on_object::tag_id::equals(tag_id)]),
						),
					)
					.await?;

					sync.write_op(
						db,
						sync.shared_delete(prisma_sync::tag::SyncId { pub_id: tag_pub_id }),
						db.tag().delete(tag::id::equals(tag_id)),
					)
					.await?;

					invalidate_query!(library, "tags.list");

					Ok(())
				}),
		)
}
