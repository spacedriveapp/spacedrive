//! Resolving a requested path inside a granted directory.
//!
//! A string prefix check is not enough to keep an agent inside its grant. A
//! symlink inside the directory can point anywhere, macOS reaches the same
//! directory as `/Users/me` and `/System/Volumes/Data/Users/me` because
//! `realpath` does not resolve firmlinks, and a case-insensitive volume
//! answers `/Users/Me/Granted` for `/Users/me/granted`. So [`resolve`]
//! follows every link with `canonicalize` and then decides containment by
//! filesystem identity: the request is inside when the granted directory is
//! one of its ancestors, compared by device and inode (file index on
//! Windows), the same test `sd_fs_watcher::spelling` uses to recognize two
//! firmlink spellings of one directory. A spelling that reaches the same
//! directory is allowed, a spelling that reaches any other is refused.
//!
//! A path that does not exist yet, the target of a create, resolves through
//! its deepest existing ancestor. A dangling symlink anywhere in it is
//! refused, since writing through it would create the file wherever it
//! points.

use std::{
	ffi::OsString,
	io,
	path::{Component, Path, PathBuf},
};

use super::GrantError;

/// A request resolved inside its grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
	/// The granted directory, canonical.
	pub root: PathBuf,
	/// The request, absolute and spelled under `root`.
	pub path: PathBuf,
	/// `path` relative to `root`.
	pub relative: PathBuf,
}

/// Resolve `requested` inside the granted directory `root`.
///
/// `requested` is relative to `root`, or absolute. Any `..` component is
/// refused outright rather than normalized, since no legitimate client needs
/// one. This does blocking filesystem calls, so async callers run it on the
/// blocking pool.
pub fn resolve(root: &Path, requested: &Path) -> Result<Resolved, GrantError> {
	let meta = std::fs::symlink_metadata(root)
		.map_err(|e| GrantError::RootUnavailable(format!("{}: {e}", root.display())))?;
	if !meta.is_dir() {
		return Err(GrantError::RootUnavailable(format!(
			"{} is no longer a directory",
			root.display()
		)));
	}
	let root = std::fs::canonicalize(root)
		.map_err(|e| GrantError::RootUnavailable(format!("{}: {e}", root.display())))?;

	let candidate = lexical(&root, requested)?;
	let (existing, tail) = split_existing(&candidate)?;
	let inside = within(&root, &existing, same_directory)
		.ok_or_else(|| GrantError::Escape(requested.display().to_string()))?;

	let relative = tail.iter().fold(inside, |path, name| path.join(name));
	Ok(Resolved {
		path: root.join(&relative),
		relative,
		root,
	})
}

/// The path a request names before any link is followed.
fn lexical(root: &Path, requested: &Path) -> Result<PathBuf, GrantError> {
	let absolute = requested.is_absolute();
	for component in requested.components() {
		match component {
			Component::ParentDir => {
				return Err(GrantError::Escape(requested.display().to_string()));
			}
			// A drive-relative `C:foo` or a rooted `\\foo` on Windows is
			// neither relative to the grant nor a full path.
			Component::Prefix(_) | Component::RootDir if !absolute => {
				return Err(GrantError::InvalidPath(requested.display().to_string()));
			}
			_ => {}
		}
	}
	Ok(if absolute {
		requested.to_path_buf()
	} else {
		root.join(requested)
	})
}

/// Split `candidate` into its deepest existing ancestor, canonical, and the
/// names below it that do not exist yet.
fn split_existing(candidate: &Path) -> Result<(PathBuf, Vec<OsString>), GrantError> {
	let mut current = candidate.to_path_buf();
	let mut tail = Vec::new();
	loop {
		match std::fs::canonicalize(&current) {
			Ok(canonical) => {
				tail.reverse();
				return Ok((canonical, tail));
			}
			Err(error) if missing(&error) => {
				// Something is there but cannot be followed: a dangling link.
				if std::fs::symlink_metadata(&current).is_ok() {
					return Err(GrantError::Escape(format!(
						"{} is a link to a missing target",
						current.display()
					)));
				}
				let Some(name) = current.file_name() else {
					return Err(GrantError::InvalidPath(candidate.display().to_string()));
				};
				tail.push(name.to_os_string());
				current.pop();
			}
			Err(error) => {
				return Err(GrantError::InvalidPath(format!(
					"{}: {error}",
					candidate.display()
				)));
			}
		}
	}
}

fn missing(error: &io::Error) -> bool {
	matches!(
		error.kind(),
		io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
	)
}

/// `existing` relative to `root` when `root` is `existing` or one of its
/// ancestors, or `None` when it is outside.
///
/// Both paths are canonical. The prefix test settles the common case without
/// touching the disk; the identity walk settles the spellings a prefix test
/// gets wrong, and `same` is passed in so those spellings can be stood in
/// for on a platform that has none.
pub(crate) fn within(
	root: &Path,
	existing: &Path,
	same: impl Fn(&Path, &Path) -> bool,
) -> Option<PathBuf> {
	if let Ok(rest) = existing.strip_prefix(root) {
		return Some(rest.to_path_buf());
	}
	existing
		.ancestors()
		.find(|ancestor| same(ancestor, root))
		.and_then(|ancestor| existing.strip_prefix(ancestor).ok())
		.map(Path::to_path_buf)
}

fn same_directory(a: &Path, b: &Path) -> bool {
	same_file::is_same_file(a, b).unwrap_or(false)
}

/// The first component of `path` a writable grant may not modify.
///
/// That is `.git` at any depth, which covers everything beneath it, and
/// any `AGENTS.md`, `CLAUDE.md` or `SKILL.md`. Names are compared the way
/// the most permissive filesystem would: case-folded, without the Unicode
/// characters HFS+ ignores, without trailing dots and spaces Windows drops,
/// without an NTFS stream suffix, and including their 8.3 short names.
pub fn protected_component(path: &Path) -> Option<String> {
	path.components().find_map(|component| {
		let Component::Normal(name) = component else {
			return None;
		};
		let folded = fold(&name.to_string_lossy());
		is_protected(&folded).then(|| name.to_string_lossy().into_owned())
	})
}

fn is_protected(folded: &str) -> bool {
	if matches!(
		folded,
		".git" | "git~1" | "agents.md" | "claude.md" | "skill.md"
	) {
		return true;
	}
	["agents~", "claude~"].iter().any(|stem| {
		folded
			.strip_prefix(stem)
			.and_then(|rest| rest.strip_suffix(".md"))
			.is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
	})
}

/// A name as the most permissive filesystem compares it.
pub(crate) fn fold(name: &str) -> String {
	let name = name.split(':').next().unwrap_or(name);
	let kept: String = name.chars().filter(|c| !hfs_ignorable(*c)).collect();
	kept.trim_end_matches(['.', ' ']).to_lowercase()
}

/// Code points HFS+ drops when comparing names, so `.g\u{200c}it` opens
/// `.git`. The list is the one git checks for the same reason.
fn hfs_ignorable(c: char) -> bool {
	matches!(
		c,
		'\u{200c}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{206a}'..='\u{206f}' | '\u{feff}'
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn tree() -> (tempfile::TempDir, PathBuf) {
		let dir = tempfile::tempdir().unwrap();
		let base = std::fs::canonicalize(dir.path()).unwrap();
		std::fs::create_dir_all(base.join("granted/sub")).unwrap();
		std::fs::write(base.join("granted/sub/a.txt"), b"a").unwrap();
		std::fs::create_dir_all(base.join("outside")).unwrap();
		std::fs::write(base.join("outside/secret.txt"), b"s").unwrap();
		std::fs::create_dir_all(base.join("granted-sibling")).unwrap();
		(dir, base)
	}

	#[test]
	fn relative_and_absolute_paths_inside_resolve() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		let resolved = resolve(&root, Path::new("sub/a.txt")).unwrap();
		assert_eq!(resolved.path, root.join("sub/a.txt"));
		assert_eq!(resolved.relative, PathBuf::from("sub/a.txt"));
		let absolute = resolve(&root, &root.join("sub/a.txt")).unwrap();
		assert_eq!(absolute, resolved);
		let itself = resolve(&root, Path::new("")).unwrap();
		assert_eq!(itself.path, root);
		assert_eq!(resolve(&root, Path::new(".")).unwrap().path, root);
	}

	#[test]
	fn a_path_that_does_not_exist_yet_resolves_through_its_parent() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		let resolved = resolve(&root, Path::new("sub/new/deeper.txt")).unwrap();
		assert_eq!(resolved.path, root.join("sub/new/deeper.txt"));
	}

	#[test]
	fn parent_components_are_refused() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		for path in ["../outside/secret.txt", "sub/../../outside", "sub/../a.txt"] {
			assert!(
				matches!(resolve(&root, Path::new(path)), Err(GrantError::Escape(_))),
				"{path}"
			);
		}
		let absolute = root.join("../outside/secret.txt");
		assert!(matches!(
			resolve(&root, &absolute),
			Err(GrantError::Escape(_))
		));
	}

	/// A rooted path with no drive is not absolute on Windows, and is refused
	/// there as invalid rather than as an escape.
	#[test]
	fn a_bare_root_is_refused() {
		let (_dir, base) = tree();
		assert!(resolve(&base.join("granted"), Path::new("/")).is_err());
	}

	#[test]
	fn absolute_paths_outside_are_refused() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		for path in [
			base.join("outside/secret.txt"),
			base.join("granted-sibling"),
			base.join("granted-sibling/new.txt"),
			base.clone(),
			base.ancestors().last().unwrap().to_path_buf(),
		] {
			assert!(
				matches!(resolve(&root, &path), Err(GrantError::Escape(_))),
				"{}",
				path.display()
			);
		}
	}

	#[cfg(unix)]
	#[test]
	fn symlinks_resolve_to_where_they_point() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		std::os::unix::fs::symlink(base.join("outside"), root.join("out")).unwrap();
		std::os::unix::fs::symlink(base.join("outside/secret.txt"), root.join("s.txt")).unwrap();
		std::os::unix::fs::symlink(root.join("sub"), root.join("in")).unwrap();
		std::os::unix::fs::symlink(base.join("nowhere"), root.join("dangling")).unwrap();

		for path in [
			"out",
			"out/secret.txt",
			"out/new.txt",
			"s.txt",
			"dangling",
			"dangling/x",
		] {
			assert!(
				matches!(resolve(&root, Path::new(path)), Err(GrantError::Escape(_))),
				"{path}"
			);
		}
		let inside = resolve(&root, Path::new("in/a.txt")).unwrap();
		assert_eq!(inside.path, root.join("sub/a.txt"));
	}

	#[cfg(unix)]
	#[test]
	fn another_spelling_of_the_root_is_the_root() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		let alias = base.join("alias");
		std::os::unix::fs::symlink(&root, &alias).unwrap();
		let resolved = resolve(&root, &alias.join("sub/a.txt")).unwrap();
		assert_eq!(resolved.path, root.join("sub/a.txt"));
	}

	#[cfg(unix)]
	#[test]
	fn a_root_replaced_by_a_link_is_unavailable() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		std::fs::rename(&root, base.join("moved")).unwrap();
		std::os::unix::fs::symlink(base.join("outside"), &root).unwrap();
		assert!(matches!(
			resolve(&root, Path::new("secret.txt")),
			Err(GrantError::RootUnavailable(_))
		));
	}

	#[test]
	fn a_missing_root_is_unavailable() {
		let (_dir, base) = tree();
		assert!(matches!(
			resolve(&base.join("gone"), Path::new("a")),
			Err(GrantError::RootUnavailable(_))
		));
	}

	/// The firmlink and case-alias cases, with identity stood in for: macOS
	/// spells one directory two ways, and a case-insensitive volume answers
	/// any case of a name.
	#[test]
	fn containment_follows_identity_not_spelling() {
		let root = Path::new("/System/Volumes/Data/Users/me/granted");
		let firmlink = |a: &Path, b: &Path| {
			let short = |p: &Path| {
				p.strip_prefix("/System/Volumes/Data")
					.map(|rest| Path::new("/").join(rest))
					.unwrap_or_else(|_| p.to_path_buf())
			};
			short(a) == short(b)
		};
		assert_eq!(
			within(root, Path::new("/Users/me/granted/a.txt"), firmlink),
			Some(PathBuf::from("a.txt"))
		);
		assert_eq!(
			within(root, Path::new("/Users/me/other/a.txt"), firmlink),
			None
		);
		assert_eq!(within(root, Path::new("/Users/me"), firmlink), None);

		let case_insensitive = |a: &Path, b: &Path| {
			a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
		};
		let root = Path::new("/Volumes/Work/Granted");
		assert_eq!(
			within(root, Path::new("/volumes/work/GRANTED/x"), case_insensitive),
			Some(PathBuf::from("x"))
		);
		assert_eq!(
			within(
				root,
				Path::new("/volumes/work/granted-other/x"),
				case_insensitive
			),
			None
		);
		let never = |_: &Path, _: &Path| false;
		assert_eq!(
			within(root, Path::new("/Volumes/Work/GrantedX"), never),
			None
		);
	}

	/// On a case-sensitive volume another case of the root's name is another
	/// directory, so it is outside.
	#[cfg(target_os = "linux")]
	#[test]
	fn another_case_of_the_root_is_another_directory_here() {
		let (_dir, base) = tree();
		let root = base.join("granted");
		std::fs::create_dir_all(base.join("GRANTED")).unwrap();
		assert!(matches!(
			resolve(&root, &base.join("GRANTED/x")),
			Err(GrantError::Escape(_))
		));
		assert!(matches!(
			resolve(&root, &base.join("Granted/x")),
			Err(GrantError::Escape(_))
		));
	}

	#[test]
	fn protected_names_in_every_spelling() {
		for path in [
			"AGENTS.md",
			"docs/agents.md",
			"a/b/c/CLAUDE.md",
			"skills/x/SKILL.md",
			"Skill.MD",
			".git",
			".git/config",
			"sub/.GIT/hooks/pre-commit",
			".g\u{200c}it/config",
			"AGENTS.md.",
			"AGENTS.md ",
			"AGENTS.md:stream",
			"GIT~1/config",
			"AGENTS~1.MD",
			"CLAUDE~12.md",
		] {
			assert!(protected_component(Path::new(path)).is_some(), "{path:?}");
		}
		for path in [
			"agents.mdx",
			"MY_AGENTS.md",
			"docs/readme.md",
			".gitignore",
			".github/workflows/ci.yml",
			"git/config",
			"AGENTS~.md",
			"AGENTS~x.md",
		] {
			assert!(protected_component(Path::new(path)).is_none(), "{path:?}");
		}
	}
}
