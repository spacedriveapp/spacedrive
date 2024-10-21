use crate::deleter::FileData;

use super::super::DeleteBehavior;

#[derive(Debug, Hash)]
pub struct RemoveBehavior;

impl DeleteBehavior for RemoveBehavior {
	async fn delete(file_data: FileData) -> Result<(), ()> {
		if file_data.full_path.is_dir() {
			tokio::fs::remove_dir_all(&file_data.full_path).await
		} else {
			tokio::fs::remove_file(&file_data.full_path).await
		};
		Ok(())
	}
}
