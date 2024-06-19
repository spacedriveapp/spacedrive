mod mock_instance;

use sd_core_sync::*;

use sd_prisma::{prisma::location, prisma_sync};
use sd_sync::*;
use sd_utils::{msgpack, uuid_to_bytes};

use mock_instance::Instance;
use tracing::info;
use tracing_test::traced_test;
use uuid::Uuid;

const MOCK_LOCATION_NAME: &str = "Location 0";
const MOCK_LOCATION_PATH: &str = "/User/Anon/Documents";

async fn write_test_location(instance: &Instance) -> location::Data {
	let location_pub_id = Uuid::new_v4();

	let location = instance
		.sync
		.write_ops(&instance.db, {
			let (sync_ops, db_ops): (Vec<_>, Vec<_>) = [
				sync_db_entry!(MOCK_LOCATION_NAME, location::name),
				sync_db_entry!(MOCK_LOCATION_PATH, location::path),
			]
			.into_iter()
			.unzip();

			(
				instance.sync.shared_create(
					prisma_sync::location::SyncId {
						pub_id: uuid_to_bytes(&location_pub_id),
					},
					sync_ops,
				),
				instance
					.db
					.location()
					.create(uuid_to_bytes(&location_pub_id), db_ops),
			)
		})
		.await
		.expect("failed to create mock location");

	instance
		.sync
		.write_ops(&instance.db, {
			let (sync_ops, db_ops): (Vec<_>, Vec<_>) = [
				sync_db_entry!(1024, location::total_capacity),
				sync_db_entry!(512, location::available_capacity),
			]
			.into_iter()
			.unzip();

			(
				sync_ops
					.into_iter()
					.map(|(k, v)| {
						instance.sync.shared_update(
							prisma_sync::location::SyncId {
								pub_id: uuid_to_bytes(&location_pub_id),
							},
							k,
							v,
						)
					})
					.collect::<Vec<_>>(),
				instance
					.db
					.location()
					.update(location::id::equals(location.id), db_ops),
			)
		})
		.await
		.expect("failed to create mock location");

	location
}

#[tokio::test]
#[traced_test]
async fn writes_operations_and_rows_together() -> Result<(), Box<dyn std::error::Error>> {
	let instance = Instance::new(Uuid::new_v4()).await;

	write_test_location(&instance).await;

	let operations = instance
		.db
		.crdt_operation()
		.find_many(vec![])
		.exec()
		.await?;

	// 1 create, 2 update
	assert_eq!(operations.len(), 3);
	assert_eq!(operations[0].model, prisma_sync::location::MODEL_ID as i32);

	let out = instance
		.sync
		.get_ops(GetOpsArgs {
			clocks: vec![],
			count: 100,
		})
		.await?;

	assert_eq!(out.len(), 3);

	let locations = instance.db.location().find_many(vec![]).exec().await?;

	assert_eq!(locations.len(), 1);
	let location = locations.first().unwrap();
	assert_eq!(location.name.as_deref(), Some(MOCK_LOCATION_NAME));
	assert_eq!(location.path.as_deref(), Some(MOCK_LOCATION_PATH));

	Ok(())
}

#[tokio::test]
#[traced_test]
async fn operations_send_and_ingest() -> Result<(), Box<dyn std::error::Error>> {
	let instance1 = Instance::new(Uuid::new_v4()).await;
	let instance2 = Instance::new(Uuid::new_v4()).await;

	let mut instance2_sync_rx = instance2.sync_rx.resubscribe();

	info!("Created instances!");

	Instance::pair(&instance1, &instance2).await;

	info!("Paired instances!");

	write_test_location(&instance1).await;

	info!("Created mock location!");

	assert!(matches!(
		instance2_sync_rx.recv().await?,
		SyncMessage::Ingested
	));

	let out = instance2
		.sync
		.get_ops(GetOpsArgs {
			clocks: vec![],
			count: 100,
		})
		.await?;

	assert_locations_equality(
		&instance1.db.location().find_many(vec![]).exec().await?[0],
		&instance2.db.location().find_many(vec![]).exec().await?[0],
	);

	assert_eq!(out.len(), 3);

	instance1.teardown().await;
	instance2.teardown().await;

	Ok(())
}

#[tokio::test]
async fn no_update_after_delete() -> Result<(), Box<dyn std::error::Error>> {
	let instance1 = Instance::new(Uuid::new_v4()).await;
	let instance2 = Instance::new(Uuid::new_v4()).await;

	let mut instance2_sync_rx = instance2.sync_rx.resubscribe();

	Instance::pair(&instance1, &instance2).await;

	let location = write_test_location(&instance1).await;

	assert!(matches!(
		instance2_sync_rx.recv().await?,
		SyncMessage::Ingested
	));

	instance2
		.sync
		.write_op(
			&instance2.db,
			instance2.sync.shared_delete(prisma_sync::location::SyncId {
				pub_id: location.pub_id.clone(),
			}),
			instance2.db.location().delete_many(vec![]),
		)
		.await?;

	assert!(matches!(
		instance1.sync_rx.resubscribe().recv().await?,
		SyncMessage::Ingested
	));

	instance1
		.sync
		.write_op(
			&instance1.db,
			instance1.sync.shared_update(
				prisma_sync::location::SyncId {
					pub_id: location.pub_id.clone(),
				},
				"name",
				msgpack!("New Location"),
			),
			instance1.db.location().find_many(vec![]),
		)
		.await
		.ok();

	// one spare update operation that actually gets ignored by instance 2
	assert_eq!(instance1.db.crdt_operation().count(vec![]).exec().await?, 5);
	assert_eq!(instance2.db.crdt_operation().count(vec![]).exec().await?, 4);

	assert_eq!(instance1.db.location().count(vec![]).exec().await?, 0);
	// the whole point of the test - the update (which is ingested as an upsert) should be ignored
	assert_eq!(instance2.db.location().count(vec![]).exec().await?, 0);

	instance1.teardown().await;
	instance2.teardown().await;

	Ok(())
}

fn assert_locations_equality(l1: &location::Data, l2: &location::Data) {
	assert_eq!(l1.pub_id, l2.pub_id, "pub id");
	assert_eq!(l1.name, l2.name, "name");
	assert_eq!(l1.path, l2.path, "path");
	assert_eq!(l1.total_capacity, l2.total_capacity, "total capacity");
	assert_eq!(
		l1.available_capacity, l2.available_capacity,
		"available capacity"
	);
	assert_eq!(l1.size_in_bytes, l2.size_in_bytes, "size in bytes");
	assert_eq!(l1.is_archived, l2.is_archived, "is archived");
	assert_eq!(
		l1.generate_preview_media, l2.generate_preview_media,
		"generate preview media"
	);
	assert_eq!(
		l1.sync_preview_media, l2.sync_preview_media,
		"sync preview media"
	);
	assert_eq!(l1.hidden, l2.hidden, "hidden");
	assert_eq!(l1.date_created, l2.date_created, "date created");
	assert_eq!(l1.scan_state, l2.scan_state, "scan state");
	assert_eq!(l1.instance_id, l2.instance_id, "instance id");
}
