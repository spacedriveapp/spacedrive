//! # Tags: definitions and assertions
//!
//! A source carries everything needed to read its own tags. Definitions and
//! assertions both live in `source.db`, so a store arriving somewhere new, on
//! a drive or as a delivered replica, presents named, colored, hierarchical
//! tags rather than opaque uuids. `docs/core/design/tags-and-assertions.md`
//! carries the model; `docs/plans/2026-09-17-tags-on-source-stores.md` the
//! execution.
//!
//! A definition is the tag itself. It is copied into every store that uses
//! it and reconciled by row-level last-writer-wins on its HLC. An assertion
//! is the claim that a definition applies to a record or to content, and it
//! is append-only: removal is a row with `asserted = 0`, never a delete, so a
//! store that spent a month detached cannot resurrect a tag removed while it
//! was away. The state of a tag on a record is the latest assertion by HLC,
//! device uuid breaking ties.
//!
//! The assertion primary key `(tag_uuid, record_uuid, hlc, device_uuid)`
//! doubles as the merge dedupe key: delivery inserts with
//! `ON CONFLICT DO NOTHING`, so replaying a batch changes nothing. Neither
//! table declares a foreign key to `record`, matching `record_overlay`: both
//! belong to the half of a store no ingest can rebuild, and their rows
//! outlive any generation row.

use std::collections::{HashMap, HashSet};

use sqlx::SqlitePool;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::db::{SourceDb, Stamp};
use crate::error::{Error, Result};

/// Namespace for tag slugs. Fixed forever: the slug of a path must come out
/// identical on every device that ever computes it.
pub const TAG_NAMESPACE: Uuid = Uuid::from_bytes([
	0x5d, 0x3f, 0x9a, 0x27, 0x6c, 0xe1, 0x5b, 0x88, 0x9a, 0x42, 0x1f, 0x07, 0xb3, 0xce, 0x44, 0x71,
]);

/// A tag definition as a store carries it. `path` is the full ancestor chain
/// (`Work/Clients/Acme`) in the author's casing; hierarchy is derived from it
/// rather than stored as parent pointers, so a definition travels whole.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct TagDefinition {
	pub uuid: Uuid,
	/// Merge key only, never a join key: consulted when a definition arrives
	/// from elsewhere to find an existing equivalent under another uuid.
	pub slug_id: Uuid,
	pub path: String,
	pub color: Option<String>,
	pub icon: Option<String>,
	/// Sortable HLC text; the later stamp wins the whole row on merge.
	pub updated_hlc: String,
	pub origin_device: Uuid,
}

/// One tag claim: apply or remove, on a record and, once hashing has reached
/// the bytes, on their convergent content uuid too.
#[derive(Debug, Clone)]
pub struct TagAssertion {
	pub tag_uuid: Uuid,
	pub record_uuid: Uuid,
	/// Rebind evidence: the source-relative path for a filesystem record, the
	/// source's own key for an adapter record.
	pub external_id: Option<String>,
	pub content_uuid: Option<Uuid>,
	/// `true` applies the tag, `false` removes it.
	pub asserted: bool,
	pub stamp: Stamp,
}

/// A definition applied to a record, shaped for decorating listings.
#[derive(Debug, Clone)]
pub struct AppliedTag {
	pub tag_uuid: Uuid,
	pub path: String,
	pub color: Option<String>,
	pub icon: Option<String>,
}

/// Canonical display form of a tag path: segments trimmed, joined with `/`,
/// the author's casing kept. Empty segments are rejected rather than
/// collapsed, because `Work//Acme` is more likely a typo than an intent, and
/// silently merging it with `Work/Acme` would hide that.
pub fn normalize_tag_path(raw: &str) -> Result<String> {
	let segments: Vec<&str> = raw.split('/').map(str::trim).collect();
	if segments.iter().any(|segment| segment.is_empty()) {
		return Err(Error::Other(format!("malformed tag path: '{raw}'")));
	}
	Ok(segments.join("/"))
}

/// The convergent slug for a normalized path. Case-folded and composed to
/// NFC, so a path typed as `Café` and one assembled from decomposed
/// filesystem strings converge, which is what lets a travelling definition
/// find the library's existing tag.
pub fn slug_for_path(normalized: &str) -> Uuid {
	let folded: String = normalized.to_lowercase().nfc().collect();
	Uuid::new_v5(&TAG_NAMESPACE, folded.as_bytes())
}

impl SourceDb {
	/// Copy definitions into this store, the later stamp winning the whole
	/// row. Applying a tag copies its definition into every store receiving
	/// an assertion; this is that copy, and it is also what a delivery batch
	/// runs first so its assertions never reference a definition the store
	/// cannot name. Returns the number of rows created or updated.
	pub async fn upsert_tag_definitions(&self, definitions: &[TagDefinition]) -> Result<u64> {
		if definitions.is_empty() {
			return Ok(0);
		}

		let mut tx = self.pool().begin().await?;
		let mut changed = 0;
		for definition in definitions {
			let result = sqlx::query(
				"INSERT INTO tag_definition
					 (uuid, slug_id, path, color, icon, updated_hlc, origin_device)
				 VALUES (?, ?, ?, ?, ?, ?, ?)
				 ON CONFLICT (uuid) DO UPDATE SET
					slug_id = excluded.slug_id,
					path = excluded.path,
					color = excluded.color,
					icon = excluded.icon,
					updated_hlc = excluded.updated_hlc,
					origin_device = excluded.origin_device
				 WHERE excluded.updated_hlc > tag_definition.updated_hlc",
			)
			.bind(definition.uuid)
			.bind(definition.slug_id)
			.bind(&definition.path)
			.bind(&definition.color)
			.bind(&definition.icon)
			.bind(&definition.updated_hlc)
			.bind(definition.origin_device)
			.execute(&mut *tx)
			.await?;
			changed += result.rows_affected();
		}
		tx.commit().await?;

		Ok(changed)
	}

	/// Append assertion rows. The primary key is the dedupe key, so replaying
	/// a delivery batch inserts nothing and is safe. Returns the number of
	/// rows that were actually new.
	pub async fn append_tag_assertions(&self, assertions: &[TagAssertion]) -> Result<u64> {
		if assertions.is_empty() {
			return Ok(0);
		}

		let mut tx = self.pool().begin().await?;
		let mut inserted = 0;
		for assertion in assertions {
			let result = sqlx::query(
				"INSERT INTO tag_assertion
					 (tag_uuid, record_uuid, external_id, content_uuid, asserted, hlc, device_uuid)
				 VALUES (?, ?, ?, ?, ?, ?, ?)
				 ON CONFLICT DO NOTHING",
			)
			.bind(assertion.tag_uuid)
			.bind(assertion.record_uuid)
			.bind(&assertion.external_id)
			.bind(assertion.content_uuid)
			.bind(assertion.asserted)
			.bind(&assertion.stamp.hlc)
			.bind(assertion.stamp.device_uuid)
			.execute(&mut *tx)
			.await?;
			inserted += result.rows_affected();
		}
		tx.commit().await?;

		Ok(inserted)
	}

	/// Late binding: fill the content key on assertions whose record has
	/// since been hashed. An application made during a walk is record-keyed;
	/// the content key is what reaches every other copy of the bytes, so the
	/// hashing commit path calls this after content identities land.
	pub async fn bind_assertion_content(&self) -> Result<u64> {
		let result = sqlx::query(
			"UPDATE tag_assertion SET content_uuid = (
				 SELECT c.uuid FROM record r JOIN content c ON c.id = r.content_id
				 WHERE r.uuid = tag_assertion.record_uuid)
			 WHERE content_uuid IS NULL AND EXISTS (
				 SELECT 1 FROM record r JOIN content c ON c.id = r.content_id
				 WHERE r.uuid = tag_assertion.record_uuid)",
		)
		.execute(self.pool())
		.await?;

		Ok(result.rows_affected())
	}

	/// Bind orphaned assertions back onto records, matching the evidence each
	/// row carries: content uuid first, since it is derived from the bytes
	/// and holds across a rename and across a machine; then the record's own
	/// key; then the external id spent as a path, since a filesystem file
	/// stores no key of its own. Same contract as
	/// [`SourceDb::rebind_overlays`], wanted in the same places.
	///
	/// An orphan whose target already carries the identical row is left in
	/// place: the primary key refuses the duplicate, and deleting a person's
	/// assertion is not this function's call to make.
	pub async fn rebind_tag_assertions(&self) -> Result<u64> {
		// Bare columns beside MAX(hlc) take their values from the winning
		// row, which SQLite guarantees, so each stale uuid resolves through
		// its most recently written evidence.
		let orphans: Vec<(Uuid, Option<String>, Option<Uuid>, String)> = sqlx::query_as(
			"SELECT a.record_uuid, a.external_id, a.content_uuid, MAX(a.hlc)
				 FROM tag_assertion a
				 WHERE NOT EXISTS (SELECT 1 FROM record r WHERE r.uuid = a.record_uuid)
				 GROUP BY a.record_uuid",
		)
		.fetch_all(self.pool())
		.await?;

		let mut rebound = 0;
		for (stale, external_id, content_uuid, _hlc) in orphans {
			let mut target: Option<(Uuid,)> = match content_uuid {
				Some(content) => {
					sqlx::query_as(
						"SELECT r.uuid FROM record r JOIN content c ON c.id = r.content_id
							 WHERE c.uuid = ? LIMIT 1",
					)
					.bind(content)
					.fetch_optional(self.pool())
					.await?
				}
				None => None,
			};

			if target.is_none() {
				if let Some(external) = &external_id {
					// An assertion carries no record type, so the raw key
					// match is best effort; the path probe covers filesystem
					// records, whose external id here is their relative path.
					target =
						sqlx::query_as("SELECT uuid FROM record WHERE external_id = ? LIMIT 1")
							.bind(external)
							.fetch_optional(self.pool())
							.await?;
					if target.is_none() {
						target = self.resolve_path(external).await?.map(|uuid| (uuid,));
					}
				}
			}

			let Some((target,)) = target else { continue };

			let result = sqlx::query(
				"UPDATE OR IGNORE tag_assertion SET record_uuid = ? WHERE record_uuid = ?",
			)
			.bind(target)
			.bind(stale)
			.execute(self.pool())
			.await?;
			rebound += result.rows_affected();
		}

		Ok(rebound)
	}

	/// Remove a definition from this store. Definitions are mutable objects
	/// rather than assertions, so a deliberate delete removes the row; the
	/// assertion history referencing it stays, because deleting a person's
	/// claims is never implied by deleting a name for them.
	pub async fn remove_tag_definition(&self, tag_uuid: Uuid) -> Result<bool> {
		let result = sqlx::query("DELETE FROM tag_definition WHERE uuid = ?")
			.bind(tag_uuid)
			.execute(self.pool())
			.await?;
		Ok(result.rows_affected() > 0)
	}

	/// Applied tags for a batch of records. See [`tag_state_for_records`].
	pub async fn tags_for_records(
		&self,
		record_uuids: &[Uuid],
	) -> Result<HashMap<Uuid, Vec<AppliedTag>>> {
		tag_state_for_records(self.pool(), record_uuids).await
	}

	/// Records currently carrying a tag. See [`records_for_tag`].
	pub async fn records_with_tag(&self, tag_uuid: Uuid) -> Result<Vec<Uuid>> {
		records_for_tag(self.pool(), tag_uuid).await
	}

	/// Definitions this store carries, ordered by path.
	pub async fn tag_definitions(&self) -> Result<Vec<TagDefinition>> {
		list_tag_definitions(self.pool()).await
	}
}

/// Applied tags for a batch of records, content collapse included: a row
/// keyed by content reaches every copy of those bytes in this store. For
/// each record and tag, the latest assertion by HLC decides, whichever key
/// it arrived on, so a record-scoped removal can beat an earlier
/// content-scoped apply for that one copy.
pub async fn tag_state_for_records(
	pool: &SqlitePool,
	record_uuids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<AppliedTag>>> {
	if record_uuids.is_empty() {
		return Ok(HashMap::new());
	}

	// Content identities for the batch, so content-keyed rows can fan out to
	// the records sharing the bytes.
	let record_ph = vec!["?"; record_uuids.len()].join(", ");
	let sql = format!(
		"SELECT r.uuid, c.uuid FROM record r JOIN content c ON c.id = r.content_id
			 WHERE r.uuid IN ({record_ph})"
	);
	let mut query = sqlx::query_as::<_, (Uuid, Uuid)>(&sql);
	for id in record_uuids {
		query = query.bind(*id);
	}
	let mut records_of_content: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
	for (record, content) in query.fetch_all(pool).await? {
		records_of_content.entry(content).or_default().push(record);
	}

	let content_ids: Vec<Uuid> = records_of_content.keys().copied().collect();
	let rows: Vec<(Uuid, Uuid, Option<Uuid>, bool, String, Uuid)> = if content_ids.is_empty() {
		let sql = format!(
			"SELECT tag_uuid, record_uuid, content_uuid, asserted, hlc, device_uuid
				 FROM tag_assertion WHERE record_uuid IN ({record_ph})"
		);
		let mut query = sqlx::query_as(&sql);
		for id in record_uuids {
			query = query.bind(*id);
		}
		query.fetch_all(pool).await?
	} else {
		let content_ph = vec!["?"; content_ids.len()].join(", ");
		let sql = format!(
			"SELECT tag_uuid, record_uuid, content_uuid, asserted, hlc, device_uuid
				 FROM tag_assertion
				 WHERE record_uuid IN ({record_ph}) OR content_uuid IN ({content_ph})"
		);
		let mut query = sqlx::query_as(&sql);
		for id in record_uuids {
			query = query.bind(*id);
		}
		for id in &content_ids {
			query = query.bind(*id);
		}
		query.fetch_all(pool).await?
	};

	let in_batch: HashSet<Uuid> = record_uuids.iter().copied().collect();
	let mut winners: HashMap<(Uuid, Uuid), (String, Uuid, bool)> = HashMap::new();
	for (tag, record, content, asserted, hlc, device) in rows {
		let mut targets: HashSet<Uuid> = HashSet::new();
		if in_batch.contains(&record) {
			targets.insert(record);
		}
		if let Some(content) = content {
			if let Some(sharing) = records_of_content.get(&content) {
				targets.extend(sharing.iter().copied());
			}
		}
		for target in targets {
			consider(&mut winners, (target, tag), (&hlc, device, asserted));
		}
	}

	collect_applied(pool, winners).await
}

/// Records whose current state carries the tag, content collapse included.
/// Orphaned assertions wait for [`SourceDb::rebind_tag_assertions`] rather
/// than surfacing a record uuid nothing can resolve.
pub async fn records_for_tag(pool: &SqlitePool, tag_uuid: Uuid) -> Result<Vec<Uuid>> {
	let rows: Vec<(Uuid, Option<Uuid>, bool, String, Uuid)> = sqlx::query_as(
		"SELECT record_uuid, content_uuid, asserted, hlc, device_uuid
			 FROM tag_assertion WHERE tag_uuid = ?",
	)
	.bind(tag_uuid)
	.fetch_all(pool)
	.await?;
	if rows.is_empty() {
		return Ok(Vec::new());
	}

	// Every record sharing bytes with a content-keyed row is a candidate,
	// whether or not any row names it directly.
	let content_ids: Vec<Uuid> = rows
		.iter()
		.filter_map(|(_, content, ..)| *content)
		.collect::<HashSet<_>>()
		.into_iter()
		.collect();
	let mut records_of_content: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
	if !content_ids.is_empty() {
		let content_ph = vec!["?"; content_ids.len()].join(", ");
		let sql = format!(
			"SELECT r.uuid, c.uuid FROM record r JOIN content c ON c.id = r.content_id
				 WHERE c.uuid IN ({content_ph})"
		);
		let mut query = sqlx::query_as::<_, (Uuid, Uuid)>(&sql);
		for id in &content_ids {
			query = query.bind(*id);
		}
		for (record, content) in query.fetch_all(pool).await? {
			records_of_content.entry(content).or_default().push(record);
		}
	}

	let mut winners: HashMap<(Uuid, Uuid), (String, Uuid, bool)> = HashMap::new();
	for (record, content, asserted, hlc, device) in &rows {
		let mut targets: HashSet<Uuid> = HashSet::new();
		targets.insert(*record);
		if let Some(content) = content {
			if let Some(sharing) = records_of_content.get(content) {
				targets.extend(sharing.iter().copied());
			}
		}
		for target in targets {
			consider(&mut winners, (target, tag_uuid), (hlc, *device, *asserted));
		}
	}

	let applied: Vec<Uuid> = winners
		.into_iter()
		.filter(|(_, (_, _, asserted))| *asserted)
		.map(|((record, _), _)| record)
		.collect();
	if applied.is_empty() {
		return Ok(Vec::new());
	}

	let applied_ph = vec!["?"; applied.len()].join(", ");
	let sql = format!("SELECT uuid FROM record WHERE uuid IN ({applied_ph})");
	let mut query = sqlx::query_as::<_, (Uuid,)>(&sql);
	for id in &applied {
		query = query.bind(*id);
	}
	Ok(query
		.fetch_all(pool)
		.await?
		.into_iter()
		.map(|(uuid,)| uuid)
		.collect())
}

/// Definitions a store carries, ordered by path.
pub async fn list_tag_definitions(pool: &SqlitePool) -> Result<Vec<TagDefinition>> {
	Ok(sqlx::query_as::<_, TagDefinition>(
		"SELECT uuid, slug_id, path, color, icon, updated_hlc, origin_device
			 FROM tag_definition ORDER BY path",
	)
	.fetch_all(pool)
	.await?)
}

/// Keep the later claim: HLC first, device uuid as the deterministic tiebreak
/// when two devices stamped the same instant.
fn consider(
	winners: &mut HashMap<(Uuid, Uuid), (String, Uuid, bool)>,
	key: (Uuid, Uuid),
	candidate: (&str, Uuid, bool),
) {
	let (hlc, device, asserted) = candidate;
	match winners.get(&key) {
		Some((current_hlc, current_device, _))
			if (current_hlc.as_str(), *current_device) >= (hlc, device) => {}
		_ => {
			winners.insert(key, (hlc.to_string(), device, asserted));
		}
	}
}

/// Resolve winning applies into definition-shaped results, one sorted list
/// per record.
async fn collect_applied(
	pool: &SqlitePool,
	winners: HashMap<(Uuid, Uuid), (String, Uuid, bool)>,
) -> Result<HashMap<Uuid, Vec<AppliedTag>>> {
	let tag_ids: Vec<Uuid> = winners
		.iter()
		.filter(|(_, (_, _, asserted))| *asserted)
		.map(|((_, tag), _)| *tag)
		.collect::<HashSet<_>>()
		.into_iter()
		.collect();
	if tag_ids.is_empty() {
		return Ok(HashMap::new());
	}

	let tag_ph = vec!["?"; tag_ids.len()].join(", ");
	let sql =
		format!("SELECT uuid, path, color, icon FROM tag_definition WHERE uuid IN ({tag_ph})");
	let mut query = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(&sql);
	for id in &tag_ids {
		query = query.bind(*id);
	}
	let definitions: HashMap<Uuid, AppliedTag> = query
		.fetch_all(pool)
		.await?
		.into_iter()
		.map(|(tag_uuid, path, color, icon)| {
			(
				tag_uuid,
				AppliedTag {
					tag_uuid,
					path,
					color,
					icon,
				},
			)
		})
		.collect();

	let mut applied: HashMap<Uuid, Vec<AppliedTag>> = HashMap::new();
	for ((record, tag), (_, _, asserted)) in winners {
		if !asserted {
			continue;
		}
		match definitions.get(&tag) {
			Some(definition) => applied.entry(record).or_default().push(definition.clone()),
			// Adoption copies the definition alongside every assertion, so a
			// missing one is a delivery that skipped that step.
			None => {
				tracing::warn!(%tag, %record, "assertion references a definition this store does not carry")
			}
		}
	}
	for tags in applied.values_mut() {
		tags.sort_by(|a, b| a.path.cmp(&b.path));
	}

	Ok(applied)
}
