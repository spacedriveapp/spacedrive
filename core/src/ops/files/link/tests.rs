use std::time::Duration;

use sd_store::file::FileKind;

use super::{
	preflight::{hard_link_allowed, CROSS_VOLUME, DIRECTORY, EXISTS},
	FileLinkAction, FileLinkInput, LinkJob, LinkKind,
};
use crate::{
	domain::SdPath,
	infra::action::preflight::{PreviewableAction, ValidatedAction},
	ops::files::{
		fixture::{Fixture, T},
		undo::UndoJob,
	},
};

#[test]
fn a_hard_link_needs_one_volume_and_a_file() {
	assert_eq!(hard_link_allowed(true, false), Ok(()));
	assert_eq!(hard_link_allowed(false, false), Err(CROSS_VOLUME));
	assert_eq!(hard_link_allowed(true, true), Err(DIRECTORY));
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_previews_one_create_and_undo_removes_it() {
	let fixture = Fixture::new().await;
	let tree = [("a.txt", FileKind::File, 4, T, Some("a"), None, None)];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	let at = fixture.source.join("a-link.txt");
	let input = FileLinkInput {
		at: SdPath::local(&at),
		target: SdPath::local(fixture.source.join("a.txt")),
		kind: LinkKind::Symlink,
	};
	let validation = FileLinkAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	let plan = FileLinkAction::preview(input.clone(), &fixture.preview())
		.await
		.expect("planned");
	assert_eq!(plan.summary.creates.files, 1);

	let handle = fixture
		.library
		.jobs()
		.dispatch(LinkJob::new(input.clone()))
		.await
		.expect("dispatched");
	let job = handle.id();
	tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	assert!(std::fs::symlink_metadata(&at)
		.expect("linked")
		.file_type()
		.is_symlink());

	let validation = FileLinkAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(validation.errors().any(|finding| finding.code == EXISTS));

	let handle = fixture
		.library
		.jobs()
		.dispatch(UndoJob::new(job.0, None))
		.await
		.expect("dispatched");
	tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	assert!(std::fs::symlink_metadata(&at).is_err(), "the link is gone");
}
