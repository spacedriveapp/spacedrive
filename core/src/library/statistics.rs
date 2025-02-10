use crate::{api::utils::get_size, invalidate_query, library::Library, Node};

use sd_prisma::prisma::{statistics, volume};
use sd_utils::db::size_in_bytes_from_db;

use chrono::Utc;
use tracing::{debug, error, info};

use super::LibraryManagerError;

pub async fn update_library_statistics(
	node: &Node,
	library: &Library,
) -> Result<statistics::Data, LibraryManagerError> {
	let (mut total_capacity, mut available_capacity) = library
		.db
		.volume()
		.find_many(vec![])
		.select(volume::select!({ total_bytes_capacity total_bytes_available }))
		.exec()
		.await?
		.into_iter()
		.fold((0, 0), |(mut total, mut available), vol| {
			total += vol
				.total_bytes_capacity
				.map(|c| c.parse::<u64>().unwrap_or(0))
				.unwrap_or(0);
			available += vol
				.total_bytes_available
				.map(|a| a.parse::<u64>().unwrap_or(0))
				.unwrap_or(0);
			(total, available)
		});
	debug!(
		?total_capacity,
		?available_capacity,
		"Fetched volume statistics;"
	);

	if total_capacity == 0 && available_capacity == 0 {
		// Failed to fetch volume statistics from database, so we compute from local volumes
		#[cfg(any(target_os = "linux", target_os = "macos"))]
		let volumes = crate::volume::get_volumes().await?;

		#[cfg(target_os = "windows")]
		let volumes = crate::volume::get_volumes().await;

		let mut local_total_capacity: u64 = 0;
		let mut local_available_capacity: u64 = 0;
		for volume in volumes {
			local_total_capacity += volume.total_bytes_capacity;
			local_available_capacity += volume.total_bytes_available;
		}

		total_capacity = local_total_capacity;
		available_capacity = local_available_capacity;
	}

	let total_bytes_used = total_capacity - available_capacity;

	let library_db_size = get_size(
		node.config
			.data_directory()
			.join("libraries")
			.join(format!("{}.db", library.id)),
	)
	.await
	.unwrap_or(0);

	let total_library_bytes = library
		.db
		.location()
		.find_many(vec![])
		.exec()
		.await
		.unwrap_or_else(|e| {
			error!(?e, "Failed to get locations;");
			vec![]
		})
		.into_iter()
		.map(|location| {
			location
				.size_in_bytes
				.map(|size| size_in_bytes_from_db(&size))
				.unwrap_or(0)
		})
		.sum::<u64>();

	let thumbnail_folder_size = get_size(node.config.data_directory().join("thumbnails"))
		.await
		.unwrap_or(0);

	use statistics::*;
	let params = vec![
		id::set(1), // Each library is a database so only one of these ever exists
		date_captured::set(Utc::now().into()),
		total_object_count::set(0),
		library_db_size::set(library_db_size.to_string()),
		total_library_bytes::set(total_library_bytes.to_string()),
		total_local_bytes_used::set(total_bytes_used.to_string()),
		total_local_bytes_capacity::set(total_capacity.to_string()),
		total_local_bytes_free::set(available_capacity.to_string()),
		total_library_preview_media_bytes::set(thumbnail_folder_size.to_string()),
	];

	let stats = library
		.db
		.statistics()
		.upsert(
			statistics::id::equals(1),
			statistics::create(params.clone()),
			params,
		)
		.exec()
		.await?;

	info!(?stats, "Updated library statistics;");

	invalidate_query!(&library, "library.statistics");

	Ok(stats)
}
