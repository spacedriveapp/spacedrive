use std::path::Path;
use tokio::time::Duration;

pub(crate) const PROGRESS_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

/// Check if two paths are on the same filesystem
pub(crate) fn is_same_filesystem(source: impl AsRef<Path>, target: impl AsRef<Path>) -> bool {
    // TODO: Implement proper filesystem check
    // For now, just check if they're on the same drive/mount point
    let source = source.as_ref().components().next();
    let target = target.as_ref().components().next();
    source == target
}
