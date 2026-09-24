//! Attributes preview what changes, refuse what a filesystem cannot carry,
//! and undo sets them back.

use std::time::Duration;

use sd_store::file::FileKind;

use super::{
	fs, preflight::UNSUPPORTED, AttributesJob, FileSetAttributesAction, FileSetAttributesInput,
};
use crate::{
	domain::SdPath,
	infra::{
		action::preflight::{PreviewableAction, ValidatedAction},
		job::{journal::Attributes, output::JobOutput},
	},
	ops::files::{
		fixture::{Fixture, T},
		plan::ChangeKind,
		undo::UndoJob,
	},
	volume::types::FileSystem,
};

#[test]
fn exfat_refuses_a_mode_and_a_dot_platform_refuses_hidden() {
	let mode = Attributes {
		mode: Some(0o600),
		modified_ms: None,
		hidden: None,
	};
	assert!(fs::supported(&FileSystem::ExFAT, &mode).is_err());
	assert!(fs::supported(&FileSystem::FAT32, &mode).is_err());
	assert!(fs::supported(&FileSystem::APFS, &mode).is_ok());
	let hidden = Attributes {
		mode: None,
		modified_ms: None,
		hidden: Some(true),
	};
	assert_eq!(
		fs::supported(&FileSystem::Ext4, &hidden).is_ok(),
		cfg!(any(target_os = "macos", target_os = "windows"))
	);
}

#[cfg(unix)]
#[tokio::test]
async fn attributes_preview_what_changes_and_undo_sets_them_back() {
	let fixture = Fixture::new().await;
	let tree = [("a.txt", FileKind::File, 4, T, Some("a"), None, None)];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	let a = fixture.source.join("a.txt");
	let before = fs::read(&a).await.expect("read");

	let input = FileSetAttributesInput {
		paths: vec![SdPath::local(&a)],
		attributes: Attributes {
			mode: Some(0o600),
			modified_ms: Some(1_700_000_000_000),
			hidden: None,
		},
	};
	let validation = FileSetAttributesAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	assert!(!validation
		.errors()
		.any(|finding| finding.code == UNSUPPORTED));
	let plan = FileSetAttributesAction::preview(input.clone(), &fixture.preview())
		.await
		.expect("planned");
	assert_eq!(plan.summary.attributes, 1);
	assert!(matches!(
		plan.changes[0].change,
		ChangeKind::SetAttributes {
			attributes: Attributes {
				mode: Some(0o600),
				..
			}
		}
	));

	let handle = fixture
		.library
		.jobs()
		.dispatch(AttributesJob::new(input))
		.await
		.expect("dispatched");
	let job = handle.id();
	let output = tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	let JobOutput::Custom(value) = output else {
		panic!("{output:?}");
	};
	assert_eq!(value["changed"], 1);
	let after = fs::read(&a).await.expect("read");
	assert_eq!(after.mode, Some(0o600));
	assert_eq!(after.modified_ms, Some(1_700_000_000_000));

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
	let restored = fs::read(&a).await.expect("read");
	assert_eq!(restored.mode, before.mode);
	assert_eq!(restored.modified_ms, before.modified_ms);
}
