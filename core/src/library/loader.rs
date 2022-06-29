use log::info;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{
	node::{get_nodestate, LibraryState},
	prisma::library,
	util::db::{run_migrations, DatabaseError},
	CoreContext,
};

pub static LIBRARY_DB_NAME: &str = "library.db";
pub static DEFAULT_NAME: &str = "My Library";

pub fn get_library_path(data_path: impl AsRef<Path>) -> PathBuf {
	data_path.as_ref().join(LIBRARY_DB_NAME)
}

// pub async fn get(core: &Node) -> Result<library::Data, LibraryError> {
// 	let config = get_nodestate();
// 	let db = &core.database;

// 	let library_state = config.get_current_library();

// 	info!("{:?}", library_state);

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

pub async fn load(
	ctx: &CoreContext,
	library_path: impl AsRef<Path> + Debug,
	library_id: &str,
) -> Result<(), DatabaseError> {
	let mut config = get_nodestate();

	info!("Initializing library: {} {:#?}", &library_id, library_path);

	if config.current_library_uuid != library_id {
		config.current_library_uuid = library_id.to_string();
		config.save().await;
	}
	// create connection with library database & run migrations
	run_migrations(ctx).await?;
	// if doesn't exist, mark as offline
	Ok(())
}

pub async fn create(ctx: &CoreContext, name: Option<String>) -> Result<(), ()> {
	let mut config = get_nodestate();

	let uuid = Uuid::new_v4().to_string();

	info!("Creating library {:?}, UUID: {:?}", name, uuid);

	let library_state = LibraryState {
		library_uuid: uuid.clone(),
		library_path: get_library_path(config.data_path.as_ref().unwrap()),
		..LibraryState::default()
	};

	run_migrations(ctx).await.unwrap();

	config.libraries.push(library_state);

	config.current_library_uuid = uuid;

	config.save().await;

	let library = ctx
		.database
		.library()
		.create(
			library::pub_id::set(config.current_library_uuid),
			library::name::set(name.unwrap_or_else(|| DEFAULT_NAME.into())),
			vec![],
		)
		.exec()
		.await
		.unwrap();

	info!("library created in database: {:?}", library);

	Ok(())
}
