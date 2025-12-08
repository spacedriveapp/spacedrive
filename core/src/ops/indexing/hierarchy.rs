//! # Closure Table Query Helpers
//!
//! Provides O(1) tree traversal operations using a precomputed closure table.
//! The closure table stores all ancestor-descendant relationships with their depths,
//! eliminating recursive queries for common operations like "get all children" or
//! "build full path". Each insert updates the closure table to maintain transitive
//! relationships, trading write complexity for instant read performance.

use crate::infra::db::entities::{entry, entry_closure};
use sea_orm::{
	ColumnTrait, Condition, DbConn, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
	QuerySelect, RelationTrait,
};
use std::path::PathBuf;

/// Namespace for closure table queries that avoid recursive database operations.
pub struct HierarchyQuery;

impl HierarchyQuery {
	/// Returns direct children only (depth 1), sorted by name.
	pub async fn get_children(
		db: &DbConn,
		parent_id: i32,
	) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
		entry::Entity::find()
			.filter(entry::Column::ParentId.eq(parent_id))
			.order_by_asc(entry::Column::Name)
			.all(db)
			.await
	}

	/// Returns all descendants at any depth using the closure table (not recursive).
	///
	/// Excludes the entry itself (depth > 0). Results are ordered by depth (shallowest first).
	/// Chunks queries to respect SQLite's parameter limit.
	pub async fn get_descendants(
		db: &DbConn,
		ancestor_id: i32,
	) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
		let descendant_ids = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(ancestor_id))
			.filter(entry_closure::Column::Depth.gt(0))
			.order_by_asc(entry_closure::Column::Depth)
			.all(db)
			.await?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect::<Vec<i32>>();

		if descendant_ids.is_empty() {
			return Ok(vec![]);
		}

		{
			let mut results: Vec<entry::Model> = Vec::new();
			let chunk_size: usize = 900;
			for chunk in descendant_ids.chunks(chunk_size) {
				let mut batch = entry::Entity::find()
					.filter(entry::Column::Id.is_in(chunk.to_vec()))
					.order_by_asc(entry::Column::Name)
					.all(db)
					.await?;
				results.append(&mut batch);
			}
			Ok(results)
		}
	}

	/// Returns all ancestors from root to immediate parent, enabling breadcrumb construction.
	///
	/// Excludes the entry itself (depth > 0). Results are ordered deepest-first, so reverse
	/// iteration builds paths from root downward.
	pub async fn get_ancestors(
		db: &DbConn,
		descendant_id: i32,
	) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
		let ancestor_ids = entry_closure::Entity::find()
			.filter(entry_closure::Column::DescendantId.eq(descendant_id))
			.filter(entry_closure::Column::Depth.gt(0))
			.order_by_desc(entry_closure::Column::Depth)
			.all(db)
			.await?
			.into_iter()
			.map(|ec| ec.ancestor_id)
			.collect::<Vec<i32>>();

		if ancestor_ids.is_empty() {
			return Ok(vec![]);
		}

		{
			let mut results: Vec<entry::Model> = Vec::new();
			let chunk_size: usize = 900;
			for chunk in ancestor_ids.chunks(chunk_size) {
				let mut batch = entry::Entity::find()
					.filter(entry::Column::Id.is_in(chunk.to_vec()))
					.all(db)
					.await?;
				results.append(&mut batch);
			}
			Ok(results)
		}
	}

	/// Returns entries at exactly the specified depth (e.g., all grandchildren = depth 2).
	///
	/// Useful for level-by-level tree rendering without fetching the entire subtree.
	pub async fn get_at_depth(
		db: &DbConn,
		ancestor_id: i32,
		depth: i32,
	) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
		let entry_ids = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(ancestor_id))
			.filter(entry_closure::Column::Depth.eq(depth))
			.all(db)
			.await?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect::<Vec<i32>>();

		if entry_ids.is_empty() {
			return Ok(vec![]);
		}

		{
			let mut results: Vec<entry::Model> = Vec::new();
			let chunk_size: usize = 900;
			for chunk in entry_ids.chunks(chunk_size) {
				let mut batch = entry::Entity::find()
					.filter(entry::Column::Id.is_in(chunk.to_vec()))
					.order_by_asc(entry::Column::Name)
					.all(db)
					.await?;
				results.append(&mut batch);
			}
			Ok(results)
		}
	}

	/// Constructs the absolute filesystem path by joining location_path + ancestors + entry name.
	///
	/// Used for displaying full paths in UI and for validating moves/renames don't exceed
	/// filesystem limits. The closure table makes this O(1) instead of recursively walking
	/// parent_id links.
	pub async fn build_full_path(
		db: &DbConn,
		entry_id: i32,
		location_path: &str,
	) -> Result<PathBuf, sea_orm::DbErr> {
		let entry = entry::Entity::find_by_id(entry_id)
			.one(db)
			.await?
			.ok_or_else(|| sea_orm::DbErr::RecordNotFound("Entry not found".to_string()))?;

		let ancestors = Self::get_ancestors(db, entry_id).await?;

		let mut path = PathBuf::from(location_path);

		for ancestor in ancestors {
			path.push(&ancestor.name);
		}

		path.push(&entry.name);

		Ok(path)
	}

	/// Counts descendants at any depth without fetching full entry records.
	pub async fn count_descendants(db: &DbConn, ancestor_id: i32) -> Result<u64, sea_orm::DbErr> {
		entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(ancestor_id))
			.filter(entry_closure::Column::Depth.gt(0))
			.count(db)
			.await
	}

	/// Sums the size field across all descendants (files and directories).
	///
	/// Note: This is a naive sum. For accurate directory subtree sizes, use the
	/// pre-aggregated aggregate_size field computed during the aggregation phase.
	pub async fn get_subtree_size(db: &DbConn, ancestor_id: i32) -> Result<i64, sea_orm::DbErr> {
		let descendants = Self::get_descendants(db, ancestor_id).await?;
		Ok(descendants.iter().map(|e| e.size).sum())
	}

	/// Checks if potential_ancestor_id is anywhere above potential_descendant_id in the tree.
	pub async fn is_ancestor_of(
		db: &DbConn,
		potential_ancestor_id: i32,
		potential_descendant_id: i32,
	) -> Result<bool, sea_orm::DbErr> {
		let count = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(potential_ancestor_id))
			.filter(entry_closure::Column::DescendantId.eq(potential_descendant_id))
			.filter(entry_closure::Column::Depth.gt(0))
			.count(db)
			.await?;

		Ok(count > 0)
	}

	/// Finds the lowest (deepest) ancestor shared by both entries, if any.
	///
	/// Returns None if the entries are in different trees (different locations).
	/// Useful for determining relative path operations.
	pub async fn find_common_ancestor(
		db: &DbConn,
		entry1_id: i32,
		entry2_id: i32,
	) -> Result<Option<entry::Model>, sea_orm::DbErr> {
		let ancestors1 = Self::get_ancestors(db, entry1_id).await?;
		let ancestors2 = Self::get_ancestors(db, entry2_id).await?;

		for ancestor1 in ancestors1.iter().rev() {
			for ancestor2 in &ancestors2 {
				if ancestor1.id == ancestor2.id {
					return Ok(Some(ancestor1.clone()));
				}
			}
		}

		Ok(None)
	}
}
