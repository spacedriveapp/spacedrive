use std::collections::HashMap;
use std::{fs, io};
use std::path::Path;

use crate::job::JobReportUpdate;
use crate::sys::get_location;
use crate::{
	file::FileError,
	job::{Job, WorkerContext},
	prisma::file_path,
	CoreContext,
};
use futures::executor::block_on;
use prisma_client_rust::prisma_models::PrismaValue;
use prisma_client_rust::raw::Raw;
use prisma_client_rust::{raw, Direction};
use serde::{Deserialize, Serialize};

use super::checksum::generate_cas_id;
#[derive(Deserialize, Serialize, Debug)]
pub struct FileCreated {
	pub id: i32,
	pub cas_id: String,
}

#[derive(Debug)]
pub struct FileIdentifierJob {
	pub location_id: i32,
	pub path: String,
}

#[async_trait::async_trait]
impl Job for FileIdentifierJob {
	fn name(&self) -> &'static str {
		"file_identifier"
	}
	async fn run(&self, ctx: WorkerContext) -> Result<(), Box<dyn std::error::Error>> {
		println!("Identifying files");
		let location = get_location(&ctx.core_ctx, self.location_id).await?;
		let location_path = location.path.unwrap_or("".to_string());

		let total_count = count_orphan_file_paths(&ctx.core_ctx, location.id.into()).await?;

		println!("Found {} orphan file paths", total_count);

		let task_count = (total_count as f64 / 100f64).ceil() as usize;

		println!("Will process {} tasks", task_count);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		let db = ctx.core_ctx.database.clone();

		let ctx = tokio::task::spawn_blocking(move || {
			let mut completed: usize = 0;
			let mut cursor: i32 = 1;

			while completed < task_count {
				let file_paths = block_on(get_orphan_file_paths(&ctx.core_ctx, cursor)).unwrap();
				println!(
					"Processing {:?} orphan files. ({} completed of {})",
					file_paths.len(),
					completed,
					task_count
				);

				// map cas_id to file_path ids
				let mut cas_id_lookup: HashMap<i32, String> = HashMap::new();
				// raw values to be inserted into the database
				let mut values: Vec<PrismaValue> = Vec::new();

				// only rows that have a valid cas_id to be inserted
				for file_path in file_paths.iter() {
					match prepare_file_values(&location_path, file_path) {
						Ok((cas_id, data)) => {
							cas_id_lookup.insert(file_path.id, cas_id);
							values.extend(data);
						}
						Err(e) => {
							println!("Error processing file: {}", e);
							continue;
						}
					};
				}
				if values.len() == 0 {
					println!("No orphan files to process, finishing...");
					break;
				}

				println!("Inserting {} unique file records ({:?} values)", file_paths.len(), values.len());
				
				let files: Vec<FileCreated> = block_on(db._query_raw(Raw::new(
				  &format!(
				    "INSERT INTO files (cas_id, size_in_bytes) VALUES {} ON CONFLICT (cas_id) DO NOTHING RETURNING id, cas_id",
				    vec!["({}, {})"; file_paths.len()].join(",")
				  ),
				  values
				))).unwrap_or_else(|e| {
					println!("Error inserting files: {}", e);
					Vec::new()
				});

				println!("Unique files: {:?}" , files);

				// assign unique file to file path
				println!("Assigning {} unique file ids to origin file_paths", files.len());
				for (_, (file_path_id, cas_id)) in cas_id_lookup.iter().enumerate() {
					// get the cas id from the lookup table
					let file_id = files.iter().find(|f| &f.cas_id == cas_id).unwrap().id;
				  block_on(
				    db.file_path()
				      .find_unique(file_path::id::equals(file_path_id.clone()))
				      .update(vec![
				        file_path::file_id::set(Some(file_id))
				      ])
				      .exec()
				  ).unwrap();
				}

				let last_row = file_paths.last().unwrap();

				cursor = last_row.id;
				completed += 1;
				ctx.progress(vec![
				  JobReportUpdate::CompletedTaskCount(completed),
				  JobReportUpdate::Message(format!(
				    "Processed {} of {} orphan files",
				    completed,
				    task_count
				  )),
				]);
			}
			ctx
		})
		.await?;

		let remaining = count_orphan_file_paths(&ctx.core_ctx, location.id.into()).await?;

		Ok(())
	}
}

#[derive(Deserialize, Serialize, Debug)]
struct CountRes {
	count: Option<usize>,
}

pub async fn count_orphan_file_paths(
	ctx: &CoreContext,
	location_id: i64,
) -> Result<usize, FileError> {
	let db = &ctx.database;
	let files_count = db
		._query_raw::<CountRes>(raw!(
			"SELECT COUNT(*) AS count FROM file_paths WHERE file_id IS NULL AND is_dir IS FALSE AND location_id = {}",
			PrismaValue::Int(location_id)
		))
		.await?;
	Ok(files_count[0].count.unwrap_or(0))
}

pub async fn get_orphan_file_paths(
	ctx: &CoreContext,
	cursor: i32,
) -> Result<Vec<file_path::Data>, FileError> {
	let db = &ctx.database;
	println!("cursor: {:?}", cursor);
	let files = db
		.file_path()
		.find_many(vec![
			file_path::file_id::equals(None),
			file_path::is_dir::equals(false),
		])
		.order_by(file_path::id::order(Direction::Asc))
		.cursor(file_path::id::cursor(cursor))
		.take(100)
		.exec()
		.await?;
	Ok(files)
}

pub fn prepare_file_values(
	location_path: &str,
	file_path: &file_path::Data,
) -> Result<(String, [PrismaValue; 2]), io::Error> {
	let path = Path::new(&location_path).join(Path::new(file_path.materialized_path.as_str()));
	// println!("Processing file: {:?}", path);
	let metadata = fs::metadata(&path)?;
	let cas_id = {
		if !file_path.is_dir {
			let mut ret = generate_cas_id(path.clone(), metadata.len()).unwrap();
			ret.truncate(16);
			ret
		} else {
			"".to_string()
		}
	};

	println!("cas id for path {:?} is {:?}", path, cas_id);

	Ok((cas_id.clone(), [PrismaValue::String(cas_id), PrismaValue::Int(0)]))
}
