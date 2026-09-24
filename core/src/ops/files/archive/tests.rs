//! An archive round trip in both formats, with a folder collision, a path
//! escape refused, and an extract resumed at its entry.

use std::time::Duration;

use sd_store::file::FileKind;

use super::{
	input::{ArchiveFormat, FileArchiveInput, FileExtractInput},
	job::{ArchiveJob, ExtractJob},
	preflight::{ESCAPE, EXISTS},
	FileArchiveAction, FileExtractAction,
};
use crate::{
	domain::SdPath,
	infra::{
		action::preflight::{PreviewableAction, ValidatedAction},
		job::{journal::Effect, output::JobOutput},
	},
	ops::files::{
		fixture::{Fixture, T},
		merge::MergeConflictPolicy,
		plan::{ChangeKind, ConflictKind, PlanBasis},
	},
};

async fn wait(handle: crate::infra::job::handle::JobHandle) -> serde_json::Value {
	let output = tokio::time::timeout(Duration::from_secs(60), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	let JobOutput::Custom(value) = output else {
		panic!("not a custom output: {output:?}");
	};
	value
}

async fn round_trip(format: ArchiveFormat) {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	let tree = [
		("a.txt", file, 4, T, Some("a"), None, None),
		("sub/b.txt", file, 3, T, Some("b"), None, None),
		("sub/deep/c.txt", file, 2, T, Some("c"), None, None),
	];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	let archive = fixture.other.join(format!("src.{}", format.extension()));

	let input = FileArchiveInput {
		sources: vec![SdPath::local(&fixture.source)],
		destination: SdPath::local(&archive),
		format,
		remove_sources: false,
	};
	let validation = FileArchiveAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	assert_eq!(validation.facts.estimated_files, Some(3));
	let plan = FileArchiveAction::preview(input.clone(), &fixture.preview())
		.await
		.expect("planned");
	assert_eq!(plan.summary.creates.files, 1);
	assert_eq!(plan.summary.creates.bytes, 9);

	let handle = fixture
		.library
		.jobs()
		.dispatch(ArchiveJob::new(input.clone()))
		.await
		.expect("dispatched");
	let job_id = handle.id();
	let output = wait(handle).await;
	assert_eq!(output["files"], 3);
	assert!(archive.exists());
	let partial: Vec<_> = std::fs::read_dir(&fixture.other)
		.expect("dir")
		.filter_map(|entry| entry.ok())
		.filter(|entry| entry.file_name().to_string_lossy().contains("sd-partial"))
		.collect();
	assert!(partial.is_empty(), "no temporary file is left behind");
	let journal = fixture
		.library
		.jobs()
		.database()
		.journal(job_id)
		.await
		.expect("journal");
	assert!(matches!(journal[0].effect, Effect::Created { .. }));

	// Writing again at the same name is refused.
	let validation = FileArchiveAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(validation.errors().any(|finding| finding.code == EXISTS));

	// Extract into the destination, where a folder already holds one name
	// as a file and one file differs.
	let src_name = fixture
		.source
		.file_name()
		.expect("name")
		.to_string_lossy()
		.into_owned();
	std::fs::create_dir_all(fixture.destination.join(&src_name)).expect("dir");
	std::fs::write(fixture.destination.join(&src_name).join("a.txt"), b"old").expect("file");
	std::fs::create_dir_all(fixture.destination.join(&src_name).join("sub/b.txt"))
		.expect("dir in the way");

	let extract = FileExtractInput {
		archive: SdPath::local(&archive),
		destination: SdPath::local(&fixture.destination),
		on_conflict: MergeConflictPolicy::Overwrite,
		strip_components: 0,
	};
	let validation = FileExtractAction::validate(&extract, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	let plan = FileExtractAction::preview(extract.clone(), &fixture.preview())
		.await
		.expect("planned");
	assert!(matches!(plan.basis, PlanBasis::Archive { entries, .. } if entries >= 3));
	let at = |name: &str| {
		plan.changes
			.iter()
			.find(|change| {
				change.path.path().map(std::path::PathBuf::as_path)
					== Some(fixture.destination.join(&src_name).join(name).as_path())
			})
			.map(|change| change.change.clone())
	};
	assert!(matches!(at("a.txt"), Some(ChangeKind::Replace { .. })));
	assert_eq!(
		at("sub/b.txt"),
		Some(ChangeKind::Conflict {
			kind: ConflictKind::FileVsDirectory
		})
	);
	assert_eq!(at("sub/deep/c.txt"), Some(ChangeKind::Create { size: 2 }));

	let handle = fixture
		.library
		.jobs()
		.dispatch(ExtractJob::new(extract))
		.await
		.expect("dispatched");
	let output = wait(handle).await;
	assert_eq!(output["replaced"], 1, "{output}");
	assert_eq!(output["conflicts"], 1);
	assert_eq!(
		std::fs::read_to_string(fixture.destination.join(&src_name).join("a.txt"))
			.expect("replaced"),
		"xxxx"
	);
	assert_eq!(
		std::fs::read_to_string(fixture.destination.join(&src_name).join("sub/deep/c.txt"))
			.expect("created"),
		"xx"
	);
}

#[tokio::test]
async fn a_zip_round_trips_with_a_collision_and_a_replacement() {
	round_trip(ArchiveFormat::Zip).await;
}

#[tokio::test]
async fn a_tar_zstd_round_trips_with_a_collision_and_a_replacement() {
	round_trip(ArchiveFormat::TarZstd).await;
}

/// An entry that would land outside the destination refuses the extract,
/// and stripping a component lands the rest one level up.
#[tokio::test]
async fn an_escaping_entry_is_refused_and_components_strip() {
	let fixture = Fixture::new().await;
	let archive = fixture.other.join("bad.zip");
	{
		let file = std::fs::File::create(&archive).expect("file");
		let mut writer = zip::ZipWriter::new(file);
		let options = zip::write::SimpleFileOptions::default();
		writer.start_file("top/inner.txt", options).expect("entry");
		std::io::Write::write_all(&mut writer, b"in").expect("bytes");
		writer.start_file("../escape.txt", options).expect("entry");
		std::io::Write::write_all(&mut writer, b"out").expect("bytes");
		writer.finish().expect("finished");
	}
	let extract = FileExtractInput {
		archive: SdPath::local(&archive),
		destination: SdPath::local(&fixture.destination),
		on_conflict: MergeConflictPolicy::Skip,
		strip_components: 0,
	};
	let validation = FileExtractAction::validate(&extract, &fixture.preview())
		.await
		.expect("validated");
	assert!(validation.errors().any(|finding| finding.code == ESCAPE));

	let stripped = FileExtractInput {
		strip_components: 1,
		..extract
	};
	let plan = FileExtractAction::preview(stripped, &fixture.preview())
		.await
		.expect("planned");
	assert!(plan.changes.iter().any(|change| {
		change.path.path().map(std::path::PathBuf::as_path)
			== Some(fixture.destination.join("inner.txt").as_path())
			&& matches!(change.change, ChangeKind::Create { .. })
	}));
}

/// A resumed extract starts at the entry it reached and writes the rest.
#[tokio::test]
async fn a_resumed_extract_continues_at_its_entry() {
	let fixture = Fixture::new().await;
	let archive = fixture.other.join("many.zip");
	{
		let file = std::fs::File::create(&archive).expect("file");
		let mut writer = zip::ZipWriter::new(file);
		let options = zip::write::SimpleFileOptions::default();
		for index in 0..6 {
			writer
				.start_file(format!("f{index}.txt"), options)
				.expect("entry");
			std::io::Write::write_all(&mut writer, format!("{index}").as_bytes()).expect("bytes");
		}
		writer.finish().expect("finished");
	}
	let mut job = ExtractJob::new(FileExtractInput {
		archive: SdPath::local(&archive),
		destination: SdPath::local(&fixture.destination),
		on_conflict: MergeConflictPolicy::Skip,
		strip_components: 0,
	});
	// The first three entries were written before the interruption.
	for index in 0..3 {
		std::fs::write(fixture.destination.join(format!("f{index}.txt")), b"kept").expect("file");
	}
	job.next = 3;
	let handle = fixture
		.library
		.jobs()
		.dispatch(job)
		.await
		.expect("dispatched");
	let output = wait(handle).await;
	assert_eq!(output["extracted"], 3, "{output}");
	for index in 0..3 {
		assert_eq!(
			std::fs::read_to_string(fixture.destination.join(format!("f{index}.txt"))).unwrap(),
			"kept",
			"entries before the cursor are left as they were"
		);
	}
	for index in 3..6 {
		assert_eq!(
			std::fs::read_to_string(fixture.destination.join(format!("f{index}.txt"))).unwrap(),
			index.to_string()
		);
	}
}
