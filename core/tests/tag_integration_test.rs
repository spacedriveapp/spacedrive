//! Comprehensive integration test for tag system
//!
//! This test validates the full tag workflow including:
//! - Tag creation with resource events
//! - File tagging with resource events
//! - Database persistence verification
//! - Multi-file and multi-tag operations

mod helpers;

use helpers::*;
use sd_core::{
	infra::{
		action::LibraryAction,
		db::entities::{entry, tag, user_metadata, user_metadata_tag},
	},
	location::IndexMode,
	ops::tags::{
		apply::{action::ApplyTagsAction, input::ApplyTagsInput},
		create::{action::CreateTagAction, input::CreateTagInput},
	},
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use tokio::time::Duration;

#[tokio::test]
async fn test_tag_creation_and_application_with_events() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("tag_integration")
		.build()
		.await?;

	// Create test location with multiple files
	let test_location = harness.create_test_location("tag_test").await?;

	test_location
		.write_file("docs/file1.txt", "Document 1")
		.await?;
	test_location
		.write_file("docs/file2.txt", "Document 2")
		.await?;
	test_location
		.write_file("images/photo1.jpg", "Photo 1")
		.await?;

	let location = test_location
		.index("Tag Test Location", IndexMode::Deep)
		.await?;

	// Wait for indexing to settle
	tokio::time::sleep(Duration::from_millis(500)).await;

	let db = harness.library.db().conn();
	let action_manager = harness.core.context.get_action_manager().await.unwrap();
	let library_id = harness.library.id();

	// ==========================================
	// PART 1: Create tags with event validation
	// ==========================================

	// Start collecting events before tag creation
	let mut collector = EventCollector::with_capture(&harness.core.events);
	let collection_handle = tokio::spawn(async move {
		collector.collect_events(Duration::from_secs(3)).await;
		collector
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Create first tag
	let create_tag1 =
		CreateTagAction::from_input(CreateTagInput::simple("Important".to_string())).unwrap();
	let tag1_output = action_manager
		.dispatch_library(Some(library_id), create_tag1)
		.await?;
	let tag1_uuid = tag1_output.tag_id;

	tracing::info!("Created tag 'Important' with UUID: {}", tag1_uuid);

	// Create second tag with namespace
	let create_tag2 = CreateTagAction::from_input(CreateTagInput::with_namespace(
		"Work".to_string(),
		"projects".to_string(),
	))
	.unwrap();
	let tag2_output = action_manager
		.dispatch_library(Some(library_id), create_tag2)
		.await?;
	let tag2_uuid = tag2_output.tag_id;

	tracing::info!("Created tag 'Work' with UUID: {}", tag2_uuid);

	// Create third tag
	let create_tag3 =
		CreateTagAction::from_input(CreateTagInput::simple("Archive".to_string())).unwrap();
	let tag3_output = action_manager
		.dispatch_library(Some(library_id), create_tag3)
		.await?;
	let tag3_uuid = tag3_output.tag_id;

	tracing::info!("Created tag 'Archive' with UUID: {}", tag3_uuid);

	// Wait for events to settle
	tokio::time::sleep(Duration::from_millis(500)).await;

	let collector = collection_handle.await?;
	let stats = collector.analyze().await;

	tracing::info!("Tag creation events collected:");
	stats.print();

	// Verify tag creation emitted resource events
	let tag_events = stats
		.resource_changed
		.get("tag")
		.or_else(|| stats.resource_changed_batch.get("tag"))
		.copied()
		.unwrap_or(0);

	assert!(
		tag_events >= 3,
		"Expected at least 3 tag resource events (one per tag created), got {}",
		tag_events
	);

	// ==========================================
	// PART 2: Verify tags in database
	// ==========================================

	// Verify tag1 exists in database
	let tag1_model = tag::Entity::find()
		.filter(tag::Column::Uuid.eq(tag1_uuid))
		.one(db)
		.await?
		.expect("tag1 'Important' should exist in database");

	assert_eq!(tag1_model.canonical_name, "important");
	assert_eq!(tag1_model.display_name, Some("Important".to_string()));

	// Verify tag2 exists with namespace
	let tag2_model = tag::Entity::find()
		.filter(tag::Column::Uuid.eq(tag2_uuid))
		.one(db)
		.await?
		.expect("tag2 'Work' should exist in database");

	assert_eq!(tag2_model.canonical_name, "work");
	assert_eq!(tag2_model.namespace, Some("projects".to_string()));

	// Verify tag3 exists
	let tag3_model = tag::Entity::find()
		.filter(tag::Column::Uuid.eq(tag3_uuid))
		.one(db)
		.await?
		.expect("tag3 'Archive' should exist in database");

	tracing::info!("All 3 tags verified in database");

	// ==========================================
	// PART 3: Find entries to tag
	// ==========================================

	// Find the files we created
	let file1 = entry::Entity::find()
		.filter(entry::Column::Name.eq("file1.txt"))
		.one(db)
		.await?
		.expect("file1.txt should be indexed");

	let file2 = entry::Entity::find()
		.filter(entry::Column::Name.eq("file2.txt"))
		.one(db)
		.await?
		.expect("file2.txt should be indexed");

	let photo1 = entry::Entity::find()
		.filter(entry::Column::Name.eq("photo1.jpg"))
		.one(db)
		.await?
		.expect("photo1.jpg should be indexed");

	tracing::info!(
		"Found entries: file1={}, file2={}, photo1={}",
		file1.id,
		file2.id,
		photo1.id
	);

	// ==========================================
	// PART 4: Apply tags with event validation
	// ==========================================

	// Start collecting events for tag application
	let mut collector = EventCollector::with_capture(&harness.core.events);
	let collection_handle = tokio::spawn(async move {
		collector.collect_events(Duration::from_secs(3)).await;
		collector
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Apply "Important" tag to file1 and photo1
	let apply1 = ApplyTagsAction::from_input(ApplyTagsInput::user_tags_entry(
		vec![file1.id, photo1.id],
		vec![tag1_uuid],
	))
	.unwrap();
	action_manager
		.dispatch_library(Some(library_id), apply1)
		.await?;

	tracing::info!("Applied 'Important' tag to file1 and photo1");

	// Apply "Work" tag to file1 and file2 (file1 gets multiple tags)
	let apply2 = ApplyTagsAction::from_input(ApplyTagsInput::user_tags_entry(
		vec![file1.id, file2.id],
		vec![tag2_uuid],
	))
	.unwrap();
	action_manager
		.dispatch_library(Some(library_id), apply2)
		.await?;

	tracing::info!("Applied 'Work' tag to file1 and file2");

	// Apply "Archive" tag to photo1
	let apply3 = ApplyTagsAction::from_input(ApplyTagsInput::user_tags_entry(
		vec![photo1.id],
		vec![tag3_uuid],
	))
	.unwrap();
	action_manager
		.dispatch_library(Some(library_id), apply3)
		.await?;

	tracing::info!("Applied 'Archive' tag to photo1");

	// Wait for events to settle
	tokio::time::sleep(Duration::from_millis(500)).await;

	let collector = collection_handle.await?;
	let stats = collector.analyze().await;

	tracing::info!("Tag application events collected:");
	stats.print();

	// Verify file resource events were emitted when tags were applied
	let file_events = stats
		.resource_changed
		.get("file")
		.or_else(|| stats.resource_changed_batch.get("file"))
		.copied()
		.unwrap_or(0);

	assert!(
		file_events >= 3,
		"Expected at least 3 file resource events (files were tagged), got {}",
		file_events
	);

	// ==========================================
	// PART 5: Verify tag applications in database
	// ==========================================

	// Verify file1 has 2 tags (Important + Work)
	let file1_uuid = file1.uuid.expect("file1 should have UUID");
	let file1_metadata = user_metadata::Entity::find()
		.filter(user_metadata::Column::EntryUuid.eq(file1_uuid))
		.one(db)
		.await?
		.expect("file1 should have user_metadata");

	let file1_tag_count = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.eq(file1_metadata.id))
		.count(db)
		.await?;

	assert_eq!(
		file1_tag_count, 2,
		"file1 should have 2 tags (Important + Work)"
	);

	// Verify file1 has the correct tags
	let file1_tags = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.eq(file1_metadata.id))
		.all(db)
		.await?;

	let file1_tag_ids: Vec<i32> = file1_tags.iter().map(|t| t.tag_id).collect();
	assert!(
		file1_tag_ids.contains(&tag1_model.id),
		"file1 should have 'Important' tag"
	);
	assert!(
		file1_tag_ids.contains(&tag2_model.id),
		"file1 should have 'Work' tag"
	);

	tracing::info!("Verified file1 has 'Important' and 'Work' tags");

	// Verify file2 has 1 tag (Work)
	let file2_uuid = file2.uuid.expect("file2 should have UUID");
	let file2_metadata = user_metadata::Entity::find()
		.filter(user_metadata::Column::EntryUuid.eq(file2_uuid))
		.one(db)
		.await?
		.expect("file2 should have user_metadata");

	let file2_tag_count = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.eq(file2_metadata.id))
		.count(db)
		.await?;

	assert_eq!(file2_tag_count, 1, "file2 should have 1 tag (Work)");

	tracing::info!("Verified file2 has 'Work' tag");

	// Verify photo1 has 2 tags (Important + Archive)
	let photo1_uuid = photo1.uuid.expect("photo1 should have UUID");
	let photo1_metadata = user_metadata::Entity::find()
		.filter(user_metadata::Column::EntryUuid.eq(photo1_uuid))
		.one(db)
		.await?
		.expect("photo1 should have user_metadata");

	let photo1_tag_count = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.eq(photo1_metadata.id))
		.count(db)
		.await?;

	assert_eq!(
		photo1_tag_count, 2,
		"photo1 should have 2 tags (Important + Archive)"
	);

	let photo1_tags = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.eq(photo1_metadata.id))
		.all(db)
		.await?;

	let photo1_tag_ids: Vec<i32> = photo1_tags.iter().map(|t| t.tag_id).collect();
	assert!(
		photo1_tag_ids.contains(&tag1_model.id),
		"photo1 should have 'Important' tag"
	);
	assert!(
		photo1_tag_ids.contains(&tag3_model.id),
		"photo1 should have 'Archive' tag"
	);

	tracing::info!("Verified photo1 has 'Important' and 'Archive' tags");

	// ==========================================
	// PART 6: Query tags by file (reverse lookup)
	// ==========================================

	// Find all tags applied to file1
	let file1_applied_tags = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.eq(file1_metadata.id))
		.find_also_related(tag::Entity)
		.all(db)
		.await?;

	assert_eq!(
		file1_applied_tags.len(),
		2,
		"file1 should have 2 tag applications"
	);

	let file1_tag_names: Vec<String> = file1_applied_tags
		.iter()
		.filter_map(|(_, tag_opt)| tag_opt.as_ref())
		.map(|t| t.canonical_name.clone())
		.collect();

	assert!(file1_tag_names.contains(&"important".to_string()));
	assert!(file1_tag_names.contains(&"work".to_string()));

	tracing::info!("Verified file1 tag reverse lookup: {:?}", file1_tag_names);

	// ==========================================
	// SUCCESS
	// ==========================================

	tracing::info!("✅ All tag integration tests passed!");

	harness.shutdown().await?;
	Ok(())
}
