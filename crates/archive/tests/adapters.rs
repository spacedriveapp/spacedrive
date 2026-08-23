//! Every bundled adapter's manifest must still produce a working index under
//! the record table. The JSONL protocol and `adapter.toml` format are unchanged, so
//! these read the real manifests rather than fixtures.

use sd_archive::adapter::script::ScriptAdapter;
use sd_archive::adapter::{Adapter, AdapterRegistry};
use sd_store::record::facet_table;
use sd_store::SourceManager;

fn adapters_dir() -> std::path::PathBuf {
	std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.expect("crates/")
		.parent()
		.expect("workspace root")
		.join("adapters")
}

fn bundled_adapters() -> Vec<(String, std::path::PathBuf)> {
	let mut found: Vec<_> = std::fs::read_dir(adapters_dir())
		.expect("adapters directory")
		.flatten()
		.map(|entry| entry.path())
		.filter(|path| path.join("adapter.toml").exists())
		.map(|path| {
			let name = path
				.file_name()
				.expect("adapter name")
				.to_string_lossy()
				.into_owned();
			(name, path)
		})
		.collect();
	found.sort();
	found
}

#[test]
fn every_bundled_adapter_is_present() {
	let adapters = bundled_adapters();
	assert!(
		adapters.len() >= 11,
		"expected the bundled adapter set, found {}: {:?}",
		adapters.len(),
		adapters.iter().map(|(n, _)| n).collect::<Vec<_>>()
	);
}

#[tokio::test]
async fn every_bundled_adapter_builds_a_usable_index() {
	let dir = tempfile::tempdir().expect("tempdir");
	let manager = SourceManager::new(dir.path().join("sources"));

	for (name, path) in bundled_adapters() {
		let adapter = ScriptAdapter::from_dir(&path)
			.unwrap_or_else(|e| panic!("{name}: manifest failed to load: {e}"));
		let schema = adapter.schema().clone();

		manager
			.create(&name, &schema)
			.await
			.unwrap_or_else(|e| panic!("{name}: index creation failed: {e}"));

		let db = manager
			.open(&name)
			.await
			.unwrap_or_else(|e| panic!("{name}: index failed to open: {e}"));

		// The record table and one facet table per declared model.
		for model in schema.models.keys() {
			let table = facet_table(model);
			let exists: Option<String> = sqlx::query_scalar(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
			)
			.bind(&table)
			.fetch_optional(db.pool())
			.await
			.expect("sqlite_master");

			assert!(exists.is_some(), "{name}: missing facet table {table}");
		}

		// The primary type round-trips through ingest and search.
		db.begin_sync().await.expect("epoch");
		db.upsert(
			&schema.search.primary_model,
			"probe-1",
			&probe_fields(&schema),
		)
		.await
		.unwrap_or_else(|e| panic!("{name}: upsert failed: {e}"));

		let items = db
			.list_items(10, 0)
			.await
			.unwrap_or_else(|e| panic!("{name}: list_items failed: {e}"));
		assert_eq!(items.len(), 1, "{name}: expected the probe record");
		assert_eq!(items[0].external_id, "probe-1");

		let hits = db
			.fts_search("spacedriveprobe", 10, None)
			.await
			.unwrap_or_else(|e| panic!("{name}: fts_search failed: {e}"));
		assert_eq!(hits.len(), 1, "{name}: probe record was not indexed");
	}
}

/// A registered adapter is listed with what its manifest declares.
#[test]
fn registry_lists_registered_adapters() {
	let registry = AdapterRegistry::new();

	let (name, path) = bundled_adapters()
		.into_iter()
		.next()
		.expect("a bundled adapter");
	let script = ScriptAdapter::from_dir(&path)
		.unwrap_or_else(|e| panic!("{name}: manifest failed to load: {e}"));
	let id = script.id().to_string();
	let declared_tier = script.trust_tier();
	registry.register(std::sync::Arc::new(script));

	let info = registry
		.list()
		.into_iter()
		.find(|i| i.id == id)
		.unwrap_or_else(|| panic!("adapter {id} not listed"));
	assert_eq!(info.trust_tier, declared_tier);
	assert!(!info.data_type.is_empty());
}

/// Fill every declared field on the primary model with a distinctive token so
/// the record is findable whichever field the search contract indexes.
fn probe_fields(schema: &sd_store::DataTypeSchema) -> serde_json::Value {
	use sd_store::FieldType;

	let model = &schema.models[&schema.search.primary_model];
	let mut fields = serde_json::Map::new();

	for (field_name, field_type) in &model.fields {
		let value = match field_type {
			FieldType::Integer | FieldType::Boolean => serde_json::json!(1),
			FieldType::Float => serde_json::json!(1.0),
			FieldType::Datetime => serde_json::json!("2026-07-28T12:00:00Z"),
			_ => serde_json::json!("spacedriveprobe"),
		};
		fields.insert(field_name.clone(), value);
	}

	serde_json::Value::Object(fields)
}
