//! Filtering logic for the indexer
//! 
//! This module provides hardcoded filtering rules that will eventually
//! be replaced by the full indexer rules system.

use std::path::Path;

/// Common directories to skip during indexing
const SKIP_DIRECTORIES: &[&str] = &[
    // Development
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".tox",
    ".nox",
    ".coverage",
    ".hypothesis",
    
    // IDEs
    ".idea",
    ".vscode",
    ".vs",
    
    // OS specific
    "$RECYCLE.BIN",
    "System Volume Information",
    ".Trash",
    ".Trash-1000",
    
    // Package managers
    ".npm",
    ".yarn",
    ".pnpm-store",
    "bower_components",
    ".cargo",
    ".rustup",
    ".gradle",
    ".m2",
    
    // Cache directories
    ".cache",
    "Cache",
    "Caches",
    "CachedData",
    "Code Cache",
    
    // Temporary
    "tmp",
    "temp",
    ".tmp",
    ".temp",
];

/// Common files to skip during indexing
const SKIP_FILES: &[&str] = &[
    // OS specific
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    ".directory",
    ".Spotlight-V100",
    ".Trashes",
    ".fseventsd",
    ".TemporaryItems",
    "ehthumbs.db",
    "ehthumbs_vista.db",
    
    // Editor
    "*.swp",
    "*.swo",
    "*~",
    ".*.swp",
    ".*.swo",
    
    // Logs
    "*.log",
    "*.log.*",
    
    // Lock files
    "*.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "Gemfile.lock",
    "poetry.lock",
    "composer.lock",
];

/// File extensions to skip (without the dot)
const SKIP_EXTENSIONS: &[&str] = &[
    // Temporary
    "tmp",
    "temp",
    "cache",
    "bak",
    "backup",
    "old",
    
    // System
    "sys",
    "dll",
    "so",
    "dylib",
];

/// Determines if a path should be skipped during indexing
/// 
/// This is a temporary implementation that will be replaced by the
/// full indexer rules system. The rules system will allow users to
/// customize these patterns per location.
/// 
/// TODO: Replace with IndexerRuleEngine when rules system is implemented
pub fn should_skip_path(path: &Path) -> bool {
    // Get the file/directory name
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    
    // Skip hidden files/directories (Unix-style)
    if name.starts_with('.') && name != "." && name != ".." {
        // Allow .config and .local as they often contain user data
        if path.is_dir() && (name == ".config" || name == ".local") {
            return false;
        }
        return true;
    }
    
    // Check if it's a directory to skip
    if path.is_dir() && SKIP_DIRECTORIES.contains(&name) {
        return true;
    }
    
    // Check if it's a file to skip
    if path.is_file() {
        // Check exact filename matches
        if SKIP_FILES.contains(&name) {
            return true;
        }
        
        // Check patterns that end with wildcards
        for pattern in SKIP_FILES {
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if name.starts_with(prefix) {
                    return true;
                }
            } else if pattern.starts_with('*') {
                let suffix = &pattern[1..];
                if name.ends_with(suffix) {
                    return true;
                }
            }
        }
        
        // Check extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if SKIP_EXTENSIONS.contains(&ext) {
                return true;
            }
        }
    }
    
    // Check file size (skip files over 4GB for now)
    if path.is_file() {
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.len() > 4 * 1024 * 1024 * 1024 {
                return true;
            }
        }
    }
    
    false
}

/// Additional filtering context that can be used for more sophisticated filtering
pub struct FilterContext {
    pub is_git_repo: bool,
    pub has_gitignore: bool,
    pub parent_ignored: bool,
}

impl Default for FilterContext {
    fn default() -> Self {
        Self {
            is_git_repo: false,
            has_gitignore: false,
            parent_ignored: false,
        }
    }
}

/// Future integration point for the rules engine
/// 
/// When the rules system is implemented, this function will delegate to it:
/// ```ignore
/// pub fn should_skip_path_with_rules(
///     path: &Path,
///     rule_engine: &IndexerRuleEngine,
///     context: &FilterContext,
/// ) -> RuleDecision {
///     rule_engine.evaluate(path, context)
/// }
/// ```

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_skip_hidden_files() {
        assert!(should_skip_path(Path::new(".hidden")));
        assert!(should_skip_path(Path::new(".DS_Store")));
        assert!(!should_skip_path(Path::new("normal_file.txt")));
    }
    
    #[test]
    fn test_skip_directories() {
        assert!(should_skip_path(Path::new("node_modules")));
        assert!(should_skip_path(Path::new("target")));
        assert!(should_skip_path(Path::new(".git")));
        assert!(!should_skip_path(Path::new("src")));
    }
    
    #[test]
    fn test_skip_system_files() {
        assert!(should_skip_path(Path::new("Thumbs.db")));
        assert!(should_skip_path(Path::new("desktop.ini")));
    }
    
    #[test]
    fn test_allow_config_dirs() {
        let config = PathBuf::from(".config");
        assert!(!should_skip_path(&config));
        
        let local = PathBuf::from(".local");
        assert!(!should_skip_path(&local));
    }
}