use crate::library::Library;

use super::TagCreateArgs;

/// Seeds tags in a new library.
/// Shouldn't be called more than once!
pub async fn new_library(library: &Library) -> Result<(), sd_core_sync::Error> {
	// remove type after tags are added

	let tags = [
		TagCreateArgs {
			name: "Keepsafe".to_string(),
			color: "#D9188E".to_string(),
		},
		TagCreateArgs {
			name: "Hidden".to_string(),
			color: "#646278".to_string(),
		},
		TagCreateArgs {
			name: "Projects".to_string(),
			color: "#42D097".to_string(),
		},
		TagCreateArgs {
			name: "Memes".to_string(),
			color: "#A718D9".to_string(),
		},
	];

	for tag in tags {
		tag.exec(library).await?;
	}

	Ok(())
}
