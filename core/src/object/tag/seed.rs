use super::TagCreateArgs;
use crate::library::Library;
use crate::prisma::tag;

/// Seeds tags in a new library.
/// Shouldn't be called more than once!
pub async fn new_library(library: &Library) -> prisma_client_rust::Result<()> {
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

	for tag in tags.clone() {
		tag.exec(library).await?;
	}

	// SAFETY: THIS IS SAFE AS IT IS ONLY DONE ONCE DURING LIBRARY CREATION, SO THE COLOURS SHOULD NEVER CLASH

	library
		.db
		.tag()
		.update_many(
			tags.iter()
				.map(|x| tag::color::equals(Some(x.color.clone())))
				.collect(),
			vec![tag::date_created::set(Some(Utc::now().into()))],
		)
		.exec()
		.await?;

	Ok(())
}
