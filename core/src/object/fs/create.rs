use crate::{
	library::LibraryContext,
	location::{check_virtual_path_exists, fetch_location, LocationError},
};

use super::error::VirtualFSError;
use crate::prisma::{file_path, location};

// TODO: we should create an action handler for all FS operations, that can work for both local and remote locations
// if the location is remote, we queue a job for that client specifically
// the actual create_folder function should be an option on an enum for all vfs actions
pub async fn create_folder(
	location_id: i32,
	path: &str,
	name: Option<&str>,
	library_ctx: &LibraryContext,
) -> Result<(), VirtualFSError> {
	// let location = fetch_location(library_ctx, location_id)
	// 	.exec()
	// 	.await?
	// 	.ok_or(LocationError::IdNotFound(location_id))?;

	// let name = name.unwrap_or("Untitled Folder");

	// let exists = check_virtual_path_exists(library_ctx, location_id, subpath).await?;

	// std::fs::create_dir_all(&obj_path)?;

	Ok(())
}
