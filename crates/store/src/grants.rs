//! # Grants as assertions
//!
//! A grant lends a subtree of a space to an agent. The space lives in this
//! store, so its grants do too: any node that holds the store (the home
//! device, a replica, a mirror) can tell which grants exist on the space and
//! which were revoked, without asking the device that minted them.
//! `docs/core/design/agent-grants.md` carries the model.
//!
//! Nothing here is a credential. A grant row holds no token and no token
//! hash, so copying a store never yields a working bearer. A signed token is
//! verified against the issuer's public key in `grant_issuer`; an opaque token
//! stays verifiable only on the device that holds its hash.
//!
//! Three tables, all in the assertion half, all stamped with an HLC and the
//! device that wrote them:
//!
//! - `space_grant`: one row per grant. The later stamp wins the whole row, the
//!   way a space or a tag definition merges, so an extension made on one
//!   device reaches the others.
//! - `grant_revocation`: one row per revoked grant. It is separate from the
//!   grant row so no merge order can undo a revocation: an older grant row
//!   arriving after the revocation changes the grant, never the revocation.
//!   When two devices revoke the same grant, the earlier stamp is kept, so
//!   every store converges on one row.
//! - `grant_issuer`: the Ed25519 public key each minting device signs tokens
//!   with, so a replica can check a signature with no lookup on the issuer.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{SourceDb, Stamp};
use crate::error::Result;

/// A grant as its space's store holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct GrantRow {
	pub uuid: Uuid,
	pub space_uuid: Uuid,
	/// The actor the grant acts as.
	pub actor_uuid: Option<Uuid>,
	/// `task` or `standing`.
	pub kind: String,
	/// `opaque` or `signed`.
	pub token_format: String,
	/// The granted folder, `/`-separated and relative to the space root.
	/// Empty is the whole space.
	pub subtree: String,
	/// `read` or `read_write`.
	pub caps: String,
	pub label: Option<String>,
	/// The device that minted the grant and, for a signed grant, signed it.
	pub issuer: Uuid,
	/// The space's authority epoch the grant was minted under.
	pub epoch: i64,
	/// RFC 3339.
	pub created_at: String,
	/// RFC 3339.
	pub expires_at: String,
	pub updated_hlc: String,
	pub origin_device: Uuid,
}

/// A revoked grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct GrantRevocation {
	pub grant_uuid: Uuid,
	/// RFC 3339.
	pub revoked_at: String,
	pub hlc: String,
	pub origin_device: Uuid,
}

/// The key a device signs grant tokens with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct GrantIssuer {
	pub device_uuid: Uuid,
	/// The 32-byte Ed25519 verifying key.
	pub public_key: Vec<u8>,
	pub updated_hlc: String,
	pub origin_device: Uuid,
}

/// Every grant assertion a store holds: what one store sends another so
/// both honor the same grants.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantAssertions {
	pub grants: Vec<GrantRow>,
	pub revocations: Vec<GrantRevocation>,
	pub issuers: Vec<GrantIssuer>,
}

const GRANT_COLUMNS: &str = "uuid, space_uuid, actor_uuid, kind, token_format, subtree, caps, \
	label, issuer, epoch, created_at, expires_at, updated_hlc, origin_device";

impl SourceDb {
	/// Write a grant, unless the store already holds it with a later stamp.
	/// Returns whether the row changed.
	pub async fn put_grant(&self, row: &GrantRow) -> Result<bool> {
		let result = sqlx::query(&format!(
			"INSERT INTO space_grant ({GRANT_COLUMNS})
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
			 ON CONFLICT (uuid) DO UPDATE SET
				space_uuid = excluded.space_uuid,
				actor_uuid = excluded.actor_uuid,
				kind = excluded.kind,
				token_format = excluded.token_format,
				subtree = excluded.subtree,
				caps = excluded.caps,
				label = excluded.label,
				issuer = excluded.issuer,
				epoch = excluded.epoch,
				created_at = excluded.created_at,
				expires_at = excluded.expires_at,
				updated_hlc = excluded.updated_hlc,
				origin_device = excluded.origin_device
			 WHERE excluded.updated_hlc > space_grant.updated_hlc
				OR (excluded.updated_hlc = space_grant.updated_hlc
					AND excluded.origin_device > space_grant.origin_device)"
		))
		.bind(row.uuid)
		.bind(row.space_uuid)
		.bind(row.actor_uuid)
		.bind(&row.kind)
		.bind(&row.token_format)
		.bind(&row.subtree)
		.bind(&row.caps)
		.bind(&row.label)
		.bind(row.issuer)
		.bind(row.epoch)
		.bind(&row.created_at)
		.bind(&row.expires_at)
		.bind(&row.updated_hlc)
		.bind(row.origin_device)
		.execute(self.pool())
		.await?;
		Ok(result.rows_affected() > 0)
	}

	/// One grant by id.
	pub async fn grant(&self, uuid: Uuid) -> Result<Option<GrantRow>> {
		let row = sqlx::query_as(&format!(
			"SELECT {GRANT_COLUMNS} FROM space_grant WHERE uuid = ?"
		))
		.bind(uuid)
		.fetch_optional(self.pool())
		.await;
		Ok(older_store_has_none(row)?.flatten())
	}

	/// Every grant on one space, revoked or not, oldest first.
	pub async fn grants_on_space(&self, space: Uuid) -> Result<Vec<GrantRow>> {
		let rows = sqlx::query_as(&format!(
			"SELECT {GRANT_COLUMNS} FROM space_grant WHERE space_uuid = ? ORDER BY created_at, uuid"
		))
		.bind(space)
		.fetch_all(self.pool())
		.await;
		Ok(older_store_has_none(rows)?.unwrap_or_default())
	}

	/// Record that a grant is revoked. A grant already revoked keeps the
	/// revocation with the earlier stamp. Returns whether the row changed.
	pub async fn revoke_grant(&self, revocation: &GrantRevocation) -> Result<bool> {
		let result = sqlx::query(
			"INSERT INTO grant_revocation (grant_uuid, revoked_at, hlc, origin_device)
			 VALUES (?, ?, ?, ?)
			 ON CONFLICT (grant_uuid) DO UPDATE SET
				revoked_at = excluded.revoked_at,
				hlc = excluded.hlc,
				origin_device = excluded.origin_device
			 WHERE excluded.hlc < grant_revocation.hlc
				OR (excluded.hlc = grant_revocation.hlc
					AND excluded.origin_device < grant_revocation.origin_device)",
		)
		.bind(revocation.grant_uuid)
		.bind(&revocation.revoked_at)
		.bind(&revocation.hlc)
		.bind(revocation.origin_device)
		.execute(self.pool())
		.await?;
		Ok(result.rows_affected() > 0)
	}

	/// The revocation of one grant, if it was revoked.
	pub async fn grant_revocation(&self, grant: Uuid) -> Result<Option<GrantRevocation>> {
		let row = sqlx::query_as(
			"SELECT grant_uuid, revoked_at, hlc, origin_device
			 FROM grant_revocation WHERE grant_uuid = ?",
		)
		.bind(grant)
		.fetch_optional(self.pool())
		.await;
		Ok(older_store_has_none(row)?.flatten())
	}

	/// Publish the key a device signs grant tokens with, unless the store
	/// already holds that key or a later one for that device. Returns whether
	/// the row changed.
	pub async fn put_grant_issuer(&self, issuer: &GrantIssuer) -> Result<bool> {
		let result = sqlx::query(
			"INSERT INTO grant_issuer (device_uuid, public_key, updated_hlc, origin_device)
			 VALUES (?, ?, ?, ?)
			 ON CONFLICT (device_uuid) DO UPDATE SET
				public_key = excluded.public_key,
				updated_hlc = excluded.updated_hlc,
				origin_device = excluded.origin_device
			 WHERE excluded.public_key != grant_issuer.public_key
				AND (excluded.updated_hlc > grant_issuer.updated_hlc
					OR (excluded.updated_hlc = grant_issuer.updated_hlc
						AND excluded.origin_device > grant_issuer.origin_device))",
		)
		.bind(issuer.device_uuid)
		.bind(&issuer.public_key)
		.bind(&issuer.updated_hlc)
		.bind(issuer.origin_device)
		.execute(self.pool())
		.await?;
		Ok(result.rows_affected() > 0)
	}

	/// The key a device signs grant tokens with, if it published one here.
	pub async fn grant_issuer(&self, device: Uuid) -> Result<Option<GrantIssuer>> {
		let row = sqlx::query_as(
			"SELECT device_uuid, public_key, updated_hlc, origin_device
			 FROM grant_issuer WHERE device_uuid = ?",
		)
		.bind(device)
		.fetch_optional(self.pool())
		.await;
		Ok(older_store_has_none(row)?.flatten())
	}

	/// Every grant assertion in this store.
	pub async fn grant_assertions(&self) -> Result<GrantAssertions> {
		let grants = sqlx::query_as(&format!(
			"SELECT {GRANT_COLUMNS} FROM space_grant ORDER BY uuid"
		))
		.fetch_all(self.pool())
		.await;
		let revocations = sqlx::query_as(
			"SELECT grant_uuid, revoked_at, hlc, origin_device
			 FROM grant_revocation ORDER BY grant_uuid",
		)
		.fetch_all(self.pool())
		.await;
		let issuers = sqlx::query_as(
			"SELECT device_uuid, public_key, updated_hlc, origin_device
			 FROM grant_issuer ORDER BY device_uuid",
		)
		.fetch_all(self.pool())
		.await;
		Ok(GrantAssertions {
			grants: older_store_has_none(grants)?.unwrap_or_default(),
			revocations: older_store_has_none(revocations)?.unwrap_or_default(),
			issuers: older_store_has_none(issuers)?.unwrap_or_default(),
		})
	}

	/// Merge grant assertions from another store of the same source. The
	/// result does not depend on the order merges arrive in. Returns how many
	/// rows changed.
	pub async fn merge_grant_assertions(&self, incoming: &GrantAssertions) -> Result<usize> {
		let mut changed = 0;
		for row in &incoming.grants {
			changed += usize::from(self.put_grant(row).await?);
		}
		for revocation in &incoming.revocations {
			changed += usize::from(self.revoke_grant(revocation).await?);
		}
		for issuer in &incoming.issuers {
			changed += usize::from(self.put_grant_issuer(issuer).await?);
		}
		Ok(changed)
	}
}

impl GrantRow {
	/// The stamp this row was last written with.
	pub fn set_stamp(&mut self, stamp: &Stamp) {
		self.updated_hlc = stamp.hlc.clone();
		self.origin_device = stamp.device_uuid;
	}
}

/// A store opened read-only runs no DDL, so one written before grants were
/// assertions, or a replica copied from one, has no tables: it holds none.
fn older_store_has_none<T>(result: std::result::Result<T, sqlx::Error>) -> Result<Option<T>> {
	match result {
		Ok(value) => Ok(Some(value)),
		Err(sqlx::Error::Database(db)) if db.message().contains("no such table") => Ok(None),
		Err(error) => Err(error.into()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	async fn store(name: &str) -> (tempfile::TempDir, SourceDb) {
		let dir = tempfile::tempdir().unwrap();
		let db = crate::SourceManager::new(dir.path().join("sources"))
			.ensure(name, &crate::filesystem_schema())
			.await
			.unwrap();
		(dir, db)
	}

	fn grant(hlc: &str, device: u128, expires_at: &str) -> GrantRow {
		GrantRow {
			uuid: Uuid::from_u128(1),
			space_uuid: Uuid::from_u128(2),
			actor_uuid: Some(Uuid::from_u128(3)),
			kind: "standing".into(),
			token_format: "signed".into(),
			subtree: "notes".into(),
			caps: "read_write".into(),
			label: Some("research-agent".into()),
			issuer: Uuid::from_u128(9),
			epoch: 4,
			created_at: "2026-11-28T00:00:00Z".into(),
			expires_at: expires_at.into(),
			updated_hlc: hlc.into(),
			origin_device: Uuid::from_u128(device),
		}
	}

	fn revocation(hlc: &str, device: u128) -> GrantRevocation {
		GrantRevocation {
			grant_uuid: Uuid::from_u128(1),
			revoked_at: format!("2026-11-28T00:00:{}Z", &hlc[hlc.len() - 2..]),
			hlc: hlc.into(),
			origin_device: Uuid::from_u128(device),
		}
	}

	fn issuer(hlc: &str, key: u8) -> GrantIssuer {
		GrantIssuer {
			device_uuid: Uuid::from_u128(9),
			public_key: vec![key; 32],
			updated_hlc: hlc.into(),
			origin_device: Uuid::from_u128(9),
		}
	}

	#[tokio::test]
	async fn a_grant_reads_back_as_written_and_the_later_stamp_wins() {
		let (_dir, db) = store("drive-1").await;
		let first = grant("0002", 9, "2026-12-01T00:00:00Z");
		assert!(db.put_grant(&first).await.unwrap());
		assert_eq!(db.grant(first.uuid).await.unwrap().as_ref(), Some(&first));

		assert!(!db
			.put_grant(&grant("0001", 9, "2026-12-30T00:00:00Z"))
			.await
			.unwrap());
		let extended = grant("0003", 9, "2026-12-27T00:00:00Z");
		assert!(db.put_grant(&extended).await.unwrap());
		assert_eq!(db.grant(first.uuid).await.unwrap(), Some(extended.clone()));
		assert_eq!(
			db.grants_on_space(extended.space_uuid).await.unwrap(),
			vec![extended]
		);
		assert!(db
			.grants_on_space(Uuid::from_u128(77))
			.await
			.unwrap()
			.is_empty());
	}

	#[tokio::test]
	async fn no_merge_order_undoes_a_revocation() {
		let (_dir, db) = store("drive-1").await;
		assert!(db.revoke_grant(&revocation("0005", 9)).await.unwrap());
		// The grant row arriving later, even with a later stamp, leaves the
		// grant revoked.
		db.put_grant(&grant("0009", 9, "2026-12-01T00:00:00Z"))
			.await
			.unwrap();
		assert!(db
			.grant_revocation(Uuid::from_u128(1))
			.await
			.unwrap()
			.is_some());

		// A second revocation keeps the earlier one, whichever arrives first.
		assert!(!db.revoke_grant(&revocation("0007", 8)).await.unwrap());
		assert!(db.revoke_grant(&revocation("0003", 8)).await.unwrap());
		assert_eq!(
			db.grant_revocation(Uuid::from_u128(1)).await.unwrap(),
			Some(revocation("0003", 8))
		);
	}

	#[tokio::test]
	async fn an_issuer_key_reads_back_and_merges_by_stamp() {
		let (_dir, db) = store("drive-1").await;
		assert!(db.put_grant_issuer(&issuer("0002", 1)).await.unwrap());
		assert!(!db.put_grant_issuer(&issuer("0003", 1)).await.unwrap());
		assert!(!db.put_grant_issuer(&issuer("0001", 2)).await.unwrap());
		assert_eq!(
			db.grant_issuer(Uuid::from_u128(9)).await.unwrap(),
			Some(issuer("0002", 1))
		);
		assert!(db.put_grant_issuer(&issuer("0004", 2)).await.unwrap());
		assert_eq!(
			db.grant_issuer(Uuid::from_u128(9))
				.await
				.unwrap()
				.unwrap()
				.public_key,
			vec![2; 32]
		);
	}

	#[tokio::test]
	async fn grant_assertions_round_trip_between_stores_in_any_order() {
		let (_a_dir, a) = store("drive-a").await;
		a.put_grant(&grant("0002", 9, "2026-12-01T00:00:00Z"))
			.await
			.unwrap();
		let mut other = grant("0004", 9, "2026-12-02T00:00:00Z");
		other.uuid = Uuid::from_u128(5);
		other.token_format = "opaque".into();
		other.kind = "task".into();
		other.actor_uuid = None;
		other.label = None;
		a.put_grant(&other).await.unwrap();
		a.revoke_grant(&revocation("0006", 9)).await.unwrap();
		a.put_grant_issuer(&issuer("0001", 7)).await.unwrap();
		let sent = a.grant_assertions().await.unwrap();
		assert_eq!(sent.grants.len(), 2);

		// Through the wire format, as a sync payload would travel.
		let wire = serde_json::to_string(&sent).unwrap();
		let received: GrantAssertions = serde_json::from_str(&wire).unwrap();

		let (_b_dir, b) = store("drive-b").await;
		assert_eq!(b.merge_grant_assertions(&received).await.unwrap(), 4);
		assert_eq!(b.grant_assertions().await.unwrap(), sent);
		assert_eq!(
			b.merge_grant_assertions(&received).await.unwrap(),
			0,
			"merging twice changes nothing"
		);

		// The same rows in reverse order, over an older local copy, converge
		// on the same state.
		let reversed = GrantAssertions {
			grants: sent.grants.iter().rev().cloned().collect(),
			revocations: sent.revocations.clone(),
			issuers: sent.issuers.clone(),
		};
		let (_c_dir, c) = store("drive-c").await;
		c.put_grant(&grant("0001", 8, "2026-11-29T00:00:00Z"))
			.await
			.unwrap();
		c.merge_grant_assertions(&reversed).await.unwrap();
		assert_eq!(c.grant_assertions().await.unwrap(), sent);
	}

	#[tokio::test]
	async fn a_store_from_before_grant_assertions_reads_as_having_none() {
		let (_dir, db) = store("drive-1").await;
		for table in ["space_grant", "grant_revocation", "grant_issuer"] {
			sqlx::query(&format!("DROP TABLE {table}"))
				.execute(db.pool())
				.await
				.unwrap();
		}
		assert!(db.grant(Uuid::from_u128(1)).await.unwrap().is_none());
		assert!(db
			.grant_revocation(Uuid::from_u128(1))
			.await
			.unwrap()
			.is_none());
		assert!(db.grant_issuer(Uuid::from_u128(9)).await.unwrap().is_none());
		assert_eq!(
			db.grant_assertions().await.unwrap(),
			GrantAssertions::default()
		);
	}
}
