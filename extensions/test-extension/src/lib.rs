//! Test Extension
//!
//! Demonstrates the Spacedrive extension SDK using procedural macros to simplify
//! extension development by abstracting FFI and state management details.

use spacedrive_sdk::prelude::*;
use spacedrive_sdk::{extension, job, model, task};

// Extension Definition
// The #[extension] macro generates plugin_init() and plugin_cleanup().
// List jobs in the jobs parameter for automatic registration.

#[extension(
	id = "test-extension",
	name = "Test Extension",
	version = "0.1.0",
	jobs = [test_counter, catalog_files, tag_files],
	models = [CatalogEntry],
)]
struct TestExtension;

// Job State Definition
// State is automatically serialized/deserialized for checkpointing.

#[derive(Serialize, Deserialize, Default)]
pub struct CounterState {
	pub current: u32,
	pub target: u32,
	pub processed: Vec<String>,
}

// Job Implementation
// The #[job] macro handles FFI bindings, serialization, and error handling.
// The name parameter enables automatic registration (extension-id:name format).

#[job(name = "counter")]
fn test_counter(ctx: &JobContext, state: &mut CounterState) -> Result<()> {
	ctx.log(&format!(
		"Starting counter (current: {}, target: {})",
		state.current, state.target
	));

	while state.current < state.target {
		// Check for interruption signals
		if ctx.check_interrupt_sync() {
			ctx.log("Interrupted, saving state...");
			ctx.checkpoint(state)?;
			return Err(Error::OperationFailed("Interrupted".into()));
		}

		// Process work unit
		state.current += 1;
		state.processed.push(format!("item_{}", state.current));

		// Update progress reporting
		let progress = state.current as f32 / state.target as f32;
		ctx.report_progress(
			progress,
			&format!("Counted {}/{}", state.current, state.target),
		);

		// Track processed items
		ctx.increment_items(1);

		// Periodic checkpoint for recovery
		if state.current % 10 == 0 {
			ctx.checkpoint(state)?;
		}
	}

	ctx.log(&format!(
		"Completed processing {} items",
		state.processed.len()
	));

	Ok(())
}

// Catalog Job
// Exercises the data host functions: records, bytes, sidecars, models, the
// clock and entropy. Reads every file of the given extensions in the
// library, writes a `digest` sidecar per content and a CatalogEntry model
// per file, and skips files already digested so a second run is a no-op.

/// One catalogued file, a standalone model keyed by its own id.
#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
pub struct CatalogEntry {
	pub id: Uuid,
	pub record: Uuid,
	pub name: String,
	pub size: u64,
	pub first_bytes: Vec<u8>,
	pub catalogued_at: chrono::DateTime<chrono::Utc>,
	pub seen_twice: bool,
}

/// What the `digest` sidecar holds for one content identity.
#[derive(Serialize, Deserialize)]
pub struct Digest {
	pub size: usize,
	pub first_bytes: Vec<u8>,
	pub record_name: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CatalogState {
	pub extensions: Vec<String>,
	pub digested: usize,
	pub skipped: usize,
	pub entries: usize,
}

/// Read one record's bytes and describe them. The timeout is generous; the
/// retry covers a transient read failure.
#[task(retries = 1, timeout_ms = 10000)]
async fn digest_record(ctx: TaskContext, record: Record) -> TaskResult<Digest> {
	let bytes = record.read().await?;
	let _ = ctx.task_id();
	Ok(Digest {
		size: bytes.len(),
		first_bytes: bytes.iter().take(4).copied().collect(),
		record_name: record.name().to_string(),
	})
}

#[job(name = "catalog")]
async fn catalog_files(ctx: &JobContext, state: &mut CatalogState) -> Result<()> {
	let records = ctx
		.vdfs()
		.query_records()
		.with_extensions(state.extensions.iter().cloned())
		.collect()
		.await?;
	ctx.log(&format!("Cataloguing {} records", records.len()));

	for record in records {
		let Some(content_uuid) = record.content_uuid() else {
			ctx.add_warning(&format!("{} has no content identity yet", record.name()));
			state.skipped += 1;
			continue;
		};
		if ctx.sidecar_exists(content_uuid, "digest")? {
			let existing: Digest = ctx.read_sidecar(content_uuid, "digest").await?;
			ctx.log(&format!(
				"{} already digested ({} bytes)",
				record.name(),
				existing.size
			));
			state.skipped += 1;
			continue;
		}
		let digest = ctx.run(digest_record, record.clone()).await?;
		ctx.save_sidecar(content_uuid, "digest", &digest).await?;
		state.digested += 1;

		let entry = CatalogEntry {
			id: Uuid::new_v4(),
			record: record.id(),
			name: record.name().to_string(),
			size: digest.size as u64,
			first_bytes: digest.first_bytes.clone(),
			catalogued_at: spacedrive_sdk::clock::now(),
			seen_twice: false,
		};
		let id = entry.id;
		ctx.vdfs().create_model(entry).await?;
		ctx.vdfs()
			.update_model(id, |mut e: CatalogEntry| {
				e.seen_twice = true;
				Ok(e)
			})
			.await?;
		state.entries += 1;
		ctx.increment_items(1);
	}

	let all = ctx.vdfs().query_models::<CatalogEntry>().collect().await?;
	ctx.log(&format!(
		"Catalog holds {} entries, {} digested this run, {} skipped",
		all.len(),
		state.digested,
		state.skipped
	));
	ctx.checkpoint(state)?;
	Ok(())
}

// Tag Job
// Exercises tags, custom fields and job dispatch: tags every file of the
// given extensions, records its size as a custom field and reads it back,
// removes the tag from the first file again, checks the host refuses a
// namespace the manifest does not grant, then queues a counter job.

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TagState {
	pub extensions: Vec<String>,
	pub tagged: usize,
	pub dispatched: Option<Uuid>,
}

#[job(name = "tag")]
async fn tag_files(ctx: &JobContext, state: &mut TagState) -> Result<()> {
	let records = ctx
		.vdfs()
		.query_records()
		.with_extensions(state.extensions.iter().cloned())
		.collect()
		.await?;

	for record in &records {
		let tag = ctx.vdfs().add_tag(record.id(), "Catalog/Digested").await?;
		ctx.log(&format!(
			"tagged {} with {} ({})",
			record.name(),
			tag.path,
			tag.id
		));
		ctx.vdfs()
			.update_custom_field(record.id(), "test.size", record.size)
			.await?;
		let stored: Option<u64> = record.custom_field("test.size").await?;
		if stored != record.size {
			return Err(Error::OperationFailed(format!(
				"custom field for {} read back as {stored:?}, wrote {:?}",
				record.name(),
				record.size
			)));
		}
		state.tagged += 1;
	}

	if let Some(first) = records.first() {
		if let Some(content_uuid) = first.content_uuid() {
			ctx.vdfs()
				.add_tag_to_content(content_uuid, "Catalog/Bytes")
				.await?;
		}
		let removed = ctx
			.vdfs()
			.remove_tag(first.id(), "Catalog/Digested")
			.await?;
		ctx.log(&format!(
			"removed {} from {}",
			removed.map(|t| t.path).unwrap_or_default(),
			first.name()
		));
		let none = ctx.vdfs().remove_tag(first.id(), "Never/Defined").await?;
		ctx.log(&format!("removing an undefined tag answers {none:?}"));

		match ctx
			.vdfs()
			.update_custom_field(first.id(), "other.size", 1)
			.await
		{
			Err(Error::PermissionDenied(reason)) => ctx.log(&format!("refused: {reason}")),
			other => {
				return Err(Error::OperationFailed(format!(
					"an ungranted namespace was not refused: {other:?}"
				)))
			}
		}
		let unset: Option<u64> = first.custom_field("test.missing").await?;
		ctx.log(&format!("an unset field reads as {unset:?}"));
	}

	let still_tagged = ctx
		.vdfs()
		.query_records()
		.with_tag("Catalog/Digested")
		.collect()
		.await?;
	ctx.log(&format!(
		"{} records still carry Catalog/Digested",
		still_tagged.len()
	));

	let job_id = ctx
		.jobs()
		.dispatch(
			"counter",
			&CounterState {
				current: 0,
				target: 5,
				processed: Vec::new(),
			},
		)
		.execute()
		.await?;
	ctx.log(&format!("dispatched counter as {job_id}"));
	state.dispatched = Some(job_id);
	ctx.checkpoint(state)?;
	Ok(())
}
