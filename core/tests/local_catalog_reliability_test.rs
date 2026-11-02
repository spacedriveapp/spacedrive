//! Regression coverage for settings persistence and explicit filesystem refresh.

mod helpers;

use helpers::IndexingHarnessBuilder;
use sd_core::{
	config::AppConfig,
	infra::{
		action::{CoreAction, LibraryAction},
		job::types::{JobId, JobStatus},
	},
	ops::{
		config::app::update::{UpdateAppConfigAction, UpdateAppConfigInput},
		sources::{
			registry,
			sync::action::{SyncSourceAction, SyncSourceInput},
		},
	},
};
use std::time::Duration;

#[tokio::test]
async fn config_updates_survive_core_shutdown() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("config_survives_shutdown")
		.disable_watcher()
		.build()
		.await?;
	let data_dir = harness.core.context.data_dir.clone();
	// Reproduce a running core whose saved startup value differs from the update.
	{
		let config = harness.core.config();
		let mut config = config.write().await;
		config.services.networking_enabled = true;
		config.save().map_err(|e| anyhow::anyhow!("{e}"))?;
	}
	let output = UpdateAppConfigAction::from_input(UpdateAppConfigInput {
		networking_enabled: Some(false),
		language: Some("fr".to_string()),
		..Default::default()
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.core.context.clone())
	.await?;
	assert!(output.requires_restart);
	assert!(
		!harness
			.core
			.config()
			.read()
			.await
			.services
			.networking_enabled
	);
	harness
		.core
		.shutdown()
		.await
		.map_err(|e| anyhow::anyhow!("{e}"))?;
	let saved = AppConfig::load_from(&data_dir).map_err(|e| anyhow::anyhow!("{e}"))?;
	assert!(!saved.services.networking_enabled);
	assert_eq!(saved.preferences.language, "fr");
	Ok(())
}

#[tokio::test]
async fn filesystem_sync_refreshes_the_existing_source_and_refuses_a_missing_root(
) -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("filesystem_explicit_sync")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("source").await?;
	dir.write_file("before.txt", "before").await?;
	let tracked = dir.track().await?;
	let source_id = tracked.id;
	let context = &harness.core.context;
	let root = context
		.volume_index()
		.source_root(source_id)
		.expect("tracked root");
	let settings = context
		.volume_index()
		.source_config(source_id)
		.expect("settings");
	let source_count = registry::all(harness.library.db().conn()).await?.len();
	tokio::fs::rename(root.join("before.txt"), root.join("renamed.txt")).await?;
	tokio::fs::write(root.join("new.txt"), "new").await?;
	let receipt = SyncSourceAction::from_input(SyncSourceInput {
		source_id: source_id.to_string(),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), context.clone())
	.await?;
	assert_eq!(receipt.job_name, "indexer");
	if let Some(job) = harness.library.jobs().get_job(JobId(receipt.id.0)).await {
		tokio::time::timeout(Duration::from_secs(30), job.wait()).await??;
	}
	let job = harness
		.library
		.jobs()
		.get_job_info(receipt.id.0)
		.await?
		.expect("job receipt");
	assert_eq!(job.status, JobStatus::Completed, "{:?}", job.error_message);
	let store = context
		.volume_index()
		.read_store(source_id)
		.await
		.expect("source store");
	let items = store.list_items(100, 0).await?;
	assert!(items.iter().any(|item| item.title == "new.txt"));
	assert!(items.iter().any(|item| item.title == "renamed.txt"));
	assert!(!items.iter().any(|item| item.title == "before.txt"));
	assert_eq!(
		registry::all(harness.library.db().conn()).await?.len(),
		source_count
	);
	assert_eq!(
		context
			.volume_index()
			.source_config(source_id)
			.expect("settings")
			.unfiltered,
		settings.unfiltered
	);

	let moved = root.with_file_name("source-unavailable");
	tokio::fs::rename(&root, &moved).await?;
	let unavailable = SyncSourceAction::from_input(SyncSourceInput {
		source_id: source_id.to_string(),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), context.clone())
	.await;
	assert!(
		unavailable.is_err(),
		"unavailable source must not start an adapter sync or empty walk"
	);
	assert_eq!(store.list_items(100, 0).await?.len(), items.len());
	tokio::fs::rename(&moved, &root).await?;
	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn returned_volume_catches_up_changes_without_watcher_events() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("returned_source_catchup")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("source").await?;
	dir.write_file("before.txt", "before").await?;
	let tracked = dir.track().await?;
	let context = &harness.core.context;
	let volume_id = tracked.volume_uuid.expect("tracked volume");
	let root = context
		.volume_index()
		.source_root(tracked.id)
		.expect("source root");
	let (mut volume, _) = context
		.volume_manager
		.locate_path(&root)
		.await
		.expect("volume");
	let store = context
		.volume_index()
		.read_store(tracked.id)
		.await
		.expect("source store");

	// Exercise the event follower with a mount transition, without unmounting
	// the host drive that holds this isolated test directory.
	volume.is_mounted = false;
	context
		.events
		.emit(sd_core::infra::event::Event::VolumeAdded(volume.clone()));
	tokio::time::timeout(Duration::from_secs(10), async {
		while context.volume_index().volume_state(volume_id)
			!= Some(sd_core::volume::VolumeState::Unmounted)
		{
			tokio::time::sleep(Duration::from_millis(20)).await;
		}
	})
	.await?;
	tokio::fs::rename(root.join("before.txt"), root.join("while-away.txt")).await?;
	tokio::fs::write(root.join("new-while-away.txt"), "new").await?;
	volume.is_mounted = true;
	context
		.events
		.emit(sd_core::infra::event::Event::VolumeAdded(volume));
	tokio::time::timeout(Duration::from_secs(30), async {
		loop {
			let items = store.list_items(100, 0).await.expect("read catalog");
			if items.iter().any(|item| item.title == "new-while-away.txt")
				&& items.iter().any(|item| item.title == "while-away.txt")
				&& !items.iter().any(|item| item.title == "before.txt")
			{
				break;
			}
			tokio::time::sleep(Duration::from_millis(20)).await;
		}
	})
	.await?;
	harness.shutdown().await?;
	Ok(())
}
