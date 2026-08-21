mod common;

use uuid::Uuid;

use sd_client::{CoreClient, LibraryContext, Wire};
use sd_core::ops::config::library::get::GetLibraryConfigQueryInput;

use common::MockDaemon;

/// Client pointed at a closed port; persistence tests never touch the wire.
fn offline_client() -> CoreClient {
	CoreClient::new("127.0.0.1:1".to_string())
}

#[tokio::test]
async fn persists_selection_across_reloads() {
	let dir = tempfile::tempdir().unwrap();
	let state_path = dir.path().join("state").join("library.json");
	let library_id = Uuid::new_v4();

	let ctx = LibraryContext::load(offline_client(), &state_path)
		.await
		.unwrap();
	assert_eq!(ctx.current(), None, "fresh state starts unselected");

	ctx.set_current(Some(library_id)).await.unwrap();
	assert_eq!(ctx.current(), Some(library_id));

	// A new context over the same file sees the persisted selection.
	let reloaded = LibraryContext::load(offline_client(), &state_path)
		.await
		.unwrap();
	assert_eq!(reloaded.current(), Some(library_id));

	// Clearing the selection persists too.
	reloaded.set_current(None).await.unwrap();
	let reloaded = LibraryContext::load(offline_client(), &state_path)
		.await
		.unwrap();
	assert_eq!(reloaded.current(), None);
}

#[tokio::test]
async fn malformed_state_file_is_an_error() {
	let dir = tempfile::tempdir().unwrap();
	let state_path = dir.path().join("library.json");
	tokio::fs::write(&state_path, b"not json").await.unwrap();

	assert!(LibraryContext::load(offline_client(), &state_path)
		.await
		.is_err());
}

#[tokio::test]
async fn watchers_are_notified_on_change_only() {
	let dir = tempfile::tempdir().unwrap();
	let state_path = dir.path().join("library.json");
	let library_id = Uuid::new_v4();

	let ctx = LibraryContext::load(offline_client(), &state_path)
		.await
		.unwrap();
	let mut watcher = ctx.watch();

	ctx.set_current(Some(library_id)).await.unwrap();
	watcher.changed().await.unwrap();
	assert_eq!(*watcher.borrow_and_update(), Some(library_id));

	// Re-selecting the same library is a no-op: no notification.
	ctx.set_current(Some(library_id)).await.unwrap();
	assert!(!watcher.has_changed().unwrap());
}

#[tokio::test]
async fn query_and_action_inject_the_current_library_id() {
	// Any Wire-registered input works: the mock echoes the request envelope
	// so the test can assert what the context injected.
	let query = GetLibraryConfigQueryInput;
	let method = <GetLibraryConfigQueryInput as Wire>::METHOD;

	let mock = MockDaemon::start().await;
	let dir = tempfile::tempdir().unwrap();
	let library_id = Uuid::new_v4();

	let ctx = LibraryContext::load(
		CoreClient::new(mock.addr()),
		dir.path().join("library.json"),
	)
	.await
	.unwrap();

	// With no selection, no library id is injected.
	let echoed: serde_json::Value = ctx.query(&query).await.unwrap();
	assert_eq!(echoed["method"], method);
	assert_eq!(echoed["library_id"], serde_json::Value::Null);

	ctx.set_current(Some(library_id)).await.unwrap();

	let echoed: serde_json::Value = ctx.query(&query).await.unwrap();
	assert_eq!(echoed["library_id"], library_id.to_string().as_str());

	let echoed = ctx.action(&query).await.unwrap();
	assert_eq!(echoed["method"], method);
	assert_eq!(echoed["library_id"], library_id.to_string().as_str());
}
