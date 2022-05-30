use anyhow::Result;
use uuid::Uuid;

use crate::node::{get_nodestate, LibraryState};
use crate::prisma::library;
use crate::util::db::run_migrations;
use crate::CoreContext;

pub static LIBRARY_DB_NAME: &str = "library.db";
pub static DEFAULT_NAME: &str = "My Library";

pub fn get_library_path(data_path: &str) -> String {
	let path = data_path.to_owned();
	format!("{}/{}", path, LIBRARY_DB_NAME)
}

// pub async fn get(core: &Node) -> Result<library::Data, LibraryError> {
// 	let config = get_nodestate();
// 	let db = &core.database;

// 	let library_state = config.get_current_library();

// 	println!("{:?}", library_state);

// 	// get library from db
// 	let library = match db
// 		.library()
// 		.find_unique(library::pub_id::equals(library_state.library_uuid.clone()))
// 		.exec()
// 		.await?
// 	{
// 		Some(library) => Ok(library),
// 		None => {
// 			// update config library state to offline
// 			// config.libraries

// 			Err(anyhow::anyhow!("library_not_found"))
// 		}
// 	};

// 	Ok(library.unwrap())
// }

pub async fn load(ctx: &CoreContext, library_path: &str, library_id: &str) -> Result<()> {
	let mut config = get_nodestate();

	println!("Initializing library: {} {}", &library_id, library_path);

	if config.current_library_uuid != library_id {
		config.current_library_uuid = library_id.to_string();
		config.save();
	}
	// create connection with library database & run migrations
	run_migrations(&ctx).await?;
	// if doesn't exist, mark as offline
	Ok(())
}

pub async fn create(ctx: &CoreContext, name: Option<String>) -> Result<()> {
	let mut config = get_nodestate();

	let uuid = Uuid::new_v4().to_string();

	println!("Creating library {:?}, UUID: {:?}", name, uuid);

	let library_state = LibraryState {
		library_uuid: uuid.clone(),
		library_path: get_library_path(&config.data_path),
		..LibraryState::default()
	};

	run_migrations(&ctx).await?;

	config.libraries.push(library_state);

	config.current_library_uuid = uuid;

	config.save();

	let db = &ctx.database;

	let _library = db
		.library()
		.create(
			library::pub_id::set(config.current_library_uuid),
			library::name::set(name.unwrap_or(DEFAULT_NAME.into())),
			vec![],
		)
		.exec()
		.await;

	println!("library created in database: {:?}", _library);

	Ok(())
}
