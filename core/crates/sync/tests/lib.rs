mod mock_instance;

use sd_core_sync::*;

use sd_prisma::{prisma, prisma_sync};
use sd_sync::*;
use sd_utils::{msgpack, uuid_to_bytes};

use mock_instance::Instance;
use uuid::Uuid;

async fn write_test_location(
	instance: &Instance,
) -> Result<prisma::location::Data, Box<dyn std::error::Error>> {
	Ok(instance
		.sync
		.write_ops(&instance.db, {
			let id = Uuid::new_v4();

			let (sync_ops, db_ops): (Vec<_>, Vec<_>) = [
				sync_db_entry!("Location 0".to_string(), prisma::location::name),
				sync_db_entry!(
					"/User/Brendan/Documents".to_string(),
					prisma::location::path
				),
			]
			.into_iter()
			.unzip();

			(
				instance.sync.shared_create(
					prisma_sync::location::SyncId {
						pub_id: uuid_to_bytes(&id),
					},
					sync_ops,
				),
				instance.db.location().create(uuid_to_bytes(&id), db_ops),
			)
		})
		.await?)
}

#[tokio::test]
async fn writes_operations_and_rows_together() -> Result<(), Box<dyn std::error::Error>> {
	let instance = Instance::new(Uuid::new_v4()).await;

	write_test_location(&instance).await?;

	let operations = instance
		.db
		.crdt_operation()
		.find_many(vec![])
		.exec()
		.await?;

	// 1 create, 2 update
	assert_eq!(operations.len(), 3);
	assert_eq!(operations[0].model, prisma_sync::location::MODEL_ID as i32);

	let locations = instance.db.location().find_many(vec![]).exec().await?;

	assert_eq!(locations.len(), 1);
	let location = locations.first().unwrap();
	assert_eq!(location.name, Some("Location 0".to_string()));
	assert_eq!(location.path, Some("/User/Brendan/Documents".to_string()));

	Ok(())
}

#[tokio::test]
async fn operations_send_and_ingest() -> Result<(), Box<dyn std::error::Error>> {
	let instance1 = Instance::new(Uuid::new_v4()).await;
	let instance2 = Instance::new(Uuid::new_v4()).await;

	Instance::pair(&instance1, &instance2).await;

	write_test_location(&instance1).await?;

	assert!(matches!(
		instance2.sync_rx.resubscribe().recv().await?,
		SyncMessage::Ingested
	));

	let out = instance2
		.sync
		.get_ops(GetOpsArgs {
			clocks: vec![],
			count: 100,
		})
		.await?;

	assert_eq!(out.len(), 3);

	instance1.teardown().await;
	instance2.teardown().await;

	Ok(())
}

#[tokio::test]
async fn no_update_after_delete() -> Result<(), Box<dyn std::error::Error>> {
	let instance1 = Instance::new(Uuid::new_v4()).await;
	let instance2 = Instance::new(Uuid::new_v4()).await;

	Instance::pair(&instance1, &instance2).await;

	let location = write_test_location(&instance1).await?;

	assert!(matches!(
		instance2.sync_rx.resubscribe().recv().await?,
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
