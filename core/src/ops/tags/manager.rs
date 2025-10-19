//! Semantic Tag Service
//!
//! Core service for managing the semantic tagging architecture.
//! Provides high-level operations for tag creation, hierarchy management,
//! context resolution, and conflict resolution during sync.

use crate::domain::tag::{
	OrganizationalPattern, PatternType, PrivacyLevel, RelationshipType, Tag, TagApplication,
	TagError, TagMergeResult, TagRelationship, TagType,
};
use crate::infra::db::entities::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use sea_orm::{
	ActiveModelTrait, ColumnTrait, ConnectionTrait, DbConn, DbErr, EntityTrait, NotSet,
	QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Service for managing semantic tags and their relationships
#[derive(Clone)]
pub struct TagManager {
	db: Arc<DatabaseConnection>,
	context_resolver: Arc<TagContextResolver>,
	usage_analyzer: Arc<TagUsageAnalyzer>,
	closure_service: Arc<TagClosureService>,
}

// Helper function to convert database model to domain model
fn model_to_domain(model: tag::Model) -> Result<Tag, TagError> {
	let aliases: Vec<String> = model
		.aliases
		.as_ref()
		.and_then(|json| serde_json::from_value(json.clone()).ok())
		.unwrap_or_default();

	let attributes: HashMap<String, serde_json::Value> = model
		.attributes
		.as_ref()
		.and_then(|json| serde_json::from_value(json.clone()).ok())
		.unwrap_or_default();

	let composition_rules = model
		.composition_rules
		.as_ref()
		.and_then(|json| serde_json::from_value(json.clone()).ok())
		.unwrap_or_default();

	let tag_type = TagType::from_str(&model.tag_type)
		.ok_or_else(|| TagError::DatabaseError(format!("Invalid tag_type: {}", model.tag_type)))?;

	let privacy_level = PrivacyLevel::from_str(&model.privacy_level).ok_or_else(|| {
		TagError::DatabaseError(format!("Invalid privacy_level: {}", model.privacy_level))
	})?;

	Ok(Tag {
		id: model.uuid,
		canonical_name: model.canonical_name,
		display_name: model.display_name,
		formal_name: model.formal_name,
		abbreviation: model.abbreviation,
		aliases,
		namespace: model.namespace,
		tag_type,
		color: model.color,
		icon: model.icon,
		description: model.description,
		is_organizational_anchor: model.is_organizational_anchor,
		privacy_level,
		search_weight: model.search_weight,
		attributes,
		composition_rules,
		created_at: model.created_at,
		updated_at: model.updated_at,
		created_by_device: model.created_by_device.unwrap_or_default(),
	})
}

impl TagManager {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		let context_resolver = Arc::new(TagContextResolver::new(db.clone()));
		let usage_analyzer = Arc::new(TagUsageAnalyzer::new(db.clone()));
		let closure_service = Arc::new(TagClosureService::new(db.clone()));

		Self {
			db,
			context_resolver,
			usage_analyzer,
			closure_service,
		}
	}

	/// Create a new semantic tag (returns domain Tag for backwards compatibility)
	pub async fn create_tag(
		&self,
		canonical_name: String,
		namespace: Option<String>,
		created_by_device: Uuid,
	) -> Result<Tag, TagError> {
		let entity = self
			.create_tag_entity(canonical_name, namespace, created_by_device)
			.await?;
		model_to_domain(entity)
	}

	/// Create a new semantic tag with full options (returns entity Model for efficient sync)
	///
	/// This variant accepts all optional fields and returns the database entity Model directly.
	/// Use this in actions that need all fields and sync events immediately after creation.
	#[allow(clippy::too_many_arguments)]
	pub async fn create_tag_entity_full(
		&self,
		canonical_name: String,
		namespace: Option<String>,
		display_name: Option<String>,
		formal_name: Option<String>,
		abbreviation: Option<String>,
		aliases: Vec<String>,
		tag_type: Option<TagType>,
		color: Option<String>,
		icon: Option<String>,
		description: Option<String>,
		is_organizational_anchor: bool,
		privacy_level: Option<PrivacyLevel>,
		search_weight: Option<i32>,
		attributes: Option<HashMap<String, serde_json::Value>>,
		created_by_device: Uuid,
	) -> Result<tag::Model, TagError> {
		let db = &*self.db;

		// Check for name conflicts in the same namespace
		if let Some(_existing) = self
			.find_tag_by_name_and_namespace(&canonical_name, namespace.as_deref())
			.await?
		{
			return Err(TagError::NameConflict(format!(
				"Tag '{}' already exists in namespace '{:?}'",
				canonical_name, namespace
			)));
		}

		let tag_uuid = Uuid::new_v4();
		let now = chrono::Utc::now();

		// Build ActiveModel with all provided fields
		let active_model = tag::ActiveModel {
			id: NotSet,
			uuid: Set(tag_uuid),
			canonical_name: Set(canonical_name),
			display_name: Set(display_name),
			formal_name: Set(formal_name),
			abbreviation: Set(abbreviation),
			aliases: Set(if aliases.is_empty() {
				None
			} else {
				Some(serde_json::to_value(&aliases).unwrap().into())
			}),
			namespace: Set(namespace),
			tag_type: Set(tag_type.unwrap_or(TagType::Standard).as_str().to_string()),
			color: Set(color),
			icon: Set(icon),
			description: Set(description),
			is_organizational_anchor: Set(is_organizational_anchor),
			privacy_level: Set(privacy_level
				.unwrap_or(PrivacyLevel::Normal)
				.as_str()
				.to_string()),
			search_weight: Set(search_weight.unwrap_or(100)),
			attributes: Set(attributes.and_then(|attrs| {
				if attrs.is_empty() {
					None
				} else {
					Some(serde_json::to_value(&attrs).unwrap().into())
				}
			})),
			composition_rules: Set(None), // Not exposed in CreateTagInput
			created_at: Set(now),
			updated_at: Set(now),
			created_by_device: Set(Some(created_by_device)),
		};

		let result = active_model
			.insert(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(result)
	}

	/// Create a new semantic tag (returns entity Model for efficient sync)
	///
	/// This variant returns the database entity Model directly, avoiding
	/// the need for a roundtrip query when syncing. Use this in actions
	/// that need to emit sync events immediately after creation.
	pub async fn create_tag_entity(
		&self,
		canonical_name: String,
		namespace: Option<String>,
		created_by_device: Uuid,
	) -> Result<tag::Model, TagError> {
		let db = &*self.db;

		// Check for name conflicts in the same namespace
		if let Some(_existing) = self
			.find_tag_by_name_and_namespace(&canonical_name, namespace.as_deref())
			.await?
		{
			return Err(TagError::NameConflict(format!(
				"Tag '{}' already exists in namespace '{:?}'",
				canonical_name, namespace
			)));
		}

		let tag = Tag::new(canonical_name.clone(), created_by_device);

		// Insert into database
		let active_model = tag::ActiveModel {
			id: NotSet,
			uuid: Set(tag.id),
			canonical_name: Set(canonical_name),
			display_name: Set(tag.display_name.clone()),
			formal_name: Set(tag.formal_name.clone()),
			abbreviation: Set(tag.abbreviation.clone()),
			aliases: Set(if tag.aliases.is_empty() {
				None
			} else {
				Some(serde_json::to_value(&tag.aliases).unwrap().into())
			}),
			namespace: Set(namespace),
			tag_type: Set(tag.tag_type.as_str().to_string()),
			color: Set(tag.color.clone()),
			icon: Set(tag.icon.clone()),
			description: Set(tag.description.clone()),
			is_organizational_anchor: Set(tag.is_organizational_anchor),
			privacy_level: Set(tag.privacy_level.as_str().to_string()),
			search_weight: Set(tag.search_weight),
			attributes: Set(if tag.attributes.is_empty() {
				None
			} else {
				Some(serde_json::to_value(&tag.attributes).unwrap().into())
			}),
			composition_rules: Set(if tag.composition_rules.is_empty() {
				None
			} else {
				Some(serde_json::to_value(&tag.composition_rules).unwrap().into())
			}),
			created_at: Set(tag.created_at),
			updated_at: Set(tag.updated_at),
			created_by_device: Set(Some(created_by_device)),
		};

		let result = active_model
			.insert(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(result)
	}

	/// Update an existing tag with new values
	pub async fn update_tag(&self, tag: &Tag) -> Result<Tag, TagError> {
		let db = &*self.db;

		// Find the existing tag by UUID to get its database ID
		let existing_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(tag.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or_else(|| TagError::TagNotFound)?;

		// Use a direct SQL update to avoid relationship issues
		use sea_orm::{ConnectionTrait, Statement};

		let aliases_json = if tag.aliases.is_empty() {
			None
		} else {
			Some(serde_json::to_value(&tag.aliases).unwrap().to_string())
		};

		let attributes_json = if tag.attributes.is_empty() {
			None
		} else {
			Some(serde_json::to_value(&tag.attributes).unwrap().to_string())
		};

		let composition_rules_json = if tag.composition_rules.is_empty() {
			None
		} else {
			Some(
				serde_json::to_value(&tag.composition_rules)
					.unwrap()
					.to_string(),
			)
		};

		// Direct SQL update to avoid SeaORM relationship issues
		let update_sql = "UPDATE tag SET
            canonical_name = ?,
            display_name = ?,
            formal_name = ?,
            abbreviation = ?,
            aliases = ?,
            namespace = ?,
            tag_type = ?,
            color = ?,
            icon = ?,
            description = ?,
            is_organizational_anchor = ?,
            privacy_level = ?,
            search_weight = ?,
            attributes = ?,
            composition_rules = ?,
            updated_at = ?
            WHERE id = ?";

		let stmt = Statement::from_sql_and_values(
			sea_orm::DatabaseBackend::Sqlite,
			update_sql,
			vec![
				tag.canonical_name.clone().into(),
				tag.display_name.clone().into(),
				tag.formal_name.clone().into(),
				tag.abbreviation.clone().into(),
				aliases_json.into(),
				tag.namespace.clone().into(),
				tag.tag_type.as_str().to_string().into(),
				tag.color.clone().into(),
				tag.icon.clone().into(),
				tag.description.clone().into(),
				tag.is_organizational_anchor.into(),
				tag.privacy_level.as_str().to_string().into(),
				tag.search_weight.into(),
				attributes_json.into(),
				composition_rules_json.into(),
				chrono::Utc::now().into(),
				existing_model.id.into(),
			],
		);

		db.execute(stmt)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Fetch the updated model and convert to domain object
		let updated_model = tag::Entity::find_by_id(existing_model.id)
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or_else(|| TagError::TagNotFound)?;

		model_to_domain(updated_model)
	}

	/// Delete a tag and all its relationships
	pub async fn delete_tag(&self, tag_id: Uuid) -> Result<(), TagError> {
		let db = &*self.db;

		// Find the tag first to ensure it exists
		let existing_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(tag_id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or_else(|| TagError::TagNotFound)?;

		// Delete all relationships where this tag is parent or child
		tag_relationship::Entity::delete_many()
			.filter(
				tag_relationship::Column::ParentTagId
					.eq(existing_model.id)
					.or(tag_relationship::Column::ChildTagId.eq(existing_model.id)),
			)
			.exec(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Delete all closure table entries for this tag
		tag_closure::Entity::delete_many()
			.filter(
				tag_closure::Column::AncestorId
					.eq(existing_model.id)
					.or(tag_closure::Column::DescendantId.eq(existing_model.id)),
			)
			.exec(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Delete all tag applications
		user_metadata_tag::Entity::delete_many()
			.filter(user_metadata_tag::Column::TagId.eq(existing_model.id))
			.exec(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Delete all usage patterns involving this tag
		tag_usage_pattern::Entity::delete_many()
			.filter(
				tag_usage_pattern::Column::TagId
					.eq(existing_model.id)
					.or(tag_usage_pattern::Column::CoOccurrenceTagId.eq(existing_model.id)),
			)
			.exec(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Finally, delete the tag itself
		tag::Entity::delete_many()
			.filter(tag::Column::Uuid.eq(tag_id))
			.exec(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(())
	}

	/// Find a tag by its canonical name and namespace
	pub async fn find_tag_by_name_and_namespace(
		&self,
		name: &str,
		namespace: Option<&str>,
	) -> Result<Option<Tag>, TagError> {
		let db = &*self.db;

		let mut query = tag::Entity::find().filter(tag::Column::CanonicalName.eq(name));

		query = match namespace {
			Some(ns) => query.filter(tag::Column::Namespace.eq(ns)),
			None => query.filter(tag::Column::Namespace.is_null()),
		};

		let model = query
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		match model {
			Some(m) => Ok(Some(model_to_domain(m)?)),
			None => Ok(None),
		}
	}

	/// Find all tags matching a name (across all namespaces)
	pub async fn find_tags_by_name(&self, name: &str) -> Result<Vec<Tag>, TagError> {
		let db = &*self.db;

		// Search across canonical_name, formal_name, and abbreviation
		let models = tag::Entity::find()
			.filter(
				tag::Column::CanonicalName
					.eq(name)
					.or(tag::Column::FormalName.eq(name))
					.or(tag::Column::Abbreviation.eq(name)), // Note: aliases are JSON, we'll handle them separately
			)
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let mut results = Vec::new();

		// Convert models to domain objects
		for model in models {
			results.push(model_to_domain(model)?);
		}

		// Also search aliases using a separate query
		// Get all tags and filter by aliases in memory (for now)
		// TODO: Optimize this with JSON query operators or FTS5
		let all_models = tag::Entity::find()
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		for model in all_models {
			if let Some(aliases_json) = &model.aliases {
				if let Ok(aliases) = serde_json::from_value::<Vec<String>>(aliases_json.clone()) {
					if aliases.iter().any(|alias| alias.eq_ignore_ascii_case(name)) {
						let domain_tag = model_to_domain(model)?;
						// Avoid duplicates
						if !results.iter().any(|t| t.id == domain_tag.id) {
							results.push(domain_tag);
						}
					}
				}
			}
		}

		Ok(results)
	}

	/// Resolve ambiguous tag names using context
	pub async fn resolve_ambiguous_tag(
		&self,
		tag_name: &str,
		context_tags: &[Tag],
	) -> Result<Vec<Tag>, TagError> {
		self.context_resolver
			.resolve_ambiguous_tag(tag_name, context_tags)
			.await
	}

	/// Create a relationship between two tags
	pub async fn create_relationship(
		&self,
		parent_id: Uuid,
		child_id: Uuid,
		relationship_type: RelationshipType,
		strength: Option<f32>,
	) -> Result<(), TagError> {
		let db = &*self.db;

		// Check for circular references
		if self.would_create_cycle(parent_id, child_id).await? {
			return Err(TagError::CircularReference);
		}

		let strength = strength.unwrap_or(1.0);

		// Get database IDs for the tags
		let parent_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(parent_id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		let child_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(child_id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		// Insert relationship into database
		let relationship = tag_relationship::ActiveModel {
			id: NotSet,
			parent_tag_id: Set(parent_model.id),
			child_tag_id: Set(child_model.id),
			relationship_type: Set(relationship_type.as_str().to_string()),
			strength: Set(strength),
			created_at: Set(Utc::now()),
			uuid: Set(Uuid::new_v4()),
			version: Set(1),
			updated_at: Set(Utc::now()),
		};

		relationship
			.insert(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Update closure table if this is a parent-child relationship
		if relationship_type == RelationshipType::ParentChild {
			self.closure_service
				.add_relationship(parent_model.id, child_model.id)
				.await?;
		}

		Ok(())
	}

	/// Check if adding a relationship would create a cycle
	async fn would_create_cycle(&self, parent_id: Uuid, child_id: Uuid) -> Result<bool, TagError> {
		// If child_id is an ancestor of parent_id, adding this relationship would create a cycle
		let ancestors = self.closure_service.get_all_ancestors(parent_id).await?;
		Ok(ancestors.contains(&child_id))
	}

	/// Check if two tags are already related
	async fn are_tags_related(&self, tag1_id: Uuid, tag2_id: Uuid) -> Result<bool, TagError> {
		let db = &*self.db;

		// Get database IDs
		let tag1_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(tag1_id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let tag2_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(tag2_id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		if let (Some(tag1), Some(tag2)) = (tag1_model, tag2_model) {
			let relationship = tag_relationship::Entity::find()
				.filter(
					tag_relationship::Column::ParentTagId
						.eq(tag1.id)
						.and(tag_relationship::Column::ChildTagId.eq(tag2.id))
						.or(tag_relationship::Column::ParentTagId
							.eq(tag2.id)
							.and(tag_relationship::Column::ChildTagId.eq(tag1.id))),
				)
				.one(&*db)
				.await
				.map_err(|e| TagError::DatabaseError(e.to_string()))?;

			Ok(relationship.is_some())
		} else {
			Ok(false)
		}
	}

	/// Get tags by their IDs (make public for use by other services)
	pub async fn get_tags_by_ids(&self, tag_ids: &[Uuid]) -> Result<Vec<Tag>, TagError> {
		let db = &*self.db;

		let models = tag::Entity::find()
			.filter(tag::Column::Uuid.is_in(tag_ids.iter().map(|id| *id).collect::<Vec<_>>()))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let mut results = Vec::new();
		for model in models {
			results.push(model_to_domain(model)?);
		}

		Ok(results)
	}

	/// Get all tags that are descendants of the given tag
	pub async fn get_descendants(&self, tag_id: Uuid) -> Result<Vec<Tag>, TagError> {
		let descendant_ids = self.closure_service.get_all_descendants(tag_id).await?;
		self.get_tags_by_ids(&descendant_ids).await
	}

	/// Get all tags that are ancestors of the given tag
	pub async fn get_ancestors(&self, tag_id: Uuid) -> Result<Vec<Tag>, TagError> {
		let ancestor_ids = self.closure_service.get_all_ancestors(tag_id).await?;
		self.get_tags_by_ids(&ancestor_ids).await
	}

	/// Apply semantic discovery to find organizational patterns
	pub async fn discover_organizational_patterns(
		&self,
	) -> Result<Vec<OrganizationalPattern>, TagError> {
		let mut patterns = Vec::new();

		// Analyze tag co-occurrence patterns
		let usage_patterns = self.usage_analyzer.get_frequent_co_occurrences(10).await?;

		for (tag1_id, tag2_id, count) in usage_patterns {
			// Check if these tags should be related
			if count > 5 && !self.are_tags_related(tag1_id, tag2_id).await? {
				patterns.push(OrganizationalPattern {
                    pattern_type: PatternType::FrequentCoOccurrence,
                    tags_involved: vec![tag1_id, tag2_id],
                    confidence: (count as f32) / 100.0,
                    suggestion: format!("Consider creating a relationship between tags that frequently appear together"),
                    discovered_at: Utc::now(),
                });
			}
		}

		// TODO: Add more pattern discovery algorithms
		// - Hierarchical relationship detection
		// - Semantic similarity analysis
		// - Contextual grouping analysis

		Ok(patterns)
	}

	/// Merge tag applications during sync (union merge strategy)
	pub async fn merge_tag_applications(
		&self,
		local_applications: Vec<TagApplication>,
		remote_applications: Vec<TagApplication>,
	) -> Result<TagMergeResult, TagError> {
		let resolver = TagConflictResolver::new();
		resolver
			.merge_tag_applications(local_applications, remote_applications)
			.await
	}

	/// Search for tags using various criteria
	pub async fn search_tags(
		&self,
		query: &str,
		namespace_filter: Option<&str>,
		tag_type_filter: Option<TagType>,
		include_archived: bool,
	) -> Result<Vec<Tag>, TagError> {
		let db = &*self.db;

		// Try FTS5 search first, fall back to LIKE patterns if FTS5 is not available
		let mut tag_db_ids = Vec::new();

		// Attempt FTS5 search (skip if FTS5 table doesn't exist)
		if let Ok(fts_results) = db.query_all(
            sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                format!(
                    "SELECT rowid FROM tag_search_fts WHERE tag_search_fts MATCH '{}' ORDER BY bm25(tag_search_fts)",
                    query.replace("\"", "\"\"")
                )
            )
        ).await {
            for row in fts_results {
                if let Ok(tag_id) = row.try_get::<i32>("", "rowid") {
                    tag_db_ids.push(tag_id);
                }
            }
        }

		// If FTS5 didn't return results, fall back to LIKE patterns
		if tag_db_ids.is_empty() {
			let search_pattern = format!("%{}%", query);
			let like_models = tag::Entity::find()
				.filter(
					tag::Column::CanonicalName
						.like(&search_pattern)
						.or(tag::Column::DisplayName.like(&search_pattern))
						.or(tag::Column::FormalName.like(&search_pattern))
						.or(tag::Column::Abbreviation.like(&search_pattern))
						.or(tag::Column::Description.like(&search_pattern)),
				)
				.all(&*db)
				.await
				.map_err(|e| TagError::DatabaseError(e.to_string()))?;

			tag_db_ids = like_models.into_iter().map(|m| m.id).collect();
		}

		if tag_db_ids.is_empty() {
			return Ok(Vec::new());
		}

		// Build filtered query with the found tag IDs
		let mut query_builder = tag::Entity::find().filter(tag::Column::Id.is_in(tag_db_ids));

		// Apply namespace filter
		if let Some(namespace) = namespace_filter {
			query_builder = query_builder.filter(tag::Column::Namespace.eq(namespace));
		}

		// Apply tag type filter
		if let Some(ref tag_type) = tag_type_filter {
			query_builder = query_builder.filter(tag::Column::TagType.eq(tag_type.as_str()));
		}

		// Apply privacy filter
		if !include_archived {
			query_builder = query_builder.filter(tag::Column::PrivacyLevel.eq("normal"));
		}

		let models = query_builder
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let mut results = Vec::new();
		for model in models {
			results.push(model_to_domain(model)?);
		}

		// Also search aliases in memory (for now)
		// TODO: Optimize this with JSON query operators or FTS5
		let all_models = tag::Entity::find()
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		for model in all_models {
			if let Some(aliases_json) = &model.aliases {
				if let Ok(aliases) = serde_json::from_value::<Vec<String>>(aliases_json.clone()) {
					if aliases
						.iter()
						.any(|alias| alias.to_lowercase().contains(&query.to_lowercase()))
					{
						// Apply additional filters to alias matches before converting to domain
						let matches_namespace = namespace_filter.map_or(true, |ns| {
							model
								.namespace
								.as_ref()
								.map_or(false, |model_ns| model_ns == ns)
						});
						let matches_tag_type = tag_type_filter
							.as_ref()
							.map_or(true, |tt| model.tag_type == tt.as_str());
						let matches_privacy = include_archived || model.privacy_level == "normal";

						if matches_namespace && matches_tag_type && matches_privacy {
							let domain_tag = model_to_domain(model)?;
							// Avoid duplicates
							if !results.iter().any(|t| t.id == domain_tag.id) {
								results.push(domain_tag);
							}
						}
					}
				}
			}
		}

		Ok(results)
	}

	/// Update tag usage statistics
	pub async fn record_tag_usage(
		&self,
		tag_applications: &[TagApplication],
	) -> Result<(), TagError> {
		self.usage_analyzer
			.record_usage_patterns(tag_applications)
			.await
	}
}

/// Resolves tag context and disambiguation
pub struct TagContextResolver {
	db: Arc<DatabaseConnection>,
}

impl TagContextResolver {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		Self { db }
	}

	/// Resolve which version of an ambiguous tag name is intended
	pub async fn resolve_ambiguous_tag(
		&self,
		tag_name: &str,
		context_tags: &[Tag],
	) -> Result<Vec<Tag>, TagError> {
		// Find all possible tags with this name
		let candidates = self.find_all_name_matches(tag_name).await?;

		if candidates.len() <= 1 {
			return Ok(candidates);
		}

		// Score candidates based on context compatibility
		let mut scored_candidates = Vec::new();

		for candidate in candidates {
			let mut score = 0.0;

			// 1. Namespace compatibility
			score += self
				.calculate_namespace_compatibility(&candidate, context_tags)
				.await?;

			// 2. Usage pattern compatibility
			score += self
				.calculate_usage_compatibility(&candidate, context_tags)
				.await?;

			// 3. Hierarchical relationship compatibility
			score += self
				.calculate_hierarchy_compatibility(&candidate, context_tags)
				.await?;

			scored_candidates.push((candidate, score));
		}

		// Sort by score and return ranked results
		scored_candidates
			.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

		Ok(scored_candidates.into_iter().map(|(tag, _)| tag).collect())
	}

	async fn find_all_name_matches(&self, name: &str) -> Result<Vec<Tag>, TagError> {
		let db = &*self.db;

		// Search across canonical_name, formal_name, and abbreviation
		let models = tag::Entity::find()
			.filter(
				tag::Column::CanonicalName
					.eq(name)
					.or(tag::Column::FormalName.eq(name))
					.or(tag::Column::Abbreviation.eq(name)),
			)
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let mut results = Vec::new();
		for model in models {
			results.push(model_to_domain(model)?);
		}

		// Also search aliases (in-memory for now)
		let all_models = tag::Entity::find()
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		for model in all_models {
			if let Some(aliases_json) = &model.aliases {
				if let Ok(aliases) = serde_json::from_value::<Vec<String>>(aliases_json.clone()) {
					if aliases.iter().any(|alias| alias.eq_ignore_ascii_case(name)) {
						let domain_tag = model_to_domain(model)?;
						if !results.iter().any(|t| t.id == domain_tag.id) {
							results.push(domain_tag);
						}
					}
				}
			}
		}

		Ok(results)
	}

	async fn calculate_namespace_compatibility(
		&self,
		candidate: &Tag,
		context_tags: &[Tag],
	) -> Result<f32, TagError> {
		let mut score = 0.0;

		if let Some(candidate_namespace) = &candidate.namespace {
			let matching_namespaces = context_tags
				.iter()
				.filter_map(|t| t.namespace.as_ref())
				.filter(|ns| *ns == candidate_namespace)
				.count();

			if !context_tags.is_empty() {
				score = (matching_namespaces as f32) / (context_tags.len() as f32);
			}
		}

		Ok(score * 0.5) // Weight namespace compatibility
	}

	async fn calculate_usage_compatibility(
		&self,
		candidate: &Tag,
		context_tags: &[Tag],
	) -> Result<f32, TagError> {
		let usage_analyzer = TagUsageAnalyzer::new(self.db.clone());
		usage_analyzer
			.calculate_co_occurrence_score(candidate, context_tags)
			.await
	}

	async fn calculate_hierarchy_compatibility(
		&self,
		candidate: &Tag,
		context_tags: &[Tag],
	) -> Result<f32, TagError> {
		let closure_service = TagClosureService::new(self.db.clone());
		let mut compatibility_score = 0.0;
		let mut relationship_count = 0;

		for context_tag in context_tags {
			// Check if candidate and context tag share any ancestors or descendants
			let candidate_ancestors = closure_service.get_all_ancestors(candidate.id).await?;
			let candidate_descendants = closure_service.get_all_descendants(candidate.id).await?;

			if candidate_ancestors.contains(&context_tag.id)
				|| candidate_descendants.contains(&context_tag.id)
			{
				compatibility_score += 1.0;
				relationship_count += 1;
			}
		}

		if relationship_count > 0 {
			Ok((compatibility_score / context_tags.len() as f32) * 0.3) // Weight hierarchy compatibility
		} else {
			Ok(0.0)
		}
	}
}

/// Analyzes tag usage patterns for intelligent suggestions
pub struct TagUsageAnalyzer {
	db: Arc<DatabaseConnection>,
}

impl TagUsageAnalyzer {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		Self { db }
	}

	/// Record co-occurrence patterns when tags are applied together
	pub async fn record_usage_patterns(
		&self,
		tag_applications: &[TagApplication],
	) -> Result<(), TagError> {
		let db = &*self.db;

		// Get database IDs for all tags
		let tag_uuids: Vec<Uuid> = tag_applications.iter().map(|app| app.tag_id).collect();
		let tag_models = tag::Entity::find()
			.filter(tag::Column::Uuid.is_in(tag_uuids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let uuid_to_db_id: HashMap<Uuid, i32> =
			tag_models.into_iter().map(|m| (m.uuid, m.id)).collect();

		// Record co-occurrence between all pairs of tags in this application set
		for (i, app1) in tag_applications.iter().enumerate() {
			for app2 in tag_applications.iter().skip(i + 1) {
				if let (Some(&tag1_db_id), Some(&tag2_db_id)) = (
					uuid_to_db_id.get(&app1.tag_id),
					uuid_to_db_id.get(&app2.tag_id),
				) {
					self.increment_co_occurrence(&*db, tag1_db_id, tag2_db_id)
						.await?;
					// Also record the reverse relationship
					self.increment_co_occurrence(&*db, tag2_db_id, tag1_db_id)
						.await?;
				}
			}
		}

		Ok(())
	}

	async fn increment_co_occurrence(
		&self,
		db: &DbConn,
		tag1_db_id: i32,
		tag2_db_id: i32,
	) -> Result<(), TagError> {
		// Try to find existing pattern
		let existing = tag_usage_pattern::Entity::find()
			.filter(tag_usage_pattern::Column::TagId.eq(tag1_db_id))
			.filter(tag_usage_pattern::Column::CoOccurrenceTagId.eq(tag2_db_id))
			.one(db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		match existing {
			Some(pattern) => {
				// Update existing pattern
				let mut active_pattern: tag_usage_pattern::ActiveModel = pattern.into();
				active_pattern.occurrence_count = Set(active_pattern.occurrence_count.unwrap() + 1);
				active_pattern.last_used_together = Set(Utc::now());

				active_pattern
					.update(db)
					.await
					.map_err(|e| TagError::DatabaseError(e.to_string()))?;
			}
			None => {
				// Create new pattern
				let new_pattern = tag_usage_pattern::ActiveModel {
					id: NotSet,
					tag_id: Set(tag1_db_id),
					co_occurrence_tag_id: Set(tag2_db_id),
					occurrence_count: Set(1),
					last_used_together: Set(Utc::now()),
				};

				new_pattern
					.insert(db)
					.await
					.map_err(|e| TagError::DatabaseError(e.to_string()))?;
			}
		}

		Ok(())
	}

	/// Get frequently co-occurring tag pairs
	pub async fn get_frequent_co_occurrences(
		&self,
		min_count: i32,
	) -> Result<Vec<(Uuid, Uuid, i32)>, TagError> {
		let db = &*self.db;

		let patterns = tag_usage_pattern::Entity::find()
			.filter(tag_usage_pattern::Column::OccurrenceCount.gte(min_count))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let mut results = Vec::new();

		for pattern in patterns {
			// Get the tag UUIDs
			let tag1_model = tag::Entity::find()
				.filter(tag::Column::Id.eq(pattern.tag_id))
				.one(&*db)
				.await
				.map_err(|e| TagError::DatabaseError(e.to_string()))?;

			let tag2_model = tag::Entity::find()
				.filter(tag::Column::Id.eq(pattern.co_occurrence_tag_id))
				.one(&*db)
				.await
				.map_err(|e| TagError::DatabaseError(e.to_string()))?;

			if let (Some(tag1), Some(tag2)) = (tag1_model, tag2_model) {
				results.push((tag1.uuid, tag2.uuid, pattern.occurrence_count));
			}
		}

		Ok(results)
	}

	/// Calculate co-occurrence score between a tag and a set of context tags
	pub async fn calculate_co_occurrence_score(
		&self,
		candidate: &Tag,
		context_tags: &[Tag],
	) -> Result<f32, TagError> {
		let mut total_score = 0.0;
		let mut count = 0;

		for context_tag in context_tags {
			if let Some(co_occurrence_count) = self
				.get_co_occurrence_count(candidate.id, context_tag.id)
				.await?
			{
				total_score += co_occurrence_count as f32;
				count += 1;
			}
		}

		if count > 0 {
			Ok((total_score / count as f32) / 100.0) // Normalize to 0-1 range
		} else {
			Ok(0.0)
		}
	}

	async fn get_co_occurrence_count(
		&self,
		tag1_uuid: Uuid,
		tag2_uuid: Uuid,
	) -> Result<Option<i32>, TagError> {
		let db = &*self.db;

		// Get database IDs for both tags
		let tag1_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(tag1_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let tag2_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(tag2_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		if let (Some(tag1), Some(tag2)) = (tag1_model, tag2_model) {
			let pattern = tag_usage_pattern::Entity::find()
				.filter(tag_usage_pattern::Column::TagId.eq(tag1.id))
				.filter(tag_usage_pattern::Column::CoOccurrenceTagId.eq(tag2.id))
				.one(&*db)
				.await
				.map_err(|e| TagError::DatabaseError(e.to_string()))?;

			Ok(pattern.map(|p| p.occurrence_count))
		} else {
			Ok(None)
		}
	}
}

/// Manages the closure table for efficient hierarchy queries
pub struct TagClosureService {
	db: Arc<DatabaseConnection>,
}

impl TagClosureService {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		Self { db }
	}

	/// Add a new parent-child relationship and update closure table
	pub async fn add_relationship(
		&self,
		parent_db_id: i32,
		child_db_id: i32,
	) -> Result<(), TagError> {
		let db = &*self.db;

		let txn = db
			.begin()
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// 1. Add direct relationship (self to self with depth 0 if not exists)
		self.ensure_self_reference(&txn, parent_db_id).await?;
		self.ensure_self_reference(&txn, child_db_id).await?;

		// 2. Add direct parent-child relationship (depth = 1)
		let direct_closure = tag_closure::ActiveModel {
			ancestor_id: Set(parent_db_id),
			descendant_id: Set(child_db_id),
			depth: Set(1),
			path_strength: Set(1.0),
		};

		direct_closure
			.insert(&txn)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// 3. Add transitive relationships
		// For all ancestors of parent, create relationships to child and its descendants
		let parent_ancestors = tag_closure::Entity::find()
			.filter(tag_closure::Column::DescendantId.eq(parent_db_id))
			.all(&txn)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let child_descendants = tag_closure::Entity::find()
			.filter(tag_closure::Column::AncestorId.eq(child_db_id))
			.all(&txn)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Create all transitive relationships
		for ancestor in parent_ancestors {
			for descendant in &child_descendants {
				let new_depth = ancestor.depth + 1 + descendant.depth;
				let new_strength = ancestor.path_strength * descendant.path_strength;

				let transitive_closure = tag_closure::ActiveModel {
					ancestor_id: Set(ancestor.ancestor_id),
					descendant_id: Set(descendant.descendant_id),
					depth: Set(new_depth),
					path_strength: Set(new_strength),
				};

				// Insert if doesn't exist
				if let Err(_) = transitive_closure.insert(&txn).await {
					// Relationship might already exist, skip
				}
			}
		}

		txn.commit()
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(())
	}

	async fn ensure_self_reference(
		&self,
		db: &impl ConnectionTrait,
		tag_id: i32,
	) -> Result<(), TagError> {
		let self_ref = tag_closure::ActiveModel {
			ancestor_id: Set(tag_id),
			descendant_id: Set(tag_id),
			depth: Set(0),
			path_strength: Set(1.0),
		};

		// Insert if doesn't exist (ignore error if already exists)
		let _ = self_ref.insert(db).await;
		Ok(())
	}

	/// Remove a relationship and update closure table
	pub async fn remove_relationship(
		&self,
		parent_id: Uuid,
		child_id: Uuid,
	) -> Result<(), TagError> {
		let db = &*self.db;

		// Get database IDs for the tags
		let parent_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(parent_id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		let child_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(child_id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		let txn = db
			.begin()
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// 1. Remove the direct relationship from tag_relationship table
		tag_relationship::Entity::delete_many()
			.filter(tag_relationship::Column::ParentTagId.eq(parent_model.id))
			.filter(tag_relationship::Column::ChildTagId.eq(child_model.id))
			.exec(&txn)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// 2. Remove all closure table entries for this relationship
		// This includes both direct and transitive relationships
		tag_closure::Entity::delete_many()
			.filter(tag_closure::Column::AncestorId.eq(parent_model.id))
			.filter(tag_closure::Column::DescendantId.eq(child_model.id))
			.exec(&txn)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// 3. Rebuild closure table for affected relationships
		// This is a simplified approach - in a production system, you'd want to be more selective
		self.rebuild_closure_table(&txn).await?;

		txn.commit()
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(())
	}

	/// Rebuild the entire closure table from scratch
	async fn rebuild_closure_table<C: ConnectionTrait>(&self, db: &C) -> Result<(), TagError> {
		// Clear the closure table
		tag_closure::Entity::delete_many()
			.exec(db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Get all direct relationships
		let relationships = tag_relationship::Entity::find()
			.all(db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Rebuild closure table for each relationship
		for relationship in relationships {
			if relationship.relationship_type == "parent_child" {
				self.add_relationship(relationship.parent_tag_id, relationship.child_tag_id)
					.await?;
			}
		}

		Ok(())
	}

	/// Get all descendant tag IDs
	pub async fn get_all_descendants(&self, ancestor_uuid: Uuid) -> Result<Vec<Uuid>, TagError> {
		let db = &*self.db;

		// First get the database ID for this UUID
		let ancestor_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(ancestor_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		// Query closure table for all descendants (excluding self)
		let descendant_closures = tag_closure::Entity::find()
			.filter(tag_closure::Column::AncestorId.eq(ancestor_model.id))
			.filter(tag_closure::Column::Depth.gt(0))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Get the descendant tag UUIDs
		let descendant_db_ids: Vec<i32> = descendant_closures
			.into_iter()
			.map(|c| c.descendant_id)
			.collect();

		if descendant_db_ids.is_empty() {
			return Ok(Vec::new());
		}

		let descendant_models = tag::Entity::find()
			.filter(tag::Column::Id.is_in(descendant_db_ids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(descendant_models.into_iter().map(|m| m.uuid).collect())
	}

	/// Get all ancestor tag IDs
	pub async fn get_all_ancestors(&self, descendant_uuid: Uuid) -> Result<Vec<Uuid>, TagError> {
		let db = &*self.db;

		// First get the database ID for this UUID
		let descendant_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(descendant_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		// Query closure table for all ancestors (excluding self)
		let ancestor_closures = tag_closure::Entity::find()
			.filter(tag_closure::Column::DescendantId.eq(descendant_model.id))
			.filter(tag_closure::Column::Depth.gt(0))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		// Get the ancestor tag UUIDs
		let ancestor_db_ids: Vec<i32> = ancestor_closures
			.into_iter()
			.map(|c| c.ancestor_id)
			.collect();

		if ancestor_db_ids.is_empty() {
			return Ok(Vec::new());
		}

		let ancestor_models = tag::Entity::find()
			.filter(tag::Column::Id.is_in(ancestor_db_ids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(ancestor_models.into_iter().map(|m| m.uuid).collect())
	}

	/// Get direct children only
	pub async fn get_direct_children(&self, parent_uuid: Uuid) -> Result<Vec<Uuid>, TagError> {
		let db = &*self.db;

		// First get the database ID for this UUID
		let parent_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(parent_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		// Query closure table with depth = 1 (direct children only)
		let child_closures = tag_closure::Entity::find()
			.filter(tag_closure::Column::AncestorId.eq(parent_model.id))
			.filter(tag_closure::Column::Depth.eq(1))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		let child_db_ids: Vec<i32> = child_closures
			.into_iter()
			.map(|c| c.descendant_id)
			.collect();

		if child_db_ids.is_empty() {
			return Ok(Vec::new());
		}

		let child_models = tag::Entity::find()
			.filter(tag::Column::Id.is_in(child_db_ids))
			.all(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		Ok(child_models.into_iter().map(|m| m.uuid).collect())
	}

	/// Get path between two tags
	pub async fn get_path_between(
		&self,
		from_tag_uuid: Uuid,
		to_tag_uuid: Uuid,
	) -> Result<Option<Vec<Uuid>>, TagError> {
		let db = &*self.db;

		// Get database IDs
		let from_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(from_tag_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		let to_model = tag::Entity::find()
			.filter(tag::Column::Uuid.eq(to_tag_uuid))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?
			.ok_or(TagError::TagNotFound)?;

		// Check if there's a path in the closure table
		let path_closure = tag_closure::Entity::find()
			.filter(tag_closure::Column::AncestorId.eq(from_model.id))
			.filter(tag_closure::Column::DescendantId.eq(to_model.id))
			.one(&*db)
			.await
			.map_err(|e| TagError::DatabaseError(e.to_string()))?;

		if path_closure.is_none() {
			return Ok(None);
		}

		// For now, return just the endpoints (pathfinding would require more complex query)
		// TODO: Implement full path reconstruction if needed
		Ok(Some(vec![from_tag_uuid, to_tag_uuid]))
	}
}

/// Handles conflict resolution during tag synchronization
pub struct TagConflictResolver;

impl TagConflictResolver {
	pub fn new() -> Self {
		Self
	}

	/// Merge tag applications using union merge strategy
	pub async fn merge_tag_applications(
		&self,
		local_applications: Vec<TagApplication>,
		remote_applications: Vec<TagApplication>,
	) -> Result<TagMergeResult, TagError> {
		let mut merged_tags = HashMap::new();
		let mut conflicts = Vec::new();

		// Add all local applications
		for app in local_applications {
			merged_tags.insert(app.tag_id, app);
		}

		// Union merge with remote applications
		for remote_app in remote_applications {
			match merged_tags.get(&remote_app.tag_id) {
				Some(local_app) => {
					// Tag exists locally - merge intelligently
					let merged_app = self.merge_single_application(local_app, &remote_app)?;
					merged_tags.insert(remote_app.tag_id, merged_app);
				}
				None => {
					// New remote tag - add it
					merged_tags.insert(remote_app.tag_id, remote_app);
				}
			}
		}

		let merge_summary = format!(
			"Merged {} tag applications with {} conflicts",
			merged_tags.len(),
			conflicts.len()
		);

		Ok(TagMergeResult {
			merged_applications: merged_tags.into_values().collect(),
			conflicts,
			merge_summary,
		})
	}

	fn merge_single_application(
		&self,
		local: &TagApplication,
		remote: &TagApplication,
	) -> Result<TagApplication, TagError> {
		let mut merged = local.clone();

		// Use higher confidence value
		if remote.confidence > local.confidence {
			merged.confidence = remote.confidence;
		}

		// Merge instance attributes (union merge)
		for (key, value) in &remote.instance_attributes {
			if !merged.instance_attributes.contains_key(key) {
				merged
					.instance_attributes
					.insert(key.clone(), value.clone());
			}
		}

		// Prefer remote context if local doesn't have one
		if merged.applied_context.is_none() && remote.applied_context.is_some() {
			merged.applied_context = remote.applied_context.clone();
		}

		Ok(merged)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::domain::tag::TagSource;

	#[test]
	fn test_semantic_tag_creation() {
		let device_id = Uuid::new_v4();
		let tag = Tag::new("test-tag".to_string(), device_id);

		assert_eq!(tag.canonical_name, "test-tag");
		assert_eq!(tag.created_by_device, device_id);
		assert_eq!(tag.tag_type, TagType::Standard);
		assert_eq!(tag.privacy_level, PrivacyLevel::Normal);
	}

	#[test]
	fn test_tag_name_matching() {
		let device_id = Uuid::new_v4();
		let mut tag = Tag::new("JavaScript".to_string(), device_id);
		tag.formal_name = Some("JavaScript Programming Language".to_string());
		tag.abbreviation = Some("JS".to_string());
		tag.add_alias("ECMAScript".to_string());

		assert!(tag.matches_name("JavaScript"));
		assert!(tag.matches_name("js")); // Case insensitive
		assert!(tag.matches_name("ECMAScript"));
		assert!(tag.matches_name("JavaScript Programming Language"));
		assert!(!tag.matches_name("Python"));
	}

	#[test]
	fn test_tag_application_creation() {
		let tag_id = Uuid::new_v4();
		let device_id = Uuid::new_v4();

		let user_app = TagApplication::user_applied(tag_id, device_id);
		assert_eq!(user_app.source, TagSource::User);
		assert_eq!(user_app.confidence, 1.0);

		let ai_app = TagApplication::ai_applied(tag_id, 0.85, device_id);
		assert_eq!(ai_app.source, TagSource::AI);
		assert_eq!(ai_app.confidence, 0.85);
		assert!(ai_app.is_high_confidence());
	}
}
