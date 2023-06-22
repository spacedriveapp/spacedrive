use super::TagCreateArgs;
use crate::library::Library;

/// Seeds tags in a new library.
/// Shouldn't be called more than once!
pub async fn new_library(library: &Library) -> prisma_client_rust::Result<()> {
	// remove type after tags are added
	let tags: [TagCreateArgs; 0] = [];

	for tag in tags {
		tag.exec(&library).await?;
	}

	Ok(())
}
