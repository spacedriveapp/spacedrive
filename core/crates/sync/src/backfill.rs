use sd_prisma::{
	prisma::{
		crdt_operation, exif_data, file_path, label, label_on_object, location, object, tag,
		tag_on_object, PrismaClient, SortOrder,
	},
	prisma_sync,
};
use sd_sync::{option_sync_entry, sync_entry, OperationFactory};
use sd_utils::chain_optional_iter;

use std::future::Future;

use futures_concurrency::future::TryJoin;
use tokio::time::Instant;
use tracing::{debug, instrument};

use super::{crdt_op_unchecked_db, Error};

/// Takes all the syncable data in the database and generates [`CRDTOperations`] for it.
/// This is a requirement before the library can sync.
pub async fn backfill_operations(
	db: &PrismaClient,
	sync: &crate::SyncManager,
) -> Result<(), Error> {
	let _lock_guard = sync.sync_lock.lock().await;

	db._transaction()
		.with_timeout(9_999_999_999)
		.run(|db| async move {
			debug!("backfill started");
			let start = Instant::now();
			db.crdt_operation()
				.delete_many(vec![crdt_operation::device_pub_id::equals(
					sync.device_pub_id.to_db(),
				)])
				.exec()
				.await?;

			(
				paginate_tags(&db, sync),
				paginate_locations(&db, sync),
				paginate_objects(&db, sync),
				paginate_exif_datas(&db, sync),
				paginate_file_paths(&db, sync),
				paginate_labels(&db, sync),
			)
				.try_join()
				.await?;

			(
				paginate_tags_on_objects(&db, sync),
				paginate_labels_on_objects(&db, sync),
			)
				.try_join()
				.await?;

			debug!(elapsed = ?start.elapsed(), "backfill ended");

			Ok(())
		})
		.await
}

async fn paginate<T, E1, E2, E3, GetterFut, OperationsFut>(
	getter: impl Fn(i32) -> GetterFut + Send,
	id: impl Fn(&T) -> i32 + Send,
	operations: impl Fn(Vec<T>) -> Result<OperationsFut, E3> + Send,
) -> Result<(), Error>
where
	T: Send,
	E1: Send,
	E2: Send,
	E3: Send,
	Error: From<E1> + From<E2> + From<E3> + Send,
	GetterFut: Future<Output = Result<Vec<T>, E1>> + Send,
	OperationsFut: Future<Output = Result<i64, E2>> + Send,
{
	let mut next_cursor = Some(-1);
	loop {
		let Some(cursor) = next_cursor else {
			break;
		};

		let items = getter(cursor).await?;
		next_cursor = items.last().map(&id);
		operations(items)?.await?;
	}

	Ok(())
}

async fn paginate_relation<T, E1, E2, E3, GetterFut, OperationsFut>(
	getter: impl Fn(i32, i32) -> GetterFut + Send,
	id: impl Fn(&T) -> (i32, i32) + Send,
	operations: impl Fn(Vec<T>) -> Result<OperationsFut, E3> + Send,
) -> Result<(), Error>
where
	T: Send,
	E1: Send,
	E2: Send,
	E3: Send,
	Error: From<E1> + From<E2> + From<E3> + Send,
	GetterFut: Future<Output = Result<Vec<T>, E1>> + Send,
	OperationsFut: Future<Output = Result<i64, E2>> + Send,
{
	let mut next_cursor = Some((-1, -1));
	loop {
		let Some(cursor) = next_cursor else {
			break;
		};

		let items = getter(cursor.0, cursor.1).await?;
		next_cursor = items.last().map(&id);
		operations(items)?.await?;
	}

	Ok(())
}

#[instrument(skip(db, sync), err)]
async fn paginate_tags(db: &PrismaClient, sync: &crate::SyncManager) -> Result<(), Error> {
	use tag::{color, date_created, date_modified, id, name};

	paginate(
		|cursor| {
			db.tag()
				.find_many(vec![id::gt(cursor)])
				.order_by(id::order(SortOrder::Asc))
				.exec()
		},
		|tag| tag.id,
		|tags| {
			tags.into_iter()
				.map(|t| {
					sync.shared_create(
						prisma_sync::tag::SyncId { pub_id: t.pub_id },
						chain_optional_iter(
							[],
							[
								option_sync_entry!(t.name, name),
								option_sync_entry!(t.color, color),
								option_sync_entry!(t.date_created, date_created),
								option_sync_entry!(t.date_modified, date_modified),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}

#[instrument(skip(db, sync), err)]
async fn paginate_locations(db: &PrismaClient, sync: &crate::SyncManager) -> Result<(), Error> {
	use location::{
		available_capacity, date_created, device, device_pub_id, generate_preview_media, hidden,
		id, include, instance, is_archived, name, path, size_in_bytes, sync_preview_media,
		total_capacity,
	};

	let device_pub_id = &sync.device_pub_id;

	paginate(
		|cursor| {
			db.location()
				.find_many(vec![
					id::gt(cursor),
					device_pub_id::equals(Some(device_pub_id.to_db())),
				])
				.order_by(id::order(SortOrder::Asc))
				.take(1000)
				.include(include!({
					instance: select {
						id
						pub_id
					}
				}))
				.exec()
		},
		|location| location.id,
		|locations| {
			locations
				.into_iter()
				.map(|l| {
					sync.shared_create(
						prisma_sync::location::SyncId { pub_id: l.pub_id },
						chain_optional_iter(
							[],
							[
								option_sync_entry!(l.name, name),
								option_sync_entry!(l.path, path),
								option_sync_entry!(l.total_capacity, total_capacity),
								option_sync_entry!(l.available_capacity, available_capacity),
								option_sync_entry!(l.size_in_bytes, size_in_bytes),
								option_sync_entry!(l.is_archived, is_archived),
								option_sync_entry!(
									l.generate_preview_media,
									generate_preview_media
								),
								option_sync_entry!(l.sync_preview_media, sync_preview_media),
								option_sync_entry!(l.hidden, hidden),
								option_sync_entry!(l.date_created, date_created),
								option_sync_entry!(
									l.instance.map(|i| {
										prisma_sync::instance::SyncId { pub_id: i.pub_id }
									}),
									instance
								),
								option_sync_entry!(
									l.device_pub_id.map(|device_pub_id| {
										prisma_sync::device::SyncId {
											pub_id: device_pub_id,
										}
									}),
									device
								),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}

#[instrument(skip(db, sync), err)]
async fn paginate_objects(db: &PrismaClient, sync: &crate::SyncManager) -> Result<(), Error> {
	use object::{
		date_accessed, date_created, device, device_pub_id, favorite, hidden, id, important, kind,
		note,
	};

	let device_pub_id = &sync.device_pub_id;

	paginate(
		|cursor| {
			db.object()
				.find_many(vec![
					id::gt(cursor),
					device_pub_id::equals(Some(device_pub_id.to_db())),
				])
				.order_by(id::order(SortOrder::Asc))
				.take(1000)
				.exec()
		},
		|object| object.id,
		|objects| {
			objects
				.into_iter()
				.map(|o| {
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
								option_sync_entry!(
									o.device_pub_id.map(|device_pub_id| {
										prisma_sync::device::SyncId {
											pub_id: device_pub_id,
										}
									}),
									device
								),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}

#[instrument(skip(db, sync), err)]
async fn paginate_exif_datas(db: &PrismaClient, sync: &crate::SyncManager) -> Result<(), Error> {
	use exif_data::{
		artist, camera_data, copyright, description, device, device_pub_id, epoch_time,
		exif_version, id, include, media_date, media_location, resolution,
	};

	let device_pub_id = &sync.device_pub_id;

	paginate(
		|cursor| {
			db.exif_data()
				.find_many(vec![
					id::gt(cursor),
					device_pub_id::equals(Some(device_pub_id.to_db())),
				])
				.order_by(id::order(SortOrder::Asc))
				.take(1000)
				.include(include!({
					object: select { pub_id }
				}))
				.exec()
		},
		|ed| ed.id,
		|exif_datas| {
			exif_datas
				.into_iter()
				.map(|ed| {
					sync.shared_create(
						prisma_sync::exif_data::SyncId {
							object: prisma_sync::object::SyncId {
								pub_id: ed.object.pub_id,
							},
						},
						chain_optional_iter(
							[],
							[
								option_sync_entry!(ed.resolution, resolution),
								option_sync_entry!(ed.media_date, media_date),
								option_sync_entry!(ed.media_location, media_location),
								option_sync_entry!(ed.camera_data, camera_data),
								option_sync_entry!(ed.artist, artist),
								option_sync_entry!(ed.description, description),
								option_sync_entry!(ed.copyright, copyright),
								option_sync_entry!(ed.exif_version, exif_version),
								option_sync_entry!(ed.epoch_time, epoch_time),
								option_sync_entry!(
									ed.device_pub_id.map(|device_pub_id| {
										prisma_sync::device::SyncId {
											pub_id: device_pub_id,
										}
									}),
									device
								),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}

#[instrument(skip(db, sync), err)]
async fn paginate_file_paths(db: &PrismaClient, sync: &crate::SyncManager) -> Result<(), Error> {
	use file_path::{
		cas_id, date_created, date_indexed, date_modified, device, device_pub_id, extension,
		hidden, id, include, inode, integrity_checksum, is_dir, location, materialized_path, name,
		object, size_in_bytes_bytes,
	};

	let device_pub_id = &sync.device_pub_id;

	paginate(
		|cursor| {
			db.file_path()
				.find_many(vec![
					id::gt(cursor),
					device_pub_id::equals(Some(device_pub_id.to_db())),
				])
				.order_by(id::order(SortOrder::Asc))
				.include(include!({
					location: select { pub_id }
					object: select { pub_id }
				}))
				.exec()
		},
		|o| o.id,
		|file_paths| {
			file_paths
				.into_iter()
				.map(|fp| {
					sync.shared_create(
						prisma_sync::file_path::SyncId { pub_id: fp.pub_id },
						chain_optional_iter(
							[],
							[
								option_sync_entry!(fp.is_dir, is_dir),
								option_sync_entry!(fp.cas_id, cas_id),
								option_sync_entry!(fp.integrity_checksum, integrity_checksum),
								option_sync_entry!(
									fp.location.map(|l| {
										prisma_sync::location::SyncId { pub_id: l.pub_id }
									}),
									location
								),
								option_sync_entry!(
									fp.object.map(|o| {
										prisma_sync::object::SyncId { pub_id: o.pub_id }
									}),
									object
								),
								option_sync_entry!(fp.materialized_path, materialized_path),
								option_sync_entry!(fp.name, name),
								option_sync_entry!(fp.extension, extension),
								option_sync_entry!(fp.hidden, hidden),
								option_sync_entry!(fp.size_in_bytes_bytes, size_in_bytes_bytes),
								option_sync_entry!(fp.inode, inode),
								option_sync_entry!(fp.date_created, date_created),
								option_sync_entry!(fp.date_modified, date_modified),
								option_sync_entry!(fp.date_indexed, date_indexed),
								option_sync_entry!(
									fp.device_pub_id.map(|device_pub_id| {
										prisma_sync::device::SyncId {
											pub_id: device_pub_id,
										}
									}),
									device
								),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}

#[instrument(skip(db, sync), err)]
async fn paginate_tags_on_objects(
	db: &PrismaClient,
	sync: &crate::SyncManager,
) -> Result<(), Error> {
	use tag_on_object::{date_created, device, device_pub_id, include, object_id, tag_id};

	let device_pub_id = &sync.device_pub_id;

	paginate_relation(
		|group_id, item_id| {
			db.tag_on_object()
				.find_many(vec![
					tag_id::gt(group_id),
					object_id::gt(item_id),
					device_pub_id::equals(Some(device_pub_id.to_db())),
				])
				.order_by(tag_id::order(SortOrder::Asc))
				.order_by(object_id::order(SortOrder::Asc))
				.include(include!({
					tag: select { pub_id }
					object: select { pub_id }
				}))
				.exec()
		},
		|t_o| (t_o.tag_id, t_o.object_id),
		|tag_on_objects| {
			tag_on_objects
				.into_iter()
				.map(|t_o| {
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
							[
								option_sync_entry!(t_o.date_created, date_created),
								option_sync_entry!(
									t_o.device_pub_id.map(|device_pub_id| {
										prisma_sync::device::SyncId {
											pub_id: device_pub_id,
										}
									}),
									device
								),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}

#[instrument(skip(db, sync), err)]
async fn paginate_labels(db: &PrismaClient, sync: &crate::SyncManager) -> Result<(), Error> {
	use label::{date_created, date_modified, id};

	paginate(
		|cursor| {
			db.label()
				.find_many(vec![id::gt(cursor)])
				.order_by(id::order(SortOrder::Asc))
				.exec()
		},
		|label| label.id,
		|labels| {
			labels
				.into_iter()
				.map(|l| {
					sync.shared_create(
						prisma_sync::label::SyncId { name: l.name },
						chain_optional_iter(
							[],
							[
								option_sync_entry!(l.date_created, date_created),
								option_sync_entry!(l.date_modified, date_modified),
							],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}

#[instrument(skip(db, sync), err)]
async fn paginate_labels_on_objects(
	db: &PrismaClient,
	sync: &crate::SyncManager,
) -> Result<(), Error> {
	use label_on_object::{date_created, device, device_pub_id, include, label_id, object_id};

	let device_pub_id = &sync.device_pub_id;

	paginate_relation(
		|group_id, item_id| {
			db.label_on_object()
				.find_many(vec![
					label_id::gt(group_id),
					object_id::gt(item_id),
					device_pub_id::equals(Some(device_pub_id.to_db())),
				])
				.order_by(label_id::order(SortOrder::Asc))
				.order_by(object_id::order(SortOrder::Asc))
				.include(include!({
					object: select { pub_id }
					label: select { name }
				}))
				.exec()
		},
		|l_o| (l_o.label_id, l_o.object_id),
		|label_on_objects| {
			label_on_objects
				.into_iter()
				.map(|l_o| {
					sync.relation_create(
						prisma_sync::label_on_object::SyncId {
							label: prisma_sync::label::SyncId {
								name: l_o.label.name,
							},
							object: prisma_sync::object::SyncId {
								pub_id: l_o.object.pub_id,
							},
						},
						chain_optional_iter(
							[sync_entry!(l_o.date_created, date_created)],
							[option_sync_entry!(
								l_o.device_pub_id.map(|device_pub_id| {
									prisma_sync::device::SyncId {
										pub_id: device_pub_id,
									}
								}),
								device
							)],
						),
					)
				})
				.map(|o| crdt_op_unchecked_db(&o))
				.collect::<Result<Vec<_>, _>>()
				.map(|creates| db.crdt_operation().create_many(creates).exec())
		},
	)
	.await
}
