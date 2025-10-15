//! Focused integration test: verifies tag creation and application persist to DB

use sd_core::domain::SdPath;
use sd_core::infra::db::entities::{entry, tag, user_metadata, user_metadata_tag};
use sd_core::{
	infra::action::LibraryAction,
	ops::indexing::IndexMode,
	ops::locations::add::action::{LocationAddAction, LocationAddInput},
	ops::tags::{
		apply::{action::ApplyTagsAction, input::ApplyTagsInput},
		create::{action::CreateTagAction, input::CreateTagInput},
	},
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;

/// Helper: create file with content (ensures parent dirs)
async fn write_file(path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).await?;
	}
	fs::write(path, content).await
}

/// Helper: find entry by name under optional parent_id
async fn find_entry_by_name(
	db: &sea_orm::DatabaseConnection,
	name: &str,
	parent_id: Option<i32>,
) -> Result<Option<entry::Model>, sea_orm::DbErr> {
	let mut query = entry::Entity::find().filter(entry::Column::Name.eq(name));
	if let Some(pid) = parent_id {
		query = query.filter(entry::Column::ParentId.eq(pid));
	} else {
		query = query.filter(entry::Column::ParentId.is_null());
	}
	query.one(db).await
}

#[tokio::test]
async fn test_tagging_persists_to_database() {
	// Use a clean, test-scoped data directory
	let data_dir = std::path::PathBuf::from("core/data/tagging-persistence-test");
	if data_dir.exists() {
		std::fs::remove_dir_all(&data_dir).unwrap();
	}
	std::fs::create_dir_all(&data_dir).unwrap();

	// Init Core and a fresh library
	let core = Arc::new(Core::new(data_dir.clone()).await.unwrap());
	let library = core
		.libraries
		.create_library(
			"Tagging Persistence Test Library",
			None,
			core.context.clone(),
		)
		.await
		.unwrap();
	let library_id = library.id();
	let action_manager = core.context.get_action_manager().await.unwrap();

	// Create a temp source with a single directory named "target_dir" (directories receive UUIDs)
	let temp_dir = TempDir::new().unwrap();
	let source_dir = temp_dir.path().join("source");
	let target_dir = source_dir.join("target_dir");
	fs::create_dir_all(&target_dir).await.unwrap();

	// Index the location (deep)
	let add_loc = LocationAddAction::from_input(LocationAddInput {
		path: SdPath::local(source_dir.clone()),
		name: Some("Source".to_string()),
		mode: IndexMode::Deep,
	})
	.unwrap();
	let _ = action_manager
		.dispatch_library(Some(library_id), add_loc)
		.await
		.unwrap();

	// Allow async indexing to complete
	tokio::time::sleep(std::time::Duration::from_secs(3)).await;

	let db = library.db().conn();

	// Resolve entries: first the root source, then the directory "target_dir"
	let source_entry = find_entry_by_name(db, "source", None)
		.await
		.unwrap()
		.expect("source entry missing");
	let target_entry = find_entry_by_name(db, "target_dir", Some(source_entry.id))
		.await
		.unwrap()
		.expect("target_dir entry missing");
	let target_uuid = target_entry
		.uuid
		.expect("target_dir entry should have a UUID after indexing");

	// Create a tag via action
	let create_tag =
		CreateTagAction::from_input(CreateTagInput::simple("Tag A".to_string())).unwrap();
	let create_out = action_manager
		.dispatch_library(Some(library_id), create_tag)
		.await
		.unwrap();
	let tag_uuid = create_out.tag_id;

	// Apply the tag to target entry via action
	let apply = ApplyTagsAction::from_input(ApplyTagsInput::user_tags(
		vec![target_entry.id],
		vec![tag_uuid],
	))
	.unwrap();
	let _ = action_manager
		.dispatch_library(Some(library_id), apply)
		.await
		.unwrap();

	// Verify: tag row exists
	let tag_model = tag::Entity::find()
		.filter(tag::Column::Uuid.eq(tag_uuid))
		.one(db)
		.await
		.unwrap()
		.expect("tag row not found");

	// Verify: user_metadata exists for entry_uuid
	let metadata_model = user_metadata::Entity::find()
		.filter(user_metadata::Column::EntryUuid.eq(target_uuid))
		.one(db)
		.await
		.unwrap()
		.expect("user_metadata row for entry not found");

	// Verify: link exists in user_metadata_tag for (metadata_id, tag_db_id)
	let link_count = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.eq(metadata_model.id))
		.filter(user_metadata_tag::Column::TagId.eq(tag_model.id))
		.count(db)
		.await
		.unwrap();

	assert_eq!(link_count, 1, "expected one user_metadata_tag link");
}
