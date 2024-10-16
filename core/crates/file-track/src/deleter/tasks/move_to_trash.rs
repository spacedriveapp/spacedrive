use super::super::{DeleteBehavior, FileData};

#[derive(Debug, Hash)]
pub struct MoveToTrashBehavior;

impl DeleteBehavior for MoveToTrashBehavior {
	async fn delete(file: FileData) -> Result<(), ()> {
		trash::delete(file.full_path).unwrap();
		Ok(())
	}
}
