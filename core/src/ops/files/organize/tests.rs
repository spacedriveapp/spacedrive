//! Organize previews the folders and moves and leaves records with their
//! identity; flatten numbers collisions and prunes the folders it empties.

use std::{path::PathBuf, time::Duration};

use sd_store::file::FileKind;

use super::{
	input::{
		FileFlattenInput, FileOrganizeInput, FlattenPolicy, Granularity, OrganizeDateField,
		OrganizeRule,
	},
	job::{Rearrange, RearrangeJob},
	preflight::CONFLICTS,
	FileFlattenAction, FileOrganizeAction,
};
use crate::{
	domain::SdPath,
	infra::{
		action::preflight::{PreviewableAction, ValidatedAction},
		job::output::JobOutput,
	},
	ops::files::{
		fixture::{Fixture, T},
		plan::{ChangeKind, ConflictKind, FsPlan},
	},
};

fn change_at(plan: &FsPlan, path: &std::path::Path) -> Option<ChangeKind> {
	plan.changes
		.iter()
		.find(|change| change.path.path().map(PathBuf::as_path) == Some(path))
		.map(|change| change.change.clone())
}

async fn run(fixture: &Fixture, what: Rearrange) -> serde_json::Value {
	let handle = fixture
		.library
		.jobs()
		.dispatch(RearrangeJob::new(what))
		.await
		.expect("dispatched");
	let output = tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	let JobOutput::Custom(value) = output else {
		panic!("not a rearrange output: {output:?}");
	};
	value
}

/// Organizing by month plans a folder per month and a move per file, and
/// each move is a rename, so the inode the record is matched by survives.
#[tokio::test]
async fn organizing_by_month_previews_folders_and_moves_and_keeps_identity() {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	// May and June 2024, in local time well inside each month.
	let may = 1_715_500_000_000;
	let june = 1_718_100_000_000;
	let tree = [
		("a.jpg", file, 4, may, Some("a"), None, None),
		("b.jpg", file, 3, may, Some("b"), None, None),
		("c.mov", file, 2, june, Some("c"), None, None),
		("2024-06/d.mov", file, 2, june, Some("d"), None, None),
	];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;

	let input = FileOrganizeInput {
		scope: SdPath::local(&fixture.source),
		rule: OrganizeRule::ByDate {
			field: OrganizeDateField::Modified,
			granularity: Granularity::YearMonth,
		},
		recursive: true,
	};
	let validation = FileOrganizeAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	assert_eq!(validation.facts.estimated_files, Some(3));

	let plan = FileOrganizeAction::preview(input.clone(), &fixture.preview())
		.await
		.expect("planned");
	assert_eq!(
		change_at(&plan, &fixture.source.join("2024-05")),
		Some(ChangeKind::CreateDirectory)
	);
	assert_eq!(
		change_at(&plan, &fixture.source.join("2024-05/a.jpg")),
		Some(ChangeKind::Move {
			from: SdPath::local(fixture.source.join("a.jpg"))
		})
	);
	assert_eq!(
		change_at(&plan, &fixture.source.join("2024-06/d.mov")),
		None,
		"already in place"
	);
	assert_eq!(plan.summary.moves.files, 3);
	assert_eq!(plan.summary.directories_created, 1);

	let inode_before = inode(&fixture.source.join("a.jpg"));
	let output = run(&fixture, Rearrange::Organize(input)).await;
	assert_eq!(output["moved"], 3);
	assert_eq!(output["directories_created"], 1);
	assert!(fixture.source.join("2024-05/a.jpg").exists());
	assert!(fixture.source.join("2024-06/c.mov").exists());
	assert!(!fixture.source.join("a.jpg").exists());
	assert_eq!(
		inode(&fixture.source.join("2024-05/a.jpg")),
		inode_before,
		"a rename keeps the inode the record is matched by"
	);
}

#[cfg(unix)]
fn inode(path: &std::path::Path) -> u64 {
	use std::os::unix::fs::MetadataExt;
	std::fs::metadata(path).expect("metadata").ino()
}

#[cfg(not(unix))]
fn inode(_path: &std::path::Path) -> u64 {
	0
}

/// Organizing by kind and by extension names the folders, and two files
/// wanting one place are a conflict left alone.
#[tokio::test]
async fn organizing_by_kind_names_folders_and_leaves_conflicts() {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	let tree = [
		("photo.jpg", file, 4, T, Some("p"), None, None),
		("notes.txt", file, 3, T, Some("n"), None, None),
		("README", file, 2, T, Some("r"), None, None),
		("old/photo.jpg", file, 5, T, Some("o"), None, None),
	];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;

	let by_kind = FileOrganizeInput {
		scope: SdPath::local(&fixture.source),
		rule: OrganizeRule::ByKind,
		recursive: true,
	};
	let validation = FileOrganizeAction::validate(&by_kind, &fixture.preview())
		.await
		.expect("validated");
	assert!(validation
		.findings
		.iter()
		.any(|finding| finding.code == CONFLICTS));
	let plan = FileOrganizeAction::preview(by_kind, &fixture.preview())
		.await
		.expect("planned");
	assert!(matches!(
		change_at(&plan, &fixture.source.join("Images/photo.jpg")),
		Some(ChangeKind::Move { .. })
			| Some(ChangeKind::Conflict {
				kind: ConflictKind::Sources
			})
	));
	assert_eq!(plan.summary.conflicts, 1);
	assert!(matches!(
		change_at(&plan, &fixture.source.join("Text/notes.txt")),
		Some(ChangeKind::Move { .. })
	));

	let by_extension = FileOrganizeInput {
		scope: SdPath::local(&fixture.source),
		rule: OrganizeRule::ByExtension,
		recursive: false,
	};
	let plan = FileOrganizeAction::preview(by_extension, &fixture.preview())
		.await
		.expect("planned");
	assert!(matches!(
		change_at(&plan, &fixture.source.join("jpg/photo.jpg")),
		Some(ChangeKind::Move { .. })
	));
	assert!(matches!(
		change_at(&plan, &fixture.source.join("No extension/README")),
		Some(ChangeKind::Move { .. })
	));
	assert_eq!(
		change_at(&plan, &fixture.source.join("jpg/photo.jpg")).is_some(),
		true
	);
	assert_eq!(
		plan.summary.moves.files, 3,
		"the nested file is left out without recursive"
	);
}

/// Flattening numbers a collision under keep both, leaves it under skip,
/// and prunes the folders it emptied.
#[tokio::test]
async fn flattening_numbers_collisions_and_prunes_emptied_folders() {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	let tree = [
		("a.txt", file, 4, T, Some("a"), None, None),
		("x/a.txt", file, 3, T, Some("xa"), None, None),
		("x/deep/b.txt", file, 2, T, Some("b"), None, None),
		("y/c.txt", file, 1, T, Some("c"), None, None),
	];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;

	let skip = FileFlattenInput {
		scope: SdPath::local(&fixture.source),
		on_conflict: FlattenPolicy::Skip,
	};
	let plan = FileFlattenAction::preview(skip, &fixture.preview())
		.await
		.expect("planned");
	assert_eq!(plan.summary.moves.files, 2);
	assert_eq!(plan.summary.skips.policy.files, 1);

	let keep_both = FileFlattenInput {
		scope: SdPath::local(&fixture.source),
		on_conflict: FlattenPolicy::KeepBoth,
	};
	let validation = FileFlattenAction::validate(&keep_both, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses());
	let plan = FileFlattenAction::preview(keep_both.clone(), &fixture.preview())
		.await
		.expect("planned");
	assert_eq!(
		change_at(&plan, &fixture.source.join("a (1).txt")),
		Some(ChangeKind::Move {
			from: SdPath::local(fixture.source.join("x/a.txt"))
		})
	);
	assert_eq!(plan.summary.moves.files, 3);

	let output = run(&fixture, Rearrange::Flatten(keep_both)).await;
	assert_eq!(output["moved"], 3);
	assert_eq!(output["pruned_directories"], 3);
	assert_eq!(
		std::fs::read_to_string(fixture.source.join("a (1).txt")).expect("numbered"),
		"xxx"
	);
	assert!(fixture.source.join("b.txt").exists());
	assert!(fixture.source.join("c.txt").exists());
	assert!(!fixture.source.join("x").exists());
	assert!(!fixture.source.join("y").exists());
}
