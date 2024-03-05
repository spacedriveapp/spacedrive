use std::future::Future;

use sd_prisma::{
	prisma::{
		file_path, label, label_on_object, location, media_data, object, tag, tag_on_object,
		PrismaClient, SortOrder,
	},
	prisma_sync,
};
use sd_sync::{option_sync_entry, OperationFactory};
use sd_utils::chain_optional_iter;
use serde_json::json;

use crate::crdt_op_unchecked_db;

pub async fn backfill_operations(db: &PrismaClient, sync: &crate::Manager, instance_id: i32) {
	db._transaction()
		.run(|db| async move {
			println!("backfill started");
			db.crdt_operation().delete_many(vec![]).exec().await?;

			paginate(
				|cursor| {
					db.location()
						.find_many(vec![location::id::gte(cursor)])
						.order_by(location::id::order(SortOrder::Asc))
						.take(1000)
						.exec()
				},
				|location| location.id,
				|locations| {
					db.crdt_operation()
						.create_many(
							locations
								.into_iter()
								.flat_map(|l| {
									use location::*;

									sync.shared_create(
										prisma_sync::location::SyncId { pub_id: l.pub_id },
										chain_optional_iter(
											[],
											[
												option_sync_entry!(l.name, name),
												option_sync_entry!(l.path, path),
												option_sync_entry!(
													l.total_capacity,
													total_capacity
												),
												option_sync_entry!(
													l.available_capacity,
													available_capacity
												),
												option_sync_entry!(l.size_in_bytes, size_in_bytes),
												option_sync_entry!(l.is_archived, is_archived),
												option_sync_entry!(
													l.generate_preview_media,
													generate_preview_media
												),
												option_sync_entry!(
													l.sync_preview_media,
													sync_preview_media
												),
												option_sync_entry!(l.hidden, hidden),
												option_sync_entry!(l.date_created, date_created),
											],
										),
									)
								})
								.map(|o| crdt_op_unchecked_db(&o, instance_id))
								.collect(),
						)
						.exec()
				},
			)
			.await?;

			paginate(
				|cursor| {
					db.object()
						.find_many(vec![object::id::gte(cursor)])
						.order_by(object::id::order(SortOrder::Asc))
						.take(1000)
						.exec()
				},
				|object| object.id,
				|objects| {
					db.crdt_operation()
						.create_many(
							objects
								.into_iter()
								.flat_map(|o| {
									use object::*;

									sync.shared_create(
										prisma_sync::object::SyncId { pub_id: o.pub_id },
										chain_optional_iter(
											[],
											[
												option_sync_entry!(o.kind, kind),
												option_sync_entry!(o.hidden, hidden),
												option_sync_entry!(o.favorite, favorite),
												option_sync_entry!(o.important, important),
												option_sync_entry!(o.note, note),
												option_sync_entry!(o.date_created, date_created),
												option_sync_entry!(o.date_accessed, date_accessed),
											],
										),
									)
								})
								.map(|o| crdt_op_unchecked_db(&o, instance_id))
								.collect(),
						)
						.exec()
				},
			)
			.await?;

			paginate(
				|cursor| {
					db.media_data()
						.find_many(vec![media_data::id::gte(cursor)])
						.order_by(media_data::id::order(SortOrder::Asc))
						.take(1000)
						.include(media_data::include!({
							object: select { pub_id }
						}))
						.exec()
				},
				|o| o.id,
				|media_datas| {
					db.crdt_operation()
						.create_many(
							media_datas
								.into_iter()
								.flat_map(|md| {
									use media_data::*;

									sync.shared_create(
										prisma_sync::media_data::SyncId {
											object: prisma_sync::object::SyncId {
												pub_id: md.object.pub_id,
											},
										},
										chain_optional_iter(
											[],
											[
												option_sync_entry!(md.resolution, resolution),
												option_sync_entry!(md.media_date, media_date),
												option_sync_entry!(
													md.media_location,
													media_location
												),
												option_sync_entry!(md.camera_data, camera_data),
												option_sync_entry!(md.artist, artist),
												option_sync_entry!(md.description, description),
												option_sync_entry!(md.copyright, copyright),
												option_sync_entry!(md.exif_version, exif_version),
												option_sync_entry!(md.epoch_time, epoch_time),
											],
										),
									)
								})
								.map(|o| crdt_op_unchecked_db(&o, instance_id))
								.collect(),
						)
						.exec()
				},
			)
			.await?;

			paginate(
				|cursor| {
					db.file_path()
						.find_many(vec![file_path::id::gte(cursor)])
						.order_by(file_path::id::order(SortOrder::Asc))
						.include(file_path::include!({
							location: select { pub_id }
							object: select { pub_id }
						}))
						.exec()
				},
				|o| o.id,
				|file_paths| {
					db.crdt_operation()
						.create_many(
							file_paths
								.into_iter()
								.flat_map(|fp| {
									use file_path::*;

									sync.shared_create(
										prisma_sync::file_path::SyncId { pub_id: fp.pub_id },
										chain_optional_iter(
											[],
											[
												option_sync_entry!(fp.is_dir, is_dir),
												option_sync_entry!(fp.cas_id, cas_id),
												option_sync_entry!(
													fp.integrity_checksum,
													integrity_checksum
												),
												option_sync_entry!(
													fp.location.map(|l| {
														prisma_sync::location::SyncId {
															pub_id: l.pub_id,
														}
													}),
													location
												),
												option_sync_entry!(
													fp.materialized_path,
													materialized_path
												),
												option_sync_entry!(fp.name, name),
												option_sync_entry!(fp.extension, extension),
												option_sync_entry!(fp.hidden, hidden),
												option_sync_entry!(
													fp.size_in_bytes_bytes,
													size_in_bytes_bytes
												),
												option_sync_entry!(fp.inode, inode),
												option_sync_entry!(fp.date_created, date_created),
												option_sync_entry!(fp.date_modified, date_modified),
												option_sync_entry!(fp.date_indexed, date_indexed),
											],
										),
									)
								})
								.map(|o| crdt_op_unchecked_db(&o, instance_id))
								.collect(),
						)
						.exec()
				},
			)
			.await?;

			paginate(
				|cursor| {
					db.tag()
						.find_many(vec![tag::id::gte(cursor)])
						.order_by(tag::id::order(SortOrder::Asc))
						.exec()
				},
				|tag| tag.id,
				|tags| {
					db.crdt_operation()
						.create_many(
							tags.into_iter()
								.flat_map(|t| {
									sync.shared_create(
										prisma_sync::tag::SyncId { pub_id: t.pub_id },
										chain_optional_iter(
											[],
											[
												t.name.map(|v| (tag::name::NAME, json!(v))),
												t.color.map(|v| (tag::color::NAME, json!(v))),
												t.date_created
													.map(|v| (tag::date_created::NAME, json!(v))),
												t.date_modified
													.map(|v| (tag::date_modified::NAME, json!(v))),
											],
										),
									)
								})
								.map(|o| crdt_op_unchecked_db(&o, instance_id))
								.collect(),
						)
						.exec()
				},
			)
			.await?;

			let tag_on_objects = db
				.tag_on_object()
				.find_many(vec![])
				.include(tag_on_object::include!({
					tag: select { pub_id }
					object: select { pub_id }
				}))
				.exec()
				.await?;
			db.crdt_operation()
				.create_many(
					tag_on_objects
						.into_iter()
						.flat_map(|t_o| {
							sync.relation_create(
								prisma_sync::tag_on_object::SyncId {
									tag: prisma_sync::tag::SyncId {
										pub_id: t_o.tag.pub_id,
									},
									object: prisma_sync::object::SyncId {
										pub_id: t_o.object.pub_id,
									},
								},
								chain_optional_iter(
									[],
									[option_sync_entry!(
										t_o.date_created,
										tag_on_object::date_created
									)],
								),
							)
						})
						.map(|o| crdt_op_unchecked_db(&o, instance_id))
						.collect(),
				)
				.exec()
				.await?;

			paginate(
				|cursor| {
					db.label()
						.find_many(vec![label::id::gte(cursor)])
						.order_by(label::id::order(SortOrder::Asc))
						.exec()
				},
				|label| label.id,
				|labels| {
					db.crdt_operation()
						.create_many(
							labels
								.into_iter()
								.flat_map(|l| {
									sync.shared_create(
										prisma_sync::label::SyncId { name: l.name },
										[
											(label::date_created::NAME, json!(l.date_created)),
											(label::date_modified::NAME, json!(l.date_modified)),
										],
									)
								})
								.map(|o| crdt_op_unchecked_db(&o, instance_id))
								.collect(),
						)
						.exec()
				},
			)
			.await?;

			let label_on_objects = db
				.label_on_object()
				.find_many(vec![])
				.select(label_on_object::select!({
					object: select { pub_id }
					label: select { name }
				}))
				.exec()
				.await?;
			let res = db
				.crdt_operation()
				.create_many(
					label_on_objects
						.into_iter()
						.flat_map(|l_o| {
							sync.relation_create(
								prisma_sync::label_on_object::SyncId {
									label: prisma_sync::label::SyncId {
										name: l_o.label.name,
									},
									object: prisma_sync::object::SyncId {
										pub_id: l_o.object.pub_id,
									},
								},
								[],
							)
						})
						.map(|o| crdt_op_unchecked_db(&o, instance_id))
						.collect(),
				)
				.exec()
				.await;
			println!("backfill ended");
			res
		})
		.await
		.unwrap();
}

async fn paginate<
	T,
	E: std::fmt::Debug,
	TGetter: Future<Output = Result<Vec<T>, E>>,
	TOperations: Future<Output = Result<i64, E>>,
>(
	getter: impl Fn(i32) -> TGetter,
	id: impl Fn(&T) -> i32,
	operations: impl Fn(Vec<T>) -> TOperations,
) -> Result<(), E> {
	let mut next_cursor = Some(0);
	loop {
		let Some(cursor) = next_cursor else {
			break;
		};

		let items = getter(cursor).await?;
		next_cursor = items.last().map(&id);
		operations(items).await?;
	}

	Ok(())
}
