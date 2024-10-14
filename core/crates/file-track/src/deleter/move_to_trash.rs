use super::{DeleteBehavior, FileData};

#[derive(Debug, Hash)]
pub struct MoveToTrashBehavior;

impl DeleteBehavior for MoveToTrashBehavior {
	async fn delete(file: FileData) -> Result<(), ()> {
		tracing::debug!(?file.full_path, "MOVE TO TRASH ---");
		trash::delete(file.full_path).unwrap();
		Ok(())
	}
}
