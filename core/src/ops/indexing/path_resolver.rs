//! # Path Resolution via directory_paths Cache
//!
//! Resolves full filesystem paths for entries without walking parent_id chains. The directory_paths
//! table caches absolute paths for all directories, making lookups O(1) instead of O(depth). Files
//! are resolved by joining their parent's cached path with the filename. This table is updated during
//! indexing and move operations to keep paths in sync with the entry hierarchy.

use std::path::PathBuf;

use sea_orm::{prelude::*, ConnectionTrait, QuerySelect, Statement};

use crate::infra::db::entities::{directory_paths, entry, DirectoryPaths, Entry};

pub struct PathResolver;

impl PathResolver {
	/// Resolves the absolute path by looking up directories in the cache or reconstructing file paths.
	pub async fn get_full_path<C: ConnectionTrait>(
		db: &C,
		entry_id: i32,
	) -> Result<PathBuf, DbErr> {
		let entry = Entry::find_by_id(entry_id)
			.one(db)
			.await?
			.ok_or_else(|| DbErr::RecordNotFound(format!("Entry {} not found", entry_id)))?;

		match entry.entry_kind() {
			crate::infra::db::entities::entry::EntryKind::Directory => {
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

					let full_filename = if let Some(ext) = &entry.extension {
						format!("{}.{}", entry.name, ext)
					} else {
						entry.name.clone()
					};

					Ok(PathBuf::from(parent_path.path).join(full_filename))
				} else {
					let full_filename = if let Some(ext) = &entry.extension {
						format!("{}.{}", entry.name, ext)
					} else {
						entry.name.clone()
					};
					Ok(PathBuf::from(full_filename))
				}
			}
		}
	}

	/// Fetches the cached path string directly from directory_paths without entry lookup.
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

	/// Constructs the path string for a new directory by joining its parent's path with its name.
	///
	/// Used during indexing to populate the directory_paths table for newly discovered directories.
	pub async fn build_directory_path<C: ConnectionTrait>(
		db: &C,
		parent_id: Option<i32>,
		name: &str,
	) -> Result<String, DbErr> {
		if let Some(parent_id) = parent_id {
			let parent_path = Self::get_directory_path(db, parent_id).await?;
			Ok(format!("{}/{}", parent_path, name))
		} else {
			Ok(name.to_string())
		}
	}

	/// Resolves paths for multiple entries in batched queries to minimize database round-trips.
	pub async fn get_paths_batch<C: ConnectionTrait>(
		db: &C,
		entry_ids: Vec<i32>,
	) -> Result<Vec<(i32, PathBuf)>, DbErr> {
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

		let mut directory_ids = Vec::new();
		let mut file_entries = Vec::new();

		for entry in entries {
			match entry.entry_kind() {
				crate::infra::db::entities::entry::EntryKind::Directory => {
					directory_ids.push(entry.id);
				}
				_ => {
					file_entries.push(entry);
				}
			}
		}

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

	/// Bulk-updates descendant directory paths after moving a directory tree.
	///
	/// Uses a single SQL REPLACE to rewrite all paths under the moved directory's old prefix.
	/// Should be called after updating the moved directory's entry.parent_id and directory_paths.path.
	pub async fn update_descendant_paths<C: ConnectionTrait>(
		db: &C,
		moved_directory_id: i32,
		old_path: &str,
		new_path: &str,
	) -> Result<u64, DbErr> {
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
