//! VDFS operations - records, models, tags, collections
//!
//! Records are the files the library's source stores hold; see
//! [`crate::types::Record`]. Reading them needs the manifest's `read_records`
//! grant. Model and tag operations are stubs until a host function backs
//! them.

use crate::types::*;
use serde::{de::DeserializeOwned, Serialize};

/// VDFS context for querying and manipulating data
pub struct VdfsContext;

impl VdfsContext {
	/// Query records (files and directories) across the library's sources.
	pub fn query_records(&self) -> RecordQuery {
		RecordQuery::default()
	}

	/// One record by uuid, from whichever source store holds it.
	pub async fn get_record(&self, uuid: Uuid) -> Result<Record> {
		crate::ffi::op_json("records.get", &serde_json::json!({ "uuid": uuid }))
	}

	/// A record's bytes, through the source's resolved path.
	pub async fn read_record(&self, uuid: Uuid) -> Result<Vec<u8>> {
		let payload = serde_json::to_vec(&serde_json::json!({ "uuid": uuid }))?;
		crate::ffi::op("records.read", &payload)
	}

	/// Whether a sidecar of one of this extension's kinds exists for the
	/// content.
	pub fn sidecar_exists(&self, content_uuid: Uuid, kind: &str) -> Result<bool> {
		crate::ffi::op_json(
			"sidecars.exists",
			&serde_json::json!({ "content_uuid": content_uuid, "kind": kind }),
		)
	}

	/// A sidecar this extension may read (`read_sidecars` or its own
	/// `write_sidecars` kinds), decoded from JSON.
	pub async fn read_sidecar<T: DeserializeOwned>(
		&self,
		content_uuid: Uuid,
		kind: &str,
	) -> Result<T> {
		crate::ffi::op_json(
			"sidecars.read",
			&serde_json::json!({ "content_uuid": content_uuid, "kind": kind }),
		)
	}

	/// Write a sidecar of one of this extension's `write_sidecars` kinds, as
	/// JSON, replacing any earlier one for the same content.
	pub async fn write_sidecar<T: Serialize>(
		&self,
		content_uuid: Uuid,
		kind: &str,
		data: &T,
	) -> Result<()> {
		let _: serde_json::Value = crate::ffi::op_json(
			"sidecars.write",
			&serde_json::json!({ "content_uuid": content_uuid, "kind": kind, "data": data }),
		)?;
		Ok(())
	}

	/// Query extension models
	pub fn query_models<T: ExtensionModel>(&self) -> ModelQuery<T> {
		ModelQuery {
			filtered: false,
			limit: None,
			_phantom: std::marker::PhantomData,
		}
	}

	/// Get model scoped to content_identity
	pub async fn get_model_by_content<T: ExtensionModel>(&self, content_uuid: Uuid) -> Result<T> {
		crate::ffi::op_json(
			"models.get",
			&serde_json::json!({ "model": T::MODEL_TYPE, "content_uuid": content_uuid }),
		)
	}

	/// Create (or replace) the model describing one content identity.
	pub async fn create_model_for_content<T: ExtensionModel>(
		&self,
		content_uuid: Uuid,
		model: T,
	) -> Result<()> {
		let _: serde_json::Value = crate::ffi::op_json(
			"models.put",
			&serde_json::json!({
				"model": T::MODEL_TYPE,
				"content_uuid": content_uuid,
				"data": model,
			}),
		)?;
		Ok(())
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
		let current = self.get_model_by_content::<T>(content_uuid).await?;
		self.create_model_for_content(content_uuid, f(current)?)
			.await
	}

	/// Create (or replace) a standalone model, keyed by its own uuid.
	pub async fn create_model<T: ExtensionModel>(&self, model: T) -> Result<()> {
		let _: serde_json::Value = crate::ffi::op_json(
			"models.put",
			&serde_json::json!({
				"model": T::MODEL_TYPE,
				"uuid": model.uuid(),
				"data": model,
			}),
		)?;
		Ok(())
	}

	/// Get standalone model by UUID
	pub async fn get_model<T: ExtensionModel>(&self, uuid: Uuid) -> Result<T> {
		crate::ffi::op_json(
			"models.get",
			&serde_json::json!({ "model": T::MODEL_TYPE, "uuid": uuid }),
		)
	}

	/// Update standalone model
	pub async fn update_model<T: ExtensionModel, F>(&self, uuid: Uuid, f: F) -> Result<()>
	where
		F: FnOnce(T) -> Result<T>,
	{
		let current = self.get_model::<T>(uuid).await?;
		self.create_model(f(current)?).await
	}

	/// Add tag to content (all entries with this content get the tag)
	pub async fn add_tag_to_content(&self, content_uuid: Uuid, tag: &str) -> Result<()> {
		Err(Error::Unsupported("add_tag_to_content".into()))
	}

	/// Add tag to model
	pub async fn add_tag_to_model(&self, model_uuid: Uuid, tag: &str) -> Result<()> {
		Err(Error::Unsupported("add_tag_to_model".into()))
	}

	/// Add tag to one record
	pub async fn add_tag(&self, record_uuid: Uuid, tag: &str) -> Result<()> {
		Err(Error::Unsupported("add_tag".into()))
	}

	/// Update custom field in UserMetadata
	pub async fn update_custom_field<T: Serialize>(
		&self,
		record_uuid: Uuid,
		field: &str,
		value: T,
	) -> Result<()> {
		Err(Error::Unsupported("update_custom_field".into()))
	}

	/// Check if a path is in user-granted scope
	pub fn in_granted_scope(&self, path: &str) -> bool {
		panic!("WASM host call")
	}
}

/// Record query builder.
///
/// Filters narrow the records a `collect` returns; the host answers from the
/// source stores of the job's library, newest sources first, in the order
/// each store walks its tree.
#[derive(Default, Serialize)]
pub struct RecordQuery {
	source: Option<Uuid>,
	scope: Option<String>,
	extensions: Option<Vec<String>>,
	tag: Option<String>,
	limit: Option<usize>,
}

impl RecordQuery {
	/// Only records of one source.
	pub fn in_source(mut self, source: Uuid) -> Self {
		self.source = Some(source);
		self
	}

	/// Only records beneath a source-relative directory; "" is the root.
	pub fn in_location(mut self, path: impl Into<String>) -> Self {
		self.scope = Some(path.into());
		self
	}

	/// Only files with one of these extensions (case-insensitive).
	pub fn with_extensions<I, S>(mut self, extensions: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<String>,
	{
		self.extensions = Some(
			extensions
				.into_iter()
				.map(|e| e.into().to_lowercase())
				.collect(),
		);
		self
	}

	/// Only files whose extension `T` names; see [`Image`] and [`Pdf`].
	pub fn of_type<T: FileType>(self) -> Self {
		self.with_extensions(T::EXTENSIONS.iter().copied())
	}

	/// Only records carrying this tag. No host side yet: a query with a tag
	/// filter is refused rather than answered without it.
	pub fn with_tag(mut self, tag: &str) -> Self {
		self.tag = Some(tag.to_string());
		self
	}

	/// At most this many records; the host caps it at 10,000.
	pub fn limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}

	pub fn where_metadata(self, field: &str, predicate: FieldPredicate) -> Self {
		panic!("Filter by metadata field")
	}

	pub async fn first(self) -> Result<Option<Record>> {
		Ok(self.limit(1).collect().await?.into_iter().next())
	}

	pub async fn collect(self) -> Result<Vec<Record>> {
		crate::ffi::op_json("records.query", &self)
	}
}

/// A file type a query can ask for by name.
pub trait FileType {
	const EXTENSIONS: &'static [&'static str];
}

impl FileType for Image {
	const EXTENSIONS: &'static [&'static str] = &[
		"jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "tiff", "tif", "bmp", "avif", "raw",
		"cr2", "nef", "dng", "arw",
	];
}

impl FileType for Pdf {
	const EXTENSIONS: &'static [&'static str] = &["pdf"];
}

/// Model query builder.
///
/// Only an unfiltered listing has a host side; a query with a field or
/// semantic filter is refused rather than answered without it.
pub struct ModelQuery<T> {
	filtered: bool,
	limit: Option<usize>,
	_phantom: std::marker::PhantomData<T>,
}

impl<T: ExtensionModel> ModelQuery<T> {
	pub fn where_field(mut self, field: &str, predicate: FieldPredicate) -> Self {
		self.filtered = true;
		self
	}

	pub fn where_json_field(mut self, path: &str, predicate: FieldPredicate) -> Self {
		self.filtered = true;
		self
	}

	pub fn search_semantic(mut self, field: &str, query: SemanticQuery) -> Self {
		self.filtered = true;
		self
	}

	pub fn limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}

	pub async fn first(self) -> Result<Option<T>> {
		Ok(self.limit(1).collect().await?.into_iter().next())
	}

	pub async fn collect(self) -> Result<Vec<T>> {
		if self.filtered {
			return Err(Error::Unsupported(
				"model queries cannot filter yet; list and filter in the guest".into(),
			));
		}
		crate::ffi::op_json(
			"models.list",
			&serde_json::json!({ "model": T::MODEL_TYPE, "limit": self.limit }),
		)
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
