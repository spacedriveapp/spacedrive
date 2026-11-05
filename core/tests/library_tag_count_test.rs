//! The library's tag count follows tags into the source stores that apply
//! them.

use sd_core::{
	infra::action::LibraryAction,
	ops::{
		sources::track::{TrackSourceAction, TrackSourceInput},
		tags::{
			apply::{action::ApplyTagsAction, input::ApplyTagsInput, input::TagTargets},
			create::{action::CreateTagAction, input::CreateTagInput},
		},
	},
	Core,
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, Instant};
use uuid::Uuid;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Applying a tag moves its definition from the library's staging table into
/// the store that carries it. The recount behind `libraries.info` read only
/// staging, so a tag in use counted as none, before and after a restart.
#[tokio::test]
async fn applied_tags_count_in_library_statistics() -> Result<(), Error> {
	let temp = TempDir::new()?;
	let core = Core::new(temp.path().join("core")).await?;
	let library = core
		.libraries
		.create_library("Tags", None, core.context.clone())
		.await?;

	let root = temp.path().join("source");
	tokio::fs::create_dir_all(&root).await?;
	for file in 0..3 {
		tokio::fs::write(root.join(format!("file_{file}.txt")), format!("{file}")).await?;
	}
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: root,
		name: None,
		overrides: Default::default(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let store = core
		.context
		.volume_index()
		.store_for(&tracked.root)
		.await
		.ok_or("the tracked source has no store")?;

	let deadline = Instant::now() + Duration::from_secs(30);
	let targets: Vec<Uuid> = loop {
		let items = store.db().list_items(1, 0).await?;
		if !items.is_empty() {
			break items.into_iter().map(|row| row.id).collect();
		}
		assert!(Instant::now() < deadline, "the walk recorded nothing");
		sleep(Duration::from_millis(50)).await;
	};

	let tag = CreateTagAction::from_input(CreateTagInput {
		path: "Projects/Keep".to_string(),
		color: None,
		icon: None,
	})?
	.execute(library.clone(), core.context.clone())
	.await?
	.tag;
	let applied = ApplyTagsAction::from_input(ApplyTagsInput {
		targets: TagTargets::File(targets),
		tag_ids: vec![tag.id],
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	assert_eq!(applied.targets_tagged, 1);
	store.flush().await?;

	let deadline = Instant::now() + Duration::from_secs(30);
	loop {
		library.recalculate_statistics().await?;
		sleep(Duration::from_millis(100)).await;
		let tag_count = library.get_statistics().await.tag_count;
		if tag_count == 1 {
			break;
		}
		assert!(
			Instant::now() < deadline,
			"library statistics report {tag_count} tags with one applied"
		);
	}

	core.shutdown().await?;
	Ok(())
}
