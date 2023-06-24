use chrono::Utc;

use super::TagCreateArgs;
use crate::library::Library;

/// Seeds tags in a new library.
/// Shouldn't be called more than once!
pub async fn new_library(library: &Library) -> prisma_client_rust::Result<()> {
	// remove type after tags are added
	let now = Utc::now().into();

	let tags = [
		TagCreateArgs {
			name: "Keepsafe".to_string(),
			color: "#D9188E".to_string(),
			date_created: now,
		},
		TagCreateArgs {
			name: "Hidden".to_string(),
			color: "#646278".to_string(),
			date_created: now,
		},
		TagCreateArgs {
			name: "Projects".to_string(),
			color: "#42D097".to_string(),
			date_created: now,
		},
		TagCreateArgs {
			name: "Memes".to_string(),
			color: "#A718D9".to_string(),
			date_created: now,
		},
	];

	for tag in tags {
		tag.exec(library).await?;
	}

	Ok(())
}
