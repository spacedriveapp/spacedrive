//! Path resolution service for the pure hierarchical model
//!
//! This service provides efficient path resolution by utilizing the directory_paths cache table.

use std::path::PathBuf;

use sea_orm::{prelude::*, ConnectionTrait, QuerySelect, Statement};

use crate::infra::database::entities::{directory_paths, entry, DirectoryPaths, Entry};

pub struct PathResolver;

impl PathResolver {
	/// Get the full path for any entry (file or directory)
	pub async fn get_full_path<C: ConnectionTrait>(
		db: &C,
		entry_id: i32,
	) -> Result<PathBuf, DbErr> {
		// First, get the entry to determine if it's a file or directory
		let entry = Entry::find_by_id(entry_id)
			.one(db)
			.await?
			.ok_or_else(|| DbErr::RecordNotFound(format!("Entry {} not found", entry_id)))?;

		match entry.entry_kind() {
			crate::infra::database::entities::entry::EntryKind::Directory => {
				// For directories, lookup in directory_paths table
				let dir_path = DirectoryPaths::find_by_id(entry_id)
					.one(db)
					.await?
					.ok_or_else(|| {
						DbErr::RecordNotFound(format!(
							"Directory path not found for entry {}",
							entry_id
						))
					})?;
				Ok(PathBuf::from(dir_path.path))
			}
			_ => {
				// For files, get parent directory path and append file name
				if let Some(parent_id) = entry.parent_id {
					let parent_path = DirectoryPaths::find_by_id(parent_id)
						.one(db)
						.await?
						.ok_or_else(|| {
							DbErr::RecordNotFound(format!(
								"Parent directory path not found for entry {}",
								parent_id
							))
						})?;
					Ok(PathBuf::from(parent_path.path).join(&entry.name))
				} else {
					// Root file (shouldn't normally happen)
					Ok(PathBuf::from(&entry.name))
				}
			}
		}
	}

	/// Get the path for a directory from the cache
	pub async fn get_directory_path<C: ConnectionTrait>(
		db: &C,
		directory_id: i32,
	) -> Result<String, DbErr> {
		DirectoryPaths::find_by_id(directory_id)
			.one(db)
			.await?
			.map(|dp| dp.path)
			.ok_or_else(|| {
				DbErr::RecordNotFound(format!(
					"Directory path not found for entry {}",
					directory_id
				))
			})
	}

	/// Build the full path for a new directory entry
	pub async fn build_directory_path<C: ConnectionTrait>(
		db: &C,
		parent_id: Option<i32>,
		name: &str,
	) -> Result<String, DbErr> {
		if let Some(parent_id) = parent_id {
			let parent_path = Self::get_directory_path(db, parent_id).await?;
			Ok(format!("{}/{}", parent_path, name))
		} else {
			// Root directory
			Ok(name.to_string())
		}
	}

	/// Get paths for multiple entries efficiently
	pub async fn get_paths_batch<C: ConnectionTrait>(
		db: &C,
		entry_ids: Vec<i32>,
	) -> Result<Vec<(i32, PathBuf)>, DbErr> {
		// First, fetch all entries to determine types
		let mut entries: Vec<entry::Model> = Vec::new();
		let chunk_size: usize = 900;
		for chunk in entry_ids.chunks(chunk_size) {
			let mut batch = Entry::find()
				.filter(entry::Column::Id.is_in(chunk.to_vec()))
				.all(db)
				.await?;
			entries.append(&mut batch);
		}

		let mut results = Vec::with_capacity(entries.len());

		// Separate directories and files
		let mut directory_ids = Vec::new();
		let mut file_entries = Vec::new();

		for entry in entries {
			match entry.entry_kind() {
				crate::infra::database::entities::entry::EntryKind::Directory => {
					directory_ids.push(entry.id);
				}
				_ => {
					file_entries.push(entry);
				}
			}
		}

		// Batch fetch directory paths
		if !directory_ids.is_empty() {
			let mut dir_paths: Vec<directory_paths::Model> = Vec::new();
			for chunk in directory_ids.chunks(chunk_size) {
				let mut batch = DirectoryPaths::find()
					.filter(directory_paths::Column::EntryId.is_in(chunk.to_vec()))
					.all(db)
					.await?;
				dir_paths.append(&mut batch);
			}

			for dir_path in dir_paths {
				results.push((dir_path.entry_id, PathBuf::from(dir_path.path)));
			}
		}

		// Handle files by fetching parent paths
		if !file_entries.is_empty() {
			let parent_ids: Vec<i32> = file_entries.iter().filter_map(|e| e.parent_id).collect();

			let mut parent_paths: Vec<directory_paths::Model> = Vec::new();
			for chunk in parent_ids.chunks(chunk_size) {
				let mut batch = DirectoryPaths::find()
					.filter(directory_paths::Column::EntryId.is_in(chunk.to_vec()))
					.all(db)
					.await?;
				parent_paths.append(&mut batch);
			}

			// Create a map for quick lookup
			let parent_map: std::collections::HashMap<i32, String> = parent_paths
				.into_iter()
				.map(|dp| (dp.entry_id, dp.path))
				.collect();

			for file_entry in file_entries {
				let path = if let Some(parent_id) = file_entry.parent_id {
					if let Some(parent_path) = parent_map.get(&parent_id) {
						PathBuf::from(parent_path).join(&file_entry.name)
					} else {
						PathBuf::from(&file_entry.name)
					}
				} else {
					PathBuf::from(&file_entry.name)
				};
				results.push((file_entry.id, path));
			}
		}

		Ok(results)
	}

	/// Update all descendant directory paths after a move operation
	/// This should be called in a background job after moving a directory
	pub async fn update_descendant_paths<C: ConnectionTrait>(
		db: &C,
		moved_directory_id: i32,
		old_path: &str,
		new_path: &str,
	) -> Result<u64, DbErr> {
		// Use raw SQL for efficient bulk update
		let sql = r#"
            UPDATE directory_paths
            SET path = REPLACE(path, ?, ?)
            WHERE path LIKE ? || '/%'
        "#;

		let result = db
			.execute(Statement::from_sql_and_values(
				db.get_database_backend(),
				sql,
				vec![old_path.into(), new_path.into(), old_path.into()],
			))
			.await?;

		Ok(result.rows_affected())
	}
}
