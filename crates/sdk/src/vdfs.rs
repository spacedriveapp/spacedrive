//! VDFS operations - queries, models, tags, collections
//!
//! Stubs for type-checking. Implementation will call WASM host functions.

use crate::types::*;
use serde::{de::DeserializeOwned, Serialize};

/// VDFS context for querying and manipulating data
pub struct VdfsContext;

impl VdfsContext {
	/// Query entries (files/directories)
	pub fn query_entries(&self) -> EntryQuery {
		EntryQuery::default()
	}

	/// Get a specific entry by UUID
	pub async fn get_entry(&self, uuid: Uuid) -> Result<Entry> {
		Err(Error::Unsupported("get_entry".into()))
	}

	/// Query extension models
	pub fn query_models<T: ExtensionModel>(&self) -> ModelQuery<T> {
		ModelQuery {
			_phantom: std::marker::PhantomData,
		}
	}

	/// Get model scoped to content_identity
	pub async fn get_model_by_content<T: ExtensionModel>(&self, content_uuid: Uuid) -> Result<T> {
		Err(Error::Unsupported("get_model_by_content".into()))
	}

	/// Create model scoped to content_identity
	pub async fn create_model_for_content<T: ExtensionModel>(
		&self,
		content_uuid: Uuid,
		model: T,
	) -> Result<()> {
		Err(Error::Unsupported("create_model_for_content".into()))
	}

	/// Update model scoped to content
	pub async fn update_model_by_content<T: ExtensionModel, F>(
		&self,
		content_uuid: Uuid,
		f: F,
	) -> Result<()>
	where
		F: FnOnce(T) -> Result<T>,
	{
		Err(Error::Unsupported("update_model_by_content".into()))
	}

	/// Create standalone model
	pub async fn create_model<T: ExtensionModel>(&self, model: T) -> Result<()> {
		Err(Error::Unsupported("create_model".into()))
	}

	/// Get standalone model by UUID
	pub async fn get_model<T: ExtensionModel>(&self, uuid: Uuid) -> Result<T> {
		Err(Error::Unsupported("get_model".into()))
	}

	/// Update standalone model
	pub async fn update_model<T: ExtensionModel, F>(&self, uuid: Uuid, f: F) -> Result<()>
	where
		F: FnOnce(T) -> Result<T>,
	{
		Err(Error::Unsupported("update_model".into()))
	}

	/// Add tag to content (all entries with this content get the tag)
	pub async fn add_tag_to_content(&self, content_uuid: Uuid, tag: &str) -> Result<()> {
		Err(Error::Unsupported("add_tag_to_content".into()))
	}

	/// Add tag to model
	pub async fn add_tag_to_model(&self, model_uuid: Uuid, tag: &str) -> Result<()> {
		Err(Error::Unsupported("add_tag_to_model".into()))
	}

	/// Add tag to specific entry
	pub async fn add_tag(&self, metadata_id: i32, tag: &str) -> Result<()> {
		Err(Error::Unsupported("add_tag".into()))
	}

	/// Update custom field in UserMetadata
	pub async fn update_custom_field<T: Serialize>(
		&self,
		entry_uuid: Uuid,
		field: &str,
		value: T,
	) -> Result<()> {
		Err(Error::Unsupported("update_custom_field".into()))
	}

	/// Check if entry is in user-granted scope
	pub fn in_granted_scope(&self, path: &str) -> bool {
		panic!("WASM host call")
	}
}

/// Entry query builder
#[derive(Default)]
pub struct EntryQuery {
	// Query state
}

impl EntryQuery {
	pub fn in_location(self, path: impl Into<String>) -> Self {
		panic!("Build query")
	}

	pub fn of_type<T>(self) -> Self {
		panic!("Filter by type")
	}

	pub fn where_content_id(self, content_uuid: Uuid) -> Self {
		panic!("Filter by content")
	}

	pub fn on_this_device(self) -> Self {
		panic!("Filter to local entries")
	}

	pub fn with_tag(self, tag: &str) -> Self {
		panic!("Filter by tag")
	}

	pub fn with_sidecar(self, kind: &str) -> Self {
		panic!("Filter by sidecar existence")
	}

	pub fn where_metadata(self, field: &str, predicate: FieldPredicate) -> Self {
		panic!("Filter by metadata field")
	}

	pub async fn first(self) -> Result<Option<Entry>> {
		Err(Error::Unsupported("first".into()))
	}

	pub async fn collect(self) -> Result<Vec<Entry>> {
		Err(Error::Unsupported("collect".into()))
	}

	pub fn map<F, T>(self, f: F) -> MappedQuery<T>
	where
		F: Fn(Entry) -> T,
	{
		panic!("Map results")
	}
}

/// Mapped query results
pub struct MappedQuery<T> {
	_phantom: std::marker::PhantomData<T>,
}

impl<T> MappedQuery<T> {
	pub async fn collect(self) -> Result<Vec<T>> {
		Err(Error::Unsupported("collect".into()))
	}
}

/// Model query builder
pub struct ModelQuery<T> {
	_phantom: std::marker::PhantomData<T>,
}

impl<T: ExtensionModel> ModelQuery<T> {
	pub fn where_field(self, field: &str, predicate: FieldPredicate) -> Self {
		panic!("Filter by field")
	}

	pub fn where_json_field(self, path: &str, predicate: FieldPredicate) -> Self {
		panic!("Filter by JSON field")
	}

	pub fn search_semantic(self, field: &str, query: SemanticQuery) -> Self {
		panic!("Semantic search")
	}

	pub async fn first(self) -> Result<Option<T>> {
		Err(Error::Unsupported("first".into()))
	}

	pub async fn collect(self) -> Result<Vec<T>> {
		Err(Error::Unsupported("collect".into()))
	}
}

/// Field predicate for queries
pub enum FieldPredicate {
	Equals(serde_json::Value),
	Contains(String),
	IsNotNull,
}

pub fn equals<T: Serialize>(value: T) -> FieldPredicate {
	FieldPredicate::Equals(serde_json::to_value(value).unwrap())
}

pub fn contains(value: impl Into<String>) -> FieldPredicate {
	FieldPredicate::Contains(value.into())
}

pub fn is_not_null() -> FieldPredicate {
	FieldPredicate::IsNotNull
}

/// Semantic query
pub enum SemanticQuery {
	SimilarTo(String),
}

pub fn similar_to(query: impl Into<String>) -> SemanticQuery {
	SemanticQuery::SimilarTo(query.into())
}

// Import ExtensionModel trait from models module
use crate::models::ExtensionModel;
