//! # Protected paths
//!
//! A writable grant may not modify some paths even inside the directory it
//! lends. Three lists decide which. The built-in names in
//! [`scope::protected_component`] (`.git`, `AGENTS.md`, `CLAUDE.md`,
//! `SKILL.md`) hold everywhere. [`DEFAULT_PATTERNS`] adds the files other
//! agent tools read as instructions or run as code, also everywhere. Each
//! space adds its own patterns, which hold inside it and inside any space
//! nested in it, so a nested space can add protection but never lift it.
//!
//! A pattern is a glob over `/`-separated paths. One that starts with `/` is
//! anchored at the space root; any other matches at every depth, so `.env*`
//! covers `app/.env.local`. A match covers everything beneath it, and
//! `dir/**` also covers `dir` itself, so the folder cannot be renamed away.
//! Names are folded the way [`scope::protected_component`] folds them before
//! they are compared, and patterns are lowercased, so case and the spellings
//! a permissive filesystem ignores do not get around a pattern.

use std::{
	path::{Component, Path, PathBuf},
	sync::{Arc, LazyLock},
};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use uuid::Uuid;

use super::scope;
use crate::context::CoreContext;

/// Patterns protected everywhere, beside the built-in names.
pub const DEFAULT_PATTERNS: &[&str] = &[
	"GEMINI.md",
	"AGENT.md",
	".cursor/rules/**",
	".github/workflows/**",
	".env*",
];

/// The most patterns one space may carry.
pub const MAX_PATTERNS: usize = 100;

static DEFAULTS: LazyLock<GlobSet> =
	LazyLock::new(|| compile(DEFAULT_PATTERNS).expect("the default patterns compile"));

/// Compile protected patterns into one set, refusing an empty or invalid
/// pattern with a message naming it.
pub fn compile<S: AsRef<str>>(patterns: &[S]) -> Result<GlobSet, String> {
	if patterns.len() > MAX_PATTERNS {
		return Err(format!(
			"a space can protect at most {MAX_PATTERNS} patterns"
		));
	}
	let mut set = GlobSetBuilder::new();
	for pattern in patterns {
		let pattern = pattern.as_ref().trim();
		let lowered = pattern.to_lowercase();
		let body = lowered.trim_end_matches('/');
		let glob = match body.strip_prefix('/') {
			Some(anchored) => anchored.to_string(),
			None => format!("**/{body}"),
		};
		if glob.is_empty() || glob == "**/" {
			return Err(format!("{pattern:?} is not a protected pattern"));
		}
		let folder = glob.strip_suffix("/**").map(str::to_string);
		for glob in std::iter::once(glob).chain(folder) {
			set.add(
				GlobBuilder::new(&glob)
					.literal_separator(true)
					.build()
					.map_err(|e| format!("{pattern:?} is not a protected pattern: {e}"))?,
			);
		}
	}
	set.build().map_err(|e| e.to_string())
}

/// The protection that applies at one path and below it.
#[derive(Clone)]
pub struct Protection {
	path: PathBuf,
	/// The path as its volume spells it, when that differs, since a source
	/// and so its spaces are rooted at the volume's spelling.
	spelled: Option<PathBuf>,
	/// Every space of the library that protects anything: its folder and
	/// its patterns.
	spaces: Vec<(PathBuf, Arc<GlobSet>)>,
}

impl Protection {
	/// The built-in names and default patterns alone, for a spelling that
	/// is not resolved to a space, such as the path a request was sent with.
	pub fn builtin(path: &Path) -> Self {
		Self {
			path: path.to_path_buf(),
			spelled: None,
			spaces: Vec::new(),
		}
	}

	/// Everything that protects `path`, which must be absolute and resolved,
	/// and anything below it. Every space of `library` is considered rather
	/// than only those holding `path`, so a scan below `path` also meets the
	/// spaces nested inside it, whichever source holds them.
	pub async fn of(context: &CoreContext, path: &Path, library: Uuid) -> Self {
		let spelled = context
			.volume_manager
			.locate_path(path)
			.await
			.map(|(_, spelled)| spelled)
			.filter(|spelled| spelled != path);
		Self {
			path: path.to_path_buf(),
			spelled,
			spaces: crate::ops::spaces::events::protected_spaces(context, library).await,
		}
	}

	/// The first protected name in the path, if any.
	pub fn find(&self) -> Option<String> {
		self.find_below(Path::new(""))
	}

	/// The first protected name in `below` under the path, or in the path
	/// itself.
	pub fn find_below(&self, below: &Path) -> Option<String> {
		let full = self.path.join(below);
		if let Some(name) = scope::protected_component(&full) {
			return Some(name);
		}
		if let Some(name) = first_match(&DEFAULTS, &names(&full)) {
			return Some(name);
		}
		let spelled = self.spelled.as_ref().map(|spelled| spelled.join(below));
		self.spaces.iter().find_map(|(dir, set)| {
			let inside = std::iter::once(&full)
				.chain(spelled.as_ref())
				.find_map(|path| path.strip_prefix(dir).ok())?;
			first_match(set, &names(inside))
		})
	}
}

/// Each name of `path`, folded and as spelled.
fn names(path: &Path) -> Vec<(String, String)> {
	path.components()
		.filter_map(|component| match component {
			Component::Normal(name) => {
				let name = name.to_string_lossy();
				Some((scope::fold(&name), name.into_owned()))
			}
			_ => None,
		})
		.collect()
}

/// The name ending the shortest prefix of `names` that `set` matches.
fn first_match(set: &GlobSet, names: &[(String, String)]) -> Option<String> {
	let mut prefix = String::new();
	for (folded, shown) in names {
		if !prefix.is_empty() {
			prefix.push('/');
		}
		prefix.push_str(folded);
		if set.is_match(&prefix) {
			return Some(shown.clone());
		}
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	fn at(path: &str, spaces: &[(&str, &[&str])]) -> Protection {
		Protection {
			path: PathBuf::from(path),
			spelled: None,
			spaces: spaces
				.iter()
				.map(|(dir, patterns)| (PathBuf::from(dir), Arc::new(compile(patterns).unwrap())))
				.collect(),
		}
	}

	#[test]
	fn default_patterns_hold_at_any_depth() {
		for path in [
			"/w/GEMINI.md",
			"/w/sub/agent.md",
			"/w/.env",
			"/w/app/.ENV.local",
			"/w/.github/workflows/ci.yml",
			"/w/repo/.github/workflows",
			"/w/.cursor/rules/style.mdc",
			"/w/.Cursor/Rules/a/b.mdc",
			"/w/AGENTS.md",
		] {
			assert!(
				Protection::builtin(Path::new(path)).find().is_some(),
				"{path}"
			);
		}
		for path in [
			"/w/.github/dependabot.yml",
			"/w/.cursor/settings.json",
			"/w/environment.md",
			"/w/my.env",
			"/w/AGENTS.mdx",
		] {
			assert!(
				Protection::builtin(Path::new(path)).find().is_none(),
				"{path}"
			);
		}
	}

	#[test]
	fn space_patterns_hold_inside_the_space() {
		let patterns: &[&str] = &["*.pem", "/secrets/**", "/plan.md"];
		assert_eq!(
			at("/w/notes/plan.md", &[("/w/notes", patterns)]).find(),
			Some("plan.md".into())
		);
		let root = at("/w", &[("/w", patterns)]);
		assert_eq!(
			root.find_below(Path::new("keys/ID.PEM")),
			Some("ID.PEM".into())
		);
		assert_eq!(
			root.find_below(Path::new("secrets")),
			Some("secrets".into())
		);
		assert_eq!(
			root.find_below(Path::new("Secrets/a/b")),
			Some("Secrets".into())
		);
		assert!(
			root.find_below(Path::new("docs/secrets/a")).is_none(),
			"anchored"
		);
		assert!(
			root.find_below(Path::new("docs/plan.md")).is_none(),
			"anchored"
		);
		assert!(root.find_below(Path::new("docs/readme.md")).is_none());
		assert!(
			at("/elsewhere/key.pem", &[("/w", patterns)])
				.find()
				.is_none(),
			"a space protects only inside itself"
		);

		let nested: &[(&str, &[&str])] = &[("/w", &["/inner/**"]), ("/w/inner", &[])];
		assert!(
			at("/w/inner/x.txt", nested).find().is_some(),
			"an outer space protects inside an inner one"
		);
		let below: &[(&str, &[&str])] = &[("/w/a/project", &["*.pem"])];
		assert_eq!(
			at("/w", below).find_below(Path::new("a/project/key.pem")),
			Some("key.pem".into()),
			"a scan from above meets the spaces nested below"
		);
	}

	#[test]
	fn bad_patterns_are_refused() {
		assert!(compile(&["  "]).is_err());
		assert!(compile(&["/"]).is_err());
		assert!(compile(&["a[b"]).is_err());
		assert!(compile(&vec!["x"; MAX_PATTERNS + 1]).is_err());
		assert!(compile(&["secrets/", "/build/**", "*.key"]).is_ok());
	}
}
