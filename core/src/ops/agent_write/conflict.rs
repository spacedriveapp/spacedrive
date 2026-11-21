//! Names for a create that found its name taken.

use std::path::{Path, PathBuf};

/// The first free `name (agent conflict N).ext` beside `path`, N from 1.
///
/// A file keeps its last extension after the marker so it still opens with
/// the same application; a directory, or a name with no stem before its dot
/// such as `.env`, takes the marker at the end. The name is chosen from what
/// is on disk now, so two calls without a write between them agree.
pub fn conflict_name(path: &Path, is_dir: bool) -> PathBuf {
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let (stem, extension) = split(&name, is_dir);
	(1u32..)
		.map(|n| {
			let candidate = match extension {
				Some(extension) => format!("{stem} (agent conflict {n}).{extension}"),
				None => format!("{stem} (agent conflict {n})"),
			};
			path.with_file_name(candidate)
		})
		.find(|candidate| std::fs::symlink_metadata(candidate).is_err())
		.expect("an unbounded range always finds a free name")
}

fn split(name: &str, is_dir: bool) -> (&str, Option<&str>) {
	if is_dir {
		return (name, None);
	}
	match name.rfind('.') {
		Some(dot) if dot > 0 && dot + 1 < name.len() => (&name[..dot], Some(&name[dot + 1..])),
		_ => (name, None),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn names_count_up_from_one_and_keep_the_extension() {
		let dir = tempfile::tempdir().expect("tempdir");
		let notes = dir.path().join("notes.md");
		assert_eq!(
			conflict_name(&notes, false),
			dir.path().join("notes (agent conflict 1).md")
		);
		std::fs::write(dir.path().join("notes (agent conflict 1).md"), b"x").expect("write");
		assert_eq!(
			conflict_name(&notes, false),
			dir.path().join("notes (agent conflict 2).md")
		);
	}

	#[test]
	fn dotfiles_directories_and_bare_names_take_the_marker_at_the_end() {
		let dir = std::path::Path::new("/nowhere-for-agent-conflicts");
		assert_eq!(
			conflict_name(&dir.join(".env"), false),
			dir.join(".env (agent conflict 1)")
		);
		assert_eq!(
			conflict_name(&dir.join("Makefile"), false),
			dir.join("Makefile (agent conflict 1)")
		);
		assert_eq!(
			conflict_name(&dir.join("v1.2"), true),
			dir.join("v1.2 (agent conflict 1)")
		);
		assert_eq!(
			conflict_name(&dir.join("archive.tar.gz"), false),
			dir.join("archive.tar (agent conflict 1).gz")
		);
	}
}
