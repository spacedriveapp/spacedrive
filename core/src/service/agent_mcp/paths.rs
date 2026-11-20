//! # Agent MCP path rules
//!
//! Path rules for what an agent sends and what the endpoint returns.
//!
//! The grant check already refuses `..` and anything that resolves outside
//! the granted folder. These rules refuse the spellings no client needs, so
//! the only paths that reach the filesystem are plain relative ones. Paths
//! going back to the agent are relative to the grant root with `/`
//! separators, so no absolute host path leaves the daemon.

use std::path::{Component, Path};

/// Check a path an agent sent. `""` and `"."` name the grant root, and one
/// trailing `/` is allowed.
pub fn check(path: &str) -> Result<(), &'static str> {
	let path = path.strip_suffix('/').unwrap_or(path);
	if path.is_empty() || path == "." {
		return Ok(());
	}
	if path.contains('\0') {
		return Err("paths cannot contain NUL");
	}
	if path.starts_with('/') || path.starts_with('\\') {
		return Err("paths are relative to the grant root; drop the leading slash");
	}
	for component in path.split('/') {
		match component {
			"" => return Err("paths cannot contain empty components"),
			"." | ".." => return Err("paths cannot contain '.' or '..' components"),
			_ if component.len() > 255 => return Err("a path component is longer than 255 bytes"),
			_ => {}
		}
		#[cfg(windows)]
		windows_component(component)?;
	}
	Ok(())
}

#[cfg(windows)]
fn windows_component(component: &str) -> Result<(), &'static str> {
	if component.contains('\\') || component.contains(':') {
		return Err("paths cannot contain '\\' or ':'");
	}
	if component.ends_with('.') || component.ends_with(' ') {
		return Err("path components cannot end in a dot or a space");
	}
	let stem = component
		.split('.')
		.next()
		.unwrap_or("")
		.to_ascii_uppercase();
	let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
		|| (stem.len() == 4
			&& (stem.starts_with("COM") || stem.starts_with("LPT"))
			&& stem.as_bytes()[3].is_ascii_digit());
	if reserved {
		return Err("paths cannot name a Windows device");
	}
	Ok(())
}

/// `relative` as the agent sees it: `/` separators, lossy UTF-8, `""` for the
/// root.
pub fn shown(relative: &Path) -> String {
	relative
		.components()
		.filter_map(|component| match component {
			Component::Normal(name) => Some(name.to_string_lossy()),
			_ => None,
		})
		.collect::<Vec<_>>()
		.join("/")
}

/// `child`, a `/`-separated path below `base`, as the agent sees it.
pub fn joined(base: &Path, child: &str) -> String {
	let base = shown(base);
	match (base.is_empty(), child.is_empty()) {
		(true, _) => child.to_string(),
		(false, true) => base,
		(false, false) => format!("{base}/{child}"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn plain_relative_paths_pass() {
		for path in ["", ".", "docs", "docs/", "docs/readme.md", ".hidden/file"] {
			assert!(check(path).is_ok(), "{path:?}");
		}
	}

	#[test]
	fn odd_spellings_are_refused() {
		let long = "a".repeat(256);
		for path in [
			"/etc/passwd",
			"\\server\\share",
			"../up",
			"docs/../../up",
			"./docs",
			"docs//readme.md",
			"nul\0byte",
			long.as_str(),
		] {
			assert!(check(path).is_err(), "{path:?}");
		}
	}

	#[test]
	fn shown_paths_use_forward_slashes() {
		assert_eq!(shown(Path::new("")), "");
		assert_eq!(shown(Path::new("a/b")), "a/b");
		assert_eq!(joined(Path::new(""), "x"), "x");
		assert_eq!(joined(Path::new("a"), ""), "a");
		assert_eq!(joined(Path::new("a"), "x/y"), "a/x/y");
	}
}
