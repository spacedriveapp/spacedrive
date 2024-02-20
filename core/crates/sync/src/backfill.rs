use sd_prisma::{
	prisma::{
		file_path, label, label_on_object, location, object, tag, tag_on_object, PrismaClient,
	},
	prisma_sync,
};
use sd_sync::OperationFactory;
use sd_utils::chain_optional_iter;
use serde_json::json;

use crate::crdt_op_unchecked_db;

pub async fn backfill_operations(db: &PrismaClient, sync: &crate::Manager, instance_id: i32) {
	println!("backfill started");
	db.crdt_operation()
		.delete_many(vec![])
		.exec()
		.await
		.unwrap();
	let locations = db.location().find_many(vec![]).exec().await.unwrap();
	db.crdt_operation()
		.create_many(
			locations
				.into_iter()
				.flat_map(|l| {
					sync.shared_create(
						prisma_sync::location::SyncId { pub_id: l.pub_id },
						chain_optional_iter(
							[],
							[
								l.name.map(|v| (location::name::NAME, json!(v))),
								l.path.map(|v| (location::path::NAME, json!(v))),
								l.total_capacity
									.map(|v| (location::total_capacity::NAME, json!(v))),
								l.available_capacity
									.map(|v| (location::available_capacity::NAME, json!(v))),
								l.size_in_bytes
									.map(|v| (location::size_in_bytes::NAME, json!(v))),
								l.is_archived
									.map(|v| (location::is_archived::NAME, json!(v))),
								l.generate_preview_media
									.map(|v| (location::generate_preview_media::NAME, json!(v))),
								l.sync_preview_media
									.map(|v| (location::sync_preview_media::NAME, json!(v))),
								l.hidden.map(|v| (location::hidden::NAME, json!(v))),
								l.date_created
									.map(|v| (location::date_created::NAME, json!(v))),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o, instance_id))
				.collect(),
		)
		.exec()
		.await
		.unwrap();

	let objects = db.object().find_many(vec![]).exec().await.unwrap();
	db.crdt_operation()
		.create_many(
			objects
				.into_iter()
				.flat_map(|o| {
					sync.shared_create(
						prisma_sync::object::SyncId { pub_id: o.pub_id },
						chain_optional_iter(
							[],
							[
								o.kind.map(|v| (object::kind::NAME, json!(v))),
								o.hidden.map(|v| (object::hidden::NAME, json!(v))),
								o.favorite.map(|v| (object::favorite::NAME, json!(v))),
								o.important.map(|v| (object::important::NAME, json!(v))),
								o.note.map(|v| (object::note::NAME, json!(v))),
								o.date_created
									.map(|v| (object::date_created::NAME, json!(v))),
								o.date_accessed
									.map(|v| (object::date_accessed::NAME, json!(v))),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o, instance_id))
				.collect(),
		)
		.exec()
		.await
		.unwrap();

	let file_paths = db
		.file_path()
		.find_many(vec![])
		.include(file_path::include!({
			location: select { pub_id }
			object: select { pub_id }
		}))
		.exec()
		.await
		.unwrap();

	db.crdt_operation()
		.create_many(
			file_paths
				.into_iter()
				.flat_map(|fp| {
					sync.shared_create(
						prisma_sync::file_path::SyncId { pub_id: fp.pub_id },
						chain_optional_iter(
							[],
							[
								fp.is_dir.map(|v| (file_path::is_dir::NAME, json!(v))),
								fp.cas_id.map(|v| (file_path::cas_id::NAME, json!(v))),
								fp.integrity_checksum
									.map(|v| (file_path::integrity_checksum::NAME, json!(v))),
								fp.location.map(|l| {
									(
										file_path::location::NAME,
										json!(prisma_sync::location::SyncId { pub_id: l.pub_id }),
									)
								}),
								fp.materialized_path
									.map(|v| (file_path::materialized_path::NAME, json!(v))),
								fp.name.map(|v| (file_path::name::NAME, json!(v))),
								fp.extension.map(|v| (file_path::extension::NAME, json!(v))),
								fp.hidden.map(|v| (file_path::hidden::NAME, json!(v))),
								fp.size_in_bytes_bytes
									.map(|v| (file_path::size_in_bytes_bytes::NAME, json!(v))),
								fp.inode.map(|v| (file_path::inode::NAME, json!(v))),
								fp.date_created
									.map(|v| (file_path::date_created::NAME, json!(v))),
								fp.date_modified
									.map(|v| (file_path::date_modified::NAME, json!(v))),
								fp.date_indexed
									.map(|v| (file_path::date_indexed::NAME, json!(v))),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o, instance_id))
				.collect(),
		)
		.exec()
		.await
		.unwrap();
	let tags = db.tag().find_many(vec![]).exec().await.unwrap();
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
								t.date_created.map(|v| (tag::date_created::NAME, json!(v))),
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
		.await
		.unwrap();

	let tag_on_objects = db
		.tag_on_object()
		.find_many(vec![])
		.include(tag_on_object::include!({
			tag: select { pub_id }
			object: select { pub_id }
		}))
		.exec()
		.await
		.unwrap();
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
							[t_o.date_created
								.map(|v| (tag_on_object::date_created::NAME, json!(v)))],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o, instance_id))
				.collect(),
		)
		.exec()
		.await
		.unwrap();

	let labels = db.label().find_many(vec![]).exec().await.unwrap();
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
		.await
		.unwrap();

	let label_on_objects = db
		.label_on_object()
		.find_many(vec![])
		.select(label_on_object::select!({
			object: select { pub_id }
			label: select { name }
		}))
		.exec()
		.await
		.unwrap();
	db.crdt_operation()
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
		.await
		.unwrap();
	println!("backfill ended")
}
