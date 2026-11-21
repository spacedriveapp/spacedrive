//! Acceptance for indexed content search (`docs/core/content-search.mdx`):
//! tracking a folder builds its text index, a query returns ranked hits
//! with snippets, the index follows a modify, a rename and a delete, and the
//! index can be dropped and rebuilt.
//!
//! One `Core` over temporary directories, with the watcher off so each step
//! is driven by a walk the way a returning volume or a re-track drives it.

mod helpers;

use std::{path::Path, sync::Arc, time::Duration};

use helpers::*;
use sd_core::{
	domain::addressing::SdPath,
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::{AddOverrides, Library},
	ops::{
		indexing::{text_extract::TextKind, SourceStore},
		search::content::{
			ContentSearchInput, ContentSearchOutput, ContentSearchQuery, RebuildContentIndexAction,
			RebuildContentIndexInput,
		},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
};

fn session(library: &Library) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	session
}

async fn track(
	harness: &IndexingHarness,
	path: &Path,
	overrides: AddOverrides,
) -> Arc<SourceStore> {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: path.to_path_buf(),
		name: None,
		overrides,
	})
	.map_err(anyhow::Error::msg)
	.unwrap()
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.unwrap();
	if let Some(job) = output.job_id {
		if let Some(walk) = harness
			.library
			.jobs()
			.get_job(sd_core::infra::job::types::JobId(job))
			.await
		{
			walk.wait().await.unwrap();
		}
	}
	harness
		.core
		.context
		.volume_index()
		.store_for(&output.root)
		.await
		.expect("the tracked source has a store")
}

async fn search(
	harness: &IndexingHarness,
	scope: &Path,
	query: &str,
	kinds: Vec<TextKind>,
) -> Result<ContentSearchOutput, String> {
	ContentSearchQuery::from_input(ContentSearchInput {
		scope: SdPath::local(scope),
		query: query.to_string(),
		kinds,
		max_results: None,
		cursor: None,
	})
	.map_err(|e| e.to_string())?
	.execute(harness.core.context.clone(), session(&harness.library))
	.await
	.map_err(|e| e.to_string())
}

fn paths(output: &ContentSearchOutput) -> Vec<&str> {
	output.hits.iter().map(|hit| hit.path.as_str()).collect()
}

/// Search until the index has no pending files and `done` holds, or fail.
async fn wait_for(
	harness: &IndexingHarness,
	scope: &Path,
	query: &str,
	done: impl Fn(&ContentSearchOutput) -> bool,
) -> ContentSearchOutput {
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let output = search(harness, scope, query, vec![]).await.expect("search");
		if output.pending_files == 0 && done(&output) {
			return output;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"index never settled for {query:?}: {output:?}"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn the_text_index_builds_answers_and_follows_the_files() {
	let harness = IndexingHarnessBuilder::new("content_search_acceptance")
		.disable_watcher()
		.build()
		.await
		.unwrap();
	let dir = harness.create_test_dir("docs").await.unwrap();
	let root = dir.path().to_path_buf();
	dir.write_file(
		"notes/plan.md",
		"# Plan\n\nThe falcon project ships in spring.",
	)
	.await
	.unwrap();
	dir.write_file(
		"notes/log.txt",
		"falcon falcon falcon: three mentions make this the best match",
	)
	.await
	.unwrap();
	dir.write_file("src/main.rs", "fn falcon() -> u32 { 7 }\n")
		.await
		.unwrap();
	dir.write_file("data.csv", "bird,speed\nheron,slow\n")
		.await
		.unwrap();
	std::fs::write(root.join("photo.jpg"), b"falcon in a jpeg header").unwrap();
	// Past the cap, so a word placed after it is never found.
	let mut big = "padding ".repeat(140_000);
	big.push_str("beyondthecap");
	dir.write_file("big.txt", &big).await.unwrap();

	let store = track(&harness, &root, AddOverrides::default()).await;

	// Ranked hits with snippets, best match first; the image is not read.
	let out = wait_for(&harness, &root, "falcon", |o| o.hits.len() == 3).await;
	assert_eq!(paths(&out)[0], "notes/log.txt", "{out:?}");
	let mut found = paths(&out);
	found.sort();
	assert_eq!(found, ["notes/log.txt", "notes/plan.md", "src/main.rs"]);
	assert!(out.hits.windows(2).all(|w| w[0].score >= w[1].score));
	let plan = out.hits.iter().find(|h| h.path == "notes/plan.md").unwrap();
	assert_eq!(plan.kind, TextKind::Markdown);
	assert!(plan
		.snippet
		.iter()
		.any(|part| part.matched && part.text == "falcon"));
	let text_index = store.text_index().await.unwrap();
	assert_eq!(text_index.counts().await.unwrap().truncated, 1);
	assert!(search(&harness, &root, "beyondthecap", vec![])
		.await
		.unwrap()
		.hits
		.is_empty());

	// Path and kind filters.
	let code = search(&harness, &root, "falcon", vec![TextKind::Code])
		.await
		.unwrap();
	assert_eq!(paths(&code), ["src/main.rs"]);
	let notes = search(&harness, &root.join("notes"), "falcon", vec![])
		.await
		.unwrap();
	let mut notes = paths(&notes);
	notes.sort();
	assert_eq!(notes, ["log.txt", "plan.md"]);
	let phrase = search(&harness, &root, "\"ships in spring\"", vec![])
		.await
		.unwrap();
	assert_eq!(paths(&phrase), ["notes/plan.md"]);

	// Modify: new words replace old ones.
	std::fs::write(
		root.join("notes/plan.md"),
		"# Plan\n\nThe osprey project ships in autumn instead.",
	)
	.unwrap();
	track(&harness, &root, AddOverrides::default()).await;
	wait_for(&harness, &root, "osprey", |o| paths(o) == ["notes/plan.md"]).await;
	assert!(search(&harness, &root, "spring", vec![])
		.await
		.unwrap()
		.hits
		.is_empty());

	// Rename: the hit follows the file.
	std::fs::rename(root.join("notes/plan.md"), root.join("src/plan-renamed.md")).unwrap();
	track(&harness, &root, AddOverrides::default()).await;
	wait_for(&harness, &root, "osprey", |o| {
		paths(o) == ["src/plan-renamed.md"]
	})
	.await;

	// Delete, as the watcher reports one: the file leaves results at once,
	// and the next pass prunes its row.
	let rows = text_index.counts().await.unwrap().files;
	std::fs::remove_file(root.join("src/main.rs")).unwrap();
	store.lost(&root.join("src/main.rs"), false).await;
	store.flush().await.unwrap();
	let out = search(&harness, &root, "falcon", vec![]).await.unwrap();
	assert_eq!(paths(&out), ["notes/log.txt"]);
	harness
		.library
		.jobs()
		.dispatch(sd_core::ops::indexing::content_text::ContentTextJob::new(
			root.clone(),
			true,
		))
		.await
		.unwrap()
		.wait()
		.await
		.unwrap();
	assert_eq!(text_index.counts().await.unwrap().files, rows - 1);

	// Drop and rebuild.
	let rebuilt =
		RebuildContentIndexAction::from_input(RebuildContentIndexInput { path: root.clone() })
			.unwrap()
			.execute(harness.library.clone(), harness.core.context.clone())
			.await
			.unwrap();
	assert!(rebuilt.cleared_files >= 4, "{rebuilt:?}");
	if let Some(job) = rebuilt.job_id {
		if let Some(pass) = harness
			.library
			.jobs()
			.get_job(sd_core::infra::job::types::JobId(job))
			.await
		{
			pass.wait().await.unwrap();
		}
	}
	let out = wait_for(&harness, &root, "falcon", |o| o.hits.len() == 1).await;
	assert_eq!(paths(&out), ["notes/log.txt"]);
	assert_eq!(
		paths(&search(&harness, &root, "osprey", vec![]).await.unwrap()),
		["src/plan-renamed.md"]
	);

	// A source that does not read contents has no index to search.
	let private = harness.create_test_dir("private").await.unwrap();
	private.write_file("a.md", "falcon").await.unwrap();
	track(
		&harness,
		private.path(),
		AddOverrides {
			identify_content: Some(false),
			..AddOverrides::default()
		},
	)
	.await;
	let refused = search(&harness, private.path(), "falcon", vec![]).await;
	assert!(
		refused.as_ref().is_err_and(|e| e.contains("search.grep")),
		"{refused:?}"
	);

	harness.shutdown().await.unwrap();
}
