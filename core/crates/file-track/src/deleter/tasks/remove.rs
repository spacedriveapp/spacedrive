use std::path::Path;

use super::super::DeleteBehavior;

#[derive(Debug, Hash)]
pub struct RemoveBehavior;

impl DeleteBehavior for RemoveBehavior {
	async fn delete(file_path: &Path) -> Result<(), ()> {
		if file_path.is_dir() {
			tokio::fs::remove_dir_all(&file_path).await
		} else {
			tokio::fs::remove_file(&file_path).await
		};
		Ok(())
	}
}
