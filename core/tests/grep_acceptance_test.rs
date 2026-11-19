//! `search.grep` reads a live tree the way a person would expect a code search
//! to: ignore files are honored, binary and oversized files are passed over,
//! odd names survive, symlinks never lead out of the scope, and paging returns
//! every match exactly once.
//!
//! The fixture is built fresh in a temporary directory. The scan tests call
//! the scan directly with the fixture as scope; the query tests track the
//! fixture as a source and go through `search.grep` the way a client does.

mod helpers;

use std::{
	path::{Path, PathBuf},
	sync::{atomic::AtomicBool, Arc},
};

use helpers::TestConfigBuilder;
use sd_core::{
	domain::addressing::SdPath,
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::Library,
	ops::{
		search::grep::{
			scan::{self, GrepError},
			GrepInput, GrepMatch, GrepOptions, GrepOutput, GrepQuery,
		},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use tempfile::TempDir;

const BIG_FILE_BYTES: usize = 2 * 1024 * 1024;

/// `<tmp>/tree` is the scope; `<tmp>/outside` is what its symlinks point at.
struct Fixture {
	_dir: TempDir,
	root: PathBuf,
}

fn fixture() -> Fixture {
	let dir = TempDir::new().expect("tempdir");
	let root = dir.path().join("tree");
	let outside = dir.path().join("outside");
	let write = |rel: &str, bytes: &[u8]| {
		let path = root.join(rel);
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(path, bytes).unwrap();
	};

	write(".gitignore", b"ignored/\n*.log\n");
	write("src/main.rs", b"fn main() {\n\tlet x = \"needle\";\n}\n");
	write(
		"src/nested/deep/lib.rs",
		b"// one\n// needle two\n// three\n// four\n// NEEDLE five\n// needle( six\n",
	);
	write("ignored/secret.txt", b"needle in an ignored dir\n");
	write("debug.log", b"needle in an ignored file\n");
	write(".hidden/notes.txt", b"needle in a hidden dir\n");
	write("blob.bin", b"needle\0\x01\x02needle\n");
	let mut big = b"needle at the top of a big file\n".to_vec();
	big.resize(BIG_FILE_BYTES, b'x');
	write("big.txt", &big);
	write("crlf.txt", b"needle with crlf\r\nsecond\r\n");

	std::fs::create_dir_all(&outside).unwrap();
	std::fs::write(outside.join("escape.txt"), b"needle outside the scope\n").unwrap();

	#[cfg(unix)]
	{
		use std::os::unix::ffi::OsStrExt;
		let name = std::ffi::OsStr::from_bytes(b"caf\xe9.txt");
		std::fs::write(root.join(name), b"a needle in a latin-1 name \xff\n").unwrap();
		std::os::unix::fs::symlink(&outside, root.join("link_dir")).unwrap();
		std::os::unix::fs::symlink(outside.join("escape.txt"), root.join("link_file.txt")).unwrap();
	}

	Fixture { _dir: dir, root }
}

fn grep(root: &Path, options: &GrepOptions) -> GrepOutput {
	scan::grep(root, options, &AtomicBool::new(false)).expect("grep")
}

fn hits(out: &GrepOutput) -> Vec<(String, u64)> {
	out.matches
		.iter()
		.map(|m| (m.path.clone(), m.line_number))
		.collect()
}

fn paths(out: &GrepOutput) -> Vec<String> {
	let mut paths: Vec<String> = out.matches.iter().map(|m| m.path.clone()).collect();
	paths.dedup();
	paths
}

#[test]
fn default_search_honors_ignores_and_skips_binary_and_big_files() {
	let f = fixture();
	let out = grep(&f.root, &GrepOptions::new("needle"));

	let mut expected = vec!["crlf.txt", "src/main.rs", "src/nested/deep/lib.rs"];
	if cfg!(unix) {
		expected.insert(0, "caf\u{FFFD}.txt");
	}
	assert_eq!(paths(&out), expected);
	assert_eq!(
		hits(&out)
			.into_iter()
			.filter(|(path, _)| path == "src/nested/deep/lib.rs")
			.map(|(_, line)| line)
			.collect::<Vec<_>>(),
		vec![2, 6]
	);
	// blob.bin holds a NUL, big.txt is over the 1 MiB default.
	assert_eq!(out.files_skipped, 2);
	assert!(!out.truncated);
	assert!(out.next_cursor.is_none());

	let crlf = out.matches.iter().find(|m| m.path == "crlf.txt").unwrap();
	assert_eq!(crlf.line, "needle with crlf");
	if cfg!(unix) {
		let latin = &out.matches[0];
		assert_eq!(latin.line, "a needle in a latin-1 name \u{FFFD}");
	}
}

#[test]
fn symlinks_never_lead_out_of_the_scope() {
	let f = fixture();
	let mut options = GrepOptions::new("outside the scope");
	options.respect_gitignore = false;
	options.include_hidden = true;
	let out = grep(&f.root, &options);
	assert!(out.matches.is_empty(), "{:?}", out.matches);
}

#[test]
fn ignore_files_and_hidden_entries_can_be_searched_on_request() {
	let f = fixture();
	let mut options = GrepOptions::new("needle in an? (ignored|hidden)");
	options.respect_gitignore = false;
	assert_eq!(
		paths(&grep(&f.root, &options)),
		vec!["debug.log", "ignored/secret.txt"]
	);

	options.include_hidden = true;
	assert_eq!(
		paths(&grep(&f.root, &options)),
		vec![".hidden/notes.txt", "debug.log", "ignored/secret.txt"]
	);
}

#[test]
fn the_byte_cap_is_per_file_and_adjustable() {
	let f = fixture();
	let mut options = GrepOptions::new("big file");
	assert!(grep(&f.root, &options).matches.is_empty());
	options.max_file_bytes = Some(BIG_FILE_BYTES as u64);
	assert_eq!(paths(&grep(&f.root, &options)), vec!["big.txt"]);
}

#[test]
fn globs_include_and_exclude() {
	let f = fixture();
	let mut options = GrepOptions::new("needle");
	options.include = vec!["*.rs".into()];
	assert_eq!(
		paths(&grep(&f.root, &options)),
		vec!["src/main.rs", "src/nested/deep/lib.rs"]
	);
	options.exclude = vec!["nested".into()];
	assert_eq!(paths(&grep(&f.root, &options)), vec!["src/main.rs"]);
}

#[test]
fn literal_and_case_options() {
	let f = fixture();
	let mut options = GrepOptions::new("needle(");
	assert!(matches!(
		scan::grep(&f.root, &options, &AtomicBool::new(false)),
		Err(GrepError::Pattern(_))
	));
	options.literal = true;
	assert_eq!(
		hits(&grep(&f.root, &options)),
		vec![("src/nested/deep/lib.rs".into(), 6)]
	);

	let mut options = GrepOptions::new("NEEDLE");
	options.include = vec!["*.rs".into()];
	assert_eq!(hits(&grep(&f.root, &options)).len(), 1);
	options.case_insensitive = true;
	assert_eq!(hits(&grep(&f.root, &options)).len(), 4);
}

#[test]
fn each_match_carries_its_own_context() {
	let f = fixture();
	let mut options = GrepOptions::new("two|five");
	options.context_lines = 1;
	let out = grep(&f.root.join("src/nested/deep/lib.rs"), &options);
	assert_eq!(
		out.matches,
		vec![
			GrepMatch {
				path: "lib.rs".into(),
				line_number: 2,
				line: "// needle two".into(),
				before: vec!["// one".into()],
				after: vec!["// three".into()],
			},
			GrepMatch {
				path: "lib.rs".into(),
				line_number: 5,
				line: "// NEEDLE five".into(),
				before: vec!["// four".into()],
				after: vec!["// needle( six".into()],
			},
		]
	);

	// Adjacent matches share lines without losing any.
	let mut options = GrepOptions::new("two|three");
	options.context_lines = 1;
	let out = grep(&f.root.join("src/nested/deep/lib.rs"), &options);
	assert_eq!(out.matches[0].after, vec!["// three"]);
	assert_eq!(out.matches[1].before, vec!["// needle two"]);
}

#[test]
fn pages_return_every_match_exactly_once() {
	let f = fixture();
	let mut options = GrepOptions::new("needle");
	options.case_insensitive = true;
	options.context_lines = 1;
	let all = grep(&f.root, &options);
	assert!(all.matches.len() >= 6);

	for page_size in [1, 2, 3] {
		let mut paged = Vec::new();
		options.max_results = Some(page_size);
		options.cursor = None;
		loop {
			let page = grep(&f.root, &options);
			assert!(page.matches.len() <= page_size as usize);
			paged.extend(page.matches);
			match page.next_cursor {
				Some(cursor) => {
					assert!(page.truncated);
					options.cursor = Some(cursor);
				}
				None => break,
			}
		}
		assert_eq!(paged, all.matches, "page size {page_size}");
	}
}

#[test]
fn bad_input_is_refused() {
	let f = fixture();
	let mut options = GrepOptions::new("needle");
	options.cursor = Some("not a cursor".into());
	assert!(matches!(
		scan::grep(&f.root, &options, &AtomicBool::new(false)),
		Err(GrepError::Cursor)
	));
	assert!(GrepOptions::new("").validate().is_err());
	let mut options = GrepOptions::new("needle");
	options.max_results = Some(0);
	assert!(options.validate().is_err());
}

#[test]
fn a_cancelled_search_stops() {
	let f = fixture();
	let result = scan::grep(&f.root, &GrepOptions::new("needle"), &AtomicBool::new(true));
	assert!(matches!(result, Err(GrepError::Cancelled)));
}

async fn boot(data_dir: &Path) -> Arc<Core> {
	TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config")
		.save()
		.expect("save config");
	Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"))
}

fn session(library: &Library) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	session
}

async fn query(
	core: &Arc<Core>,
	library: &Arc<Library>,
	scope: SdPath,
) -> Result<GrepOutput, String> {
	GrepQuery::from_input(GrepInput {
		scope,
		options: GrepOptions::new("needle"),
	})
	.map_err(|e| e.to_string())?
	.execute(core.context.clone(), session(library))
	.await
	.map_err(|e| e.to_string())
}

#[tokio::test]
async fn the_query_greps_a_tracked_source_on_this_device_only() {
	let data = TempDir::new().unwrap();
	let core = boot(data.path()).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	let f = fixture();

	let untracked = query(&core, &library, SdPath::local(&f.root)).await;
	assert!(
		untracked
			.as_ref()
			.is_err_and(|e| e.contains("not under a source")),
		"{untracked:?}"
	);

	TrackSourceAction::from_input(TrackSourceInput {
		path: f.root.clone(),
		name: None,
		overrides: Default::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("track");

	let out = query(&core, &library, SdPath::local(f.root.join("src")))
		.await
		.expect("grep a tracked subtree");
	assert_eq!(paths(&out), vec!["main.rs", "nested/deep/lib.rs"]);

	let peer = query(
		&core,
		&library,
		SdPath::Physical {
			device_slug: "another-device".into(),
			path: f.root.clone(),
		},
	)
	.await;
	assert!(
		peer.as_ref()
			.is_err_and(|e| e.contains("not available on this device yet")),
		"{peer:?}"
	);

	#[cfg(unix)]
	{
		let escape = query(&core, &library, SdPath::local(f.root.join("link_dir"))).await;
		assert!(
			escape
				.as_ref()
				.is_err_and(|e| e.contains("outside its source")),
			"{escape:?}"
		);
	}

	core.shutdown().await.ok();
}
