use sd_prisma::{
	prisma::{
		crdt_operation, device, exif_data, file_path, label, label_on_object, location, object,
		tag, tag_on_object, volume, PrismaClient, SortOrder,
	},
	prisma_sync,
};
use sd_sync::{option_sync_entry, sync_entry, OperationFactory};
use sd_utils::chain_optional_iter;

use std::future::Future;

use futures_concurrency::future::TryJoin;
use tokio::time::Instant;
use tracing::{debug, instrument};

use super::{crdt_op_unchecked_db, Error, SyncManager};

/// Takes all the syncable data in the database and generates [`CRDTOperations`] for it.
/// This is a requirement before the library can sync.
pub async fn backfill_operations(sync: &SyncManager) -> Result<(), Error> {
	let _lock_guard = sync.sync_lock.lock().await;

	let db = &sync.db;

	let local_device = db
		.device()
		.find_unique(device::pub_id::equals(sync.device_pub_id.to_db()))
		.exec()
		.await?
		.ok_or(Error::DeviceNotFound(sync.device_pub_id.clone()))?;

	let local_device_id = local_device.id;

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

			backfill_device(&db, sync, local_device).await?;

			(
				backfill_volumes(&db, sync, local_device_id),
				paginate_tags(&db, sync),
				paginate_locations(&db, sync, local_device_id),
				paginate_objects(&db, sync, local_device_id),
				paginate_labels(&db, sync),
			)
				.try_join()
				.await?;

			(
				paginate_exif_datas(&db, sync, local_device_id),
				paginate_file_paths(&db, sync, local_device_id),
				paginate_tags_on_objects(&db, sync, local_device_id),
				paginate_labels_on_objects(&db, sync, local_device_id),
			)
				.try_join()
				.await?;

			debug!(elapsed = ?start.elapsed(), "backfill ended");

			Ok(())
		})
		.await
}

#[instrument(skip(db, sync), err)]
async fn backfill_device(
	db: &PrismaClient,
	sync: &SyncManager,
	local_device: device::Data,
) -> Result<(), Error> {
	db.crdt_operation()
		.create_many(vec![crdt_op_unchecked_db(&sync.shared_create(
			prisma_sync::device::SyncId {
				pub_id: local_device.pub_id,
			},
			chain_optional_iter(
				[],
				[
					option_sync_entry!(local_device.name, device::name),
					option_sync_entry!(local_device.os, device::os),
					option_sync_entry!(local_device.hardware_model, device::hardware_model),
					option_sync_entry!(local_device.timestamp, device::timestamp),
					option_sync_entry!(local_device.date_created, device::date_created),
					option_sync_entry!(local_device.date_deleted, device::date_deleted),
				],
			),
		))?])
		.exec()
		.await?;

	Ok(())
}

#[instrument(skip(db, sync), err)]
async fn backfill_volumes(
	db: &PrismaClient,
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<(), Error> {
	let Some(volume) = db
		.volume()
		.find_first(vec![volume::device_id::equals(Some(device_id))])
		.include(volume::include!({device: select { pub_id }}))
		.exec()
		.await?
	else {
		// Nothing to do
		return Ok(());
	};

	db.crdt_operation()
		.create_many(vec![crdt_op_unchecked_db(&sync.shared_create(
			prisma_sync::volume::SyncId {
				pub_id: volume.pub_id,
			},
			chain_optional_iter(
				[
					sync_entry!(volume.name, volume::name),
					sync_entry!(volume.mount_type, volume::mount_type),
					sync_entry!(volume.mount_point, volume::mount_point),
					sync_entry!(volume.is_mounted, volume::is_mounted),
					sync_entry!(volume.disk_type, volume::disk_type),
					sync_entry!(volume.file_system, volume::file_system),
					sync_entry!(volume.read_only, volume::read_only),
					sync_entry!(volume.error_status, volume::error_status),
					sync_entry!(volume.total_bytes_capacity, volume::total_bytes_capacity),
					sync_entry!(volume.total_bytes_available, volume::total_bytes_available),
				],
				[option_sync_entry!(
					volume.device.map(|device| {
						prisma_sync::device::SyncId {
							pub_id: device.pub_id,
						}
					}),
					volume::device
				)],
			),
		))?])
		.exec()
		.await?;

	Ok(())
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
async fn paginate_tags(db: &PrismaClient, sync: &SyncManager) -> Result<(), Error> {
	paginate(
		|cursor| {
			db.tag()
				.find_many(vec![tag::id::gt(cursor)])
				.order_by(tag::id::order(SortOrder::Asc))
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
								option_sync_entry!(t.name, tag::name),
								option_sync_entry!(t.color, tag::color),
								option_sync_entry!(t.date_created, tag::date_created),
								option_sync_entry!(t.date_modified, tag::date_modified),
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
async fn paginate_locations(
	db: &PrismaClient,
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<(), Error> {
	paginate(
		|cursor| {
			db.location()
				.find_many(vec![
					location::id::gt(cursor),
					location::device_id::equals(Some(device_id)),
				])
				.order_by(location::id::order(SortOrder::Asc))
				.take(1000)
				.include(location::include!({
					device: select { pub_id }
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
								option_sync_entry!(l.name, location::name),
								option_sync_entry!(l.path, location::path),
								option_sync_entry!(l.total_capacity, location::total_capacity),
								option_sync_entry!(
									l.available_capacity,
									location::available_capacity
								),
								option_sync_entry!(l.size_in_bytes, location::size_in_bytes),
								option_sync_entry!(l.is_archived, location::is_archived),
								option_sync_entry!(
									l.generate_preview_media,
									location::generate_preview_media
								),
								option_sync_entry!(
									l.sync_preview_media,
									location::sync_preview_media
								),
								option_sync_entry!(l.hidden, location::hidden),
								option_sync_entry!(l.date_created, location::date_created),
								option_sync_entry!(
									l.device.map(|device| {
										prisma_sync::device::SyncId {
											pub_id: device.pub_id,
										}
									}),
									location::device
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
async fn paginate_objects(
	db: &PrismaClient,
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<(), Error> {
	paginate(
		|cursor| {
			db.object()
				.find_many(vec![
					object::id::gt(cursor),
					object::device_id::equals(Some(device_id)),
				])
				.order_by(object::id::order(SortOrder::Asc))
				.take(1000)
				.include(object::include!({
					device: select { pub_id }
				}))
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
								option_sync_entry!(o.kind, object::kind),
								option_sync_entry!(o.hidden, object::hidden),
								option_sync_entry!(o.favorite, object::favorite),
								option_sync_entry!(o.important, object::important),
								option_sync_entry!(o.note, object::note),
								option_sync_entry!(o.date_created, object::date_created),
								option_sync_entry!(o.date_accessed, object::date_accessed),
								option_sync_entry!(
									o.device.map(|device| {
										prisma_sync::device::SyncId {
											pub_id: device.pub_id,
										}
									}),
									object::device
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
async fn paginate_exif_datas(
	db: &PrismaClient,
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<(), Error> {
	paginate(
		|cursor| {
			db.exif_data()
				.find_many(vec![
					exif_data::id::gt(cursor),
					exif_data::device_id::equals(Some(device_id)),
				])
				.order_by(exif_data::id::order(SortOrder::Asc))
				.take(1000)
				.include(exif_data::include!({
					object: select { pub_id }
					device: select { pub_id }
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
								option_sync_entry!(ed.resolution, exif_data::resolution),
								option_sync_entry!(ed.media_date, exif_data::media_date),
								option_sync_entry!(ed.media_location, exif_data::media_location),
								option_sync_entry!(ed.camera_data, exif_data::camera_data),
								option_sync_entry!(ed.artist, exif_data::artist),
								option_sync_entry!(ed.description, exif_data::description),
								option_sync_entry!(ed.copyright, exif_data::copyright),
								option_sync_entry!(ed.exif_version, exif_data::exif_version),
								option_sync_entry!(ed.epoch_time, exif_data::epoch_time),
								option_sync_entry!(
									ed.device.map(|device| {
										prisma_sync::device::SyncId {
											pub_id: device.pub_id,
										}
									}),
									exif_data::device
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
async fn paginate_file_paths(
	db: &PrismaClient,
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<(), Error> {
	paginate(
		|cursor| {
			db.file_path()
				.find_many(vec![
					file_path::id::gt(cursor),
					file_path::device_id::equals(Some(device_id)),
				])
				.order_by(file_path::id::order(SortOrder::Asc))
				.include(file_path::include!({
					location: select { pub_id }
					object: select { pub_id }
					device: select { pub_id }
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
								option_sync_entry!(fp.is_dir, file_path::is_dir),
								option_sync_entry!(fp.cas_id, file_path::cas_id),
								option_sync_entry!(
									fp.integrity_checksum,
									file_path::integrity_checksum
								),
								option_sync_entry!(
									fp.location.map(|l| {
										prisma_sync::location::SyncId { pub_id: l.pub_id }
									}),
									file_path::location
								),
								option_sync_entry!(
									fp.object.map(|o| {
										prisma_sync::object::SyncId { pub_id: o.pub_id }
									}),
									file_path::object
								),
								option_sync_entry!(
									fp.materialized_path,
									file_path::materialized_path
								),
								option_sync_entry!(fp.name, file_path::name),
								option_sync_entry!(fp.extension, file_path::extension),
								option_sync_entry!(fp.hidden, file_path::hidden),
								option_sync_entry!(
									fp.size_in_bytes_bytes,
									file_path::size_in_bytes_bytes
								),
								option_sync_entry!(fp.inode, file_path::inode),
								option_sync_entry!(fp.date_created, file_path::date_created),
								option_sync_entry!(fp.date_modified, file_path::date_modified),
								option_sync_entry!(fp.date_indexed, file_path::date_indexed),
								option_sync_entry!(
									fp.device.map(|device| {
										prisma_sync::device::SyncId {
											pub_id: device.pub_id,
										}
									}),
									file_path::device
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
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<(), Error> {
	paginate_relation(
		|group_id, item_id| {
			db.tag_on_object()
				.find_many(vec![
					tag_on_object::tag_id::gt(group_id),
					tag_on_object::object_id::gt(item_id),
					tag_on_object::device_id::equals(Some(device_id)),
				])
				.order_by(tag_on_object::tag_id::order(SortOrder::Asc))
				.order_by(tag_on_object::object_id::order(SortOrder::Asc))
				.include(tag_on_object::include!({
					tag: select { pub_id }
					object: select { pub_id }
					device: select { pub_id }
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
								option_sync_entry!(t_o.date_created, tag_on_object::date_created),
								option_sync_entry!(
									t_o.device.map(|device| {
										prisma_sync::device::SyncId {
											pub_id: device.pub_id,
										}
									}),
									tag_on_object::device
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
async fn paginate_labels(db: &PrismaClient, sync: &SyncManager) -> Result<(), Error> {
	paginate(
		|cursor| {
			db.label()
				.find_many(vec![label::id::gt(cursor)])
				.order_by(label::id::order(SortOrder::Asc))
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
								option_sync_entry!(l.date_created, label::date_created),
								option_sync_entry!(l.date_modified, label::date_modified),
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
	sync: &SyncManager,
	device_id: device::id::Type,
) -> Result<(), Error> {
	paginate_relation(
		|group_id, item_id| {
			db.label_on_object()
				.find_many(vec![
					label_on_object::label_id::gt(group_id),
					label_on_object::object_id::gt(item_id),
					label_on_object::device_id::equals(Some(device_id)),
				])
				.order_by(label_on_object::label_id::order(SortOrder::Asc))
				.order_by(label_on_object::object_id::order(SortOrder::Asc))
				.include(label_on_object::include!({
					object: select { pub_id }
					label: select { name }
					device: select { pub_id }
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
							[sync_entry!(l_o.date_created, label_on_object::date_created)],
							[option_sync_entry!(
								l_o.device.map(|device| {
									prisma_sync::device::SyncId {
										pub_id: device.pub_id,
									}
								}),
								label_on_object::device
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
