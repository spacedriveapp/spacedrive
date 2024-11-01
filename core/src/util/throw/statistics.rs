// use crate::{
// 	library::Library,
// 	volume::{Volume, VolumeError},
// 	Node,
// };
// use futures_concurrency::future::Join;
// use sd_core_sync::SyncManager;
// use sd_prisma::{
// 	prisma::{device, PrismaClient},
// 	prisma_sync,
// };
// use sd_sync::*;
// use sd_utils::uuid_to_bytes;
// use std::sync::Arc;
// use tracing::error;
// use uuid::Uuid;

// use super::os::get_volumes;

// async fn update_storage_statistics(
// 	db: &PrismaClient,
// 	sync: &SyncManager,
// 	total_capacity: u64,
// 	available_capacity: u64,
// ) -> Result<(), VolumeError> {
// 	let device_pub_id = sync.device_pub_id.to_db();

// 	let storage_statistics_pub_id = db
// 		.storage_statistics()
// 		.find_first(vec![storage_statistics::device::is(vec![
// 			device::pub_id::equals(device_pub_id.clone()),
// 		])])
// 		.select(storage_statistics::select!({ pub_id }))
// 		.exec()
// 		.await?
// 		.map(|s| s.pub_id);

// 	if let Some(storage_statistics_pub_id) = storage_statistics_pub_id {
// 		sync.write_ops(
// 			db,
// 			(
// 				[
// 					sync_entry!(total_capacity, storage_statistics::total_capacity),
// 					sync_entry!(available_capacity, storage_statistics::available_capacity),
// 				]
// 				.into_iter()
// 				.map(|(field, value)| {
// 					sync.shared_update(
// 						prisma_sync::storage_statistics::SyncId {
// 							pub_id: storage_statistics_pub_id.clone(),
// 						},
// 						field,
// 						value,
// 					)
// 				})
// 				.collect(),
// 				db.storage_statistics()
// 					.update(
// 						storage_statistics::pub_id::equals(storage_statistics_pub_id),
// 						vec![
// 							storage_statistics::total_capacity::set(total_capacity as i64),
// 							storage_statistics::available_capacity::set(available_capacity as i64),
// 						],
// 					)
// 					// We don't need any data here, just the id avoids receiving the entire object
// 					// as we can't pass an empty select macro call
// 					.select(storage_statistics::select!({ id })),
// 			),
// 		)
// 		.await?;
// 	} else {
// 		let new_storage_statistics_id = uuid_to_bytes(&Uuid::now_v7());

// 		sync.write_op(
// 			db,
// 			sync.shared_create(
// 				prisma_sync::storage_statistics::SyncId {
// 					pub_id: new_storage_statistics_id.clone(),
// 				},
// 				[
// 					sync_entry!(total_capacity, storage_statistics::total_capacity),
// 					sync_entry!(available_capacity, storage_statistics::available_capacity),
// 					sync_entry!(
// 						prisma_sync::device::SyncId {
// 							pub_id: device_pub_id.clone()
// 						},
// 						storage_statistics::device
// 					),
// 				],
// 			),
// 			db.storage_statistics()
// 				.create(
// 					new_storage_statistics_id,
// 					vec![
// 						storage_statistics::total_capacity::set(total_capacity as i64),
// 						storage_statistics::available_capacity::set(available_capacity as i64),
// 						storage_statistics::device::connect(device::pub_id::equals(device_pub_id)),
// 					],
// 				)
// 				// We don't need any data here, just the id avoids receiving the entire object
// 				// as we can't pass an empty select macro call
// 				.select(storage_statistics::select!({ id })),
// 		)
// 		.await?;
// 	}

// 	Ok(())
// }

// pub fn save_storage_statistics(node: &Node) {
// 	// tokio::spawn({
// 	// 	let libraries = Arc::clone(&node.libraries);
// 	// 	async move {
// 	// 		let (total_capacity, available_capacity) = compute_stats(&get_volumes().await);

// 	// 		libraries
// 	// 			.get_all()
// 	// 			.await
// 	// 			.into_iter()
// 	// 			.map(move |library: Arc<Library>| async move {
// 	// 				let Library { db, sync, .. } = &*library;

// 	// 				update_storage_statistics(db, sync, total_capacity, available_capacity).await
// 	// 			})
// 	// 			.collect::<Vec<_>>()
// 	// 			.join()
// 	// 			.await
// 	// 			.into_iter()
// 	// 			.for_each(|res| {
// 	// 				if let Err(e) = res {
// 	// 					error!(?e, "Failed to save storage statistics;");
// 	// 				}
// 	// 			});
// 	// 	}
// 	// });
// }

// fn compute_stats<'v>(volumes: impl IntoIterator<Item = &'v Volume>) -> (u64, u64) {
// 	volumes
// 		.into_iter()
// 		.fold((0, 0), |(mut total, mut available), volume| {
// 			total += volume.total_bytes_capacity;
// 			available += volume.total_bytes_available;

// 			(total, available)
// 		})
// }
