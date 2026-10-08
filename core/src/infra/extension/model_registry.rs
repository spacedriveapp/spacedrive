//! The data models extensions declared, and the store that holds them.
//!
//! `plugin_init` registers each `#[model]` through `register_model`; the
//! registry turns an extension's models into one `DataTypeSchema`, which is
//! what opens the extension's store in a library. The store is an ordinary
//! `sd_store` database under `<library>/extensions/<extension id>/`: the
//! same record and facet shape as a source store, owned by the extension,
//! and the thing the extension-kinds plan registers as an app-owned source
//! when that phase lands.

use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use indexmap::IndexMap;
use sd_store::{DataTypeSchema, FieldType, ModelDef, SourceDb, SourceManager};
use serde::Deserialize;

/// A model as the SDK's `#[model]` macro declares it.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelDefinition {
	pub name: String,
	pub fields: IndexMap<String, FieldType>,
}

#[derive(Default)]
pub struct ExtensionModelRegistry {
	models: RwLock<HashMap<String, Vec<ModelDefinition>>>,
}

impl ExtensionModelRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	/// Accept a model a guest declared.
	///
	/// The name becomes a table and each field a column, spliced into DDL
	/// and into the facet reader's SQL, and the guest can hand `register_model`
	/// any JSON it likes, so every identifier is checked here rather than
	/// trusted from the macro.
	pub fn register(&self, extension_id: &str, model: ModelDefinition) -> Result<(), String> {
		let identifier = |s: &str| {
			!s.is_empty()
				&& !s.starts_with(|c: char| c.is_ascii_digit())
				&& s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
		};
		if !identifier(&model.name) {
			return Err(format!(
				"model name {:?} is not a valid table name",
				model.name
			));
		}
		if let Some(field) = model.fields.keys().find(|f| !identifier(f)) {
			return Err(format!(
				"model {} field {field:?} is not a valid column name",
				model.name
			));
		}
		if model.name == CUSTOM_FIELD_MODEL {
			return Err(format!(
				"model name {CUSTOM_FIELD_MODEL:?} is reserved for the extension's custom fields"
			));
		}
		if model.fields.contains_key("record_uuid") {
			return Err(format!(
				"model {} field \"record_uuid\" is the facet table's key column",
				model.name
			));
		}
		let mut models = self.models.write().unwrap_or_else(|p| p.into_inner());
		let list = models.entry(extension_id.to_string()).or_default();
		if list.iter().any(|m| m.name == model.name) {
			return Err(format!("model {} is already registered", model.name));
		}
		tracing::info!(extension = extension_id, model = %model.name, "Registered extension model");
		list.push(model);
		Ok(())
	}

	pub fn unregister_extension(&self, extension_id: &str) {
		self.models
			.write()
			.unwrap_or_else(|p| p.into_inner())
			.remove(extension_id);
	}

	/// The schema of an extension's store: the models it declared plus the
	/// custom field model every extension gets, so an extension with no
	/// models of its own still has somewhere to put a field.
	pub fn schema_for(&self, extension_id: &str) -> DataTypeSchema {
		let models = self.models.read().unwrap_or_else(|p| p.into_inner());
		let declared = models.get(extension_id).map(Vec::as_slice).unwrap_or(&[]);
		let mut defs = IndexMap::new();
		for model in declared {
			defs.insert(
				model.name.clone(),
				ModelDef {
					fields: model.fields.clone(),
					relations: Default::default(),
				},
			);
		}
		defs.insert(CUSTOM_FIELD_MODEL.to_string(), custom_field_model());
		DataTypeSchema {
			data_type: sd_store::schema::DataTypeMeta {
				id: extension_id.to_string(),
				name: extension_id.to_string(),
				icon: None,
			},
			search: sd_store::schema::SearchContract {
				primary_model: declared
					.first()
					.map_or(CUSTOM_FIELD_MODEL.to_string(), |m| m.name.clone()),
				title: "_derived.title".to_string(),
				preview: "_derived.none".to_string(),
				subtitle: None,
				search_fields: Vec::new(),
				date_field: None,
			},
			models: defs,
		}
	}
}

/// The model holding an extension's custom fields: one row per field on a
/// record, keyed `<record uuid>:<namespace>:<name>`, the value kept as JSON
/// text so any shape round-trips. The source store carries no per-record
/// extension metadata, so fields live in the extension's own store.
pub const CUSTOM_FIELD_MODEL: &str = "custom_field";

fn custom_field_model() -> ModelDef {
	ModelDef {
		fields: [
			("record", FieldType::String),
			("namespace", FieldType::String),
			("name", FieldType::String),
			("value", FieldType::Text),
		]
		.into_iter()
		.map(|(name, ty)| (name.to_string(), ty))
		.collect(),
		relations: Default::default(),
	}
}

/// Open an extension's store in a library, creating it on first use and
/// widening its facets when the extension declared new fields.
pub async fn open_extension_store(
	library_path: &Path,
	extension_id: &str,
	schema: &DataTypeSchema,
) -> anyhow::Result<SourceDb> {
	let manager = SourceManager::new(library_path.join("extensions"));
	manager.create(extension_id, schema).await?;
	let (db, _) = manager.open_with_migration(extension_id, schema).await?;
	Ok(db)
}
