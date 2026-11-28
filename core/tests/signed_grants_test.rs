//! Signed grant tokens, standing grants, and grants kept as assertions in the
//! space's store (`docs/core/design/agent-grants.md`).
//!
//! A signed token is checked against the issuer key its space's store
//! publishes, so tampering, another key, or a space whose store never vouched
//! for the issuer all read as an unknown token. A revocation is honoured on
//! the next request whether it was made here or merged into the store from
//! another device. A standing grant is refused anywhere but a workspace space.

#![cfg(unix)]

mod helpers;

use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use helpers::*;
use sd_core::{
	domain::{Space, SpaceKind},
	infra::{action::LibraryAction, db::entities::agent_grant},
	ops::{
		grants::{
			authorize,
			create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
			extend::{ExtendGrantAction, ExtendGrantInput},
			revoke::{RevokeGrantAction, RevokeGrantInput},
			signed,
			store::revoke_for_source,
			GrantCap, GrantError, GrantKind, TokenFormat,
		},
		spaces::{SpaceCreateAction, SpaceCreateInput},
	},
};
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

async fn space(harness: &IndexingHarness, kind: SpaceKind, path: Option<PathBuf>) -> Space {
	SpaceCreateAction::from_input(SpaceCreateInput {
		name: None,
		kind,
		path,
		content_indexing: None,
		protected: None,
	})
	.expect("space input")
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.expect("space created")
	.space
}

async fn grant(
	harness: &IndexingHarness,
	path: &Path,
	kind: Option<GrantKind>,
	token_format: Option<TokenFormat>,
	ttl_seconds: Option<u32>,
) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(GrantCap::ReadWrite),
		ttl_seconds,
		label: Some("signed".to_string()),
		kind,
		token_format,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

async fn check(harness: &IndexingHarness, token: &str, path: &str) -> Result<PathBuf, GrantError> {
	authorize(
		&harness.core.context,
		token,
		"test",
		Path::new(path),
		GrantCap::Read,
	)
	.await
	.map(|scope| scope.path)
}

async fn signing_key(harness: &IndexingHarness) -> ed25519_dalek::SigningKey {
	let master = harness
		.core
		.context
		.device_manager
		.master_key()
		.await
		.expect("master key");
	signed::signing_key(&master)
}

async fn store_of(harness: &IndexingHarness, space: &Space) -> std::sync::Arc<sd_store::SourceDb> {
	harness
		.core
		.context
		.volume_index()
		.read_store(space.source_id)
		.await
		.expect("the space's store")
}

#[tokio::test]
async fn standing_grants_only_on_workspace_spaces() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("standing_grants")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("docs/notes.md", "notes").await?;
	tracked.write_file("loose/file.md", "file").await?;
	tracked.track().await?;
	let base = std::fs::canonicalize(harness.temp_path())?;
	let docs = base.join("tracked/docs");
	space(&harness, SpaceKind::Folder, Some(docs.clone())).await;

	// A folder space and a folder in no space both refuse a standing grant.
	for path in [docs.clone(), base.join("tracked/loose")] {
		let refused = grant(&harness, &path, Some(GrantKind::Standing), None, None).await;
		assert!(
			refused
				.unwrap_err()
				.to_string()
				.contains("only for a workspace space"),
			"{}",
			path.display()
		);
	}
	// A signed token needs a space to vouch for it.
	assert!(grant(
		&harness,
		&base.join("tracked/loose"),
		None,
		Some(TokenFormat::Signed),
		None
	)
	.await
	.is_err());

	let workspace = space(&harness, SpaceKind::Workspace, None).await;
	let root = workspace.path.clone().expect("located here");
	std::fs::write(root.join("plan.md"), "plan")?;

	// Thirty days by default, signed by default, and never longer.
	let standing = grant(&harness, &root, Some(GrantKind::Standing), None, None).await?;
	assert_eq!(standing.grant.kind, GrantKind::Standing);
	assert_eq!(standing.grant.token_format, TokenFormat::Signed);
	assert_eq!(standing.grant.space_id, Some(workspace.id));
	assert!(standing.token.starts_with(signed::SIGNED_PREFIX));
	assert_eq!(
		standing.grant.expires_at - standing.grant.created_at,
		Duration::days(30)
	);
	assert!(grant(
		&harness,
		&root,
		Some(GrantKind::Standing),
		None,
		Some(31 * 86_400)
	)
	.await
	.is_err());
	let opaque = grant(
		&harness,
		&root,
		Some(GrantKind::Standing),
		Some(TokenFormat::Opaque),
		Some(20 * 86_400),
	)
	.await?;
	assert!(!opaque.token.starts_with(signed::SIGNED_PREFIX));

	// A task grant keeps its 12 hour ceiling on a workspace too.
	assert!(grant(&harness, &root, None, None, Some(13 * 3600))
		.await
		.is_err());

	for token in [&standing.token, &opaque.token] {
		assert_eq!(
			check(&harness, token, "plan.md").await?,
			root.join("plan.md")
		);
	}
	Ok(())
}

#[tokio::test]
async fn signed_tokens_verify_against_their_space_and_honour_revocation() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("signed_grants")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("docs/notes.md", "notes").await?;
	tracked.track().await?;
	let base = std::fs::canonicalize(harness.temp_path())?;
	let folder = space(&harness, SpaceKind::Folder, Some(base.join("tracked/docs"))).await;
	let workspace = space(&harness, SpaceKind::Workspace, None).await;
	let root = workspace.path.clone().expect("located here");
	std::fs::create_dir_all(root.join("src"))?;
	std::fs::write(root.join("src/main.rs"), "fn main() {}")?;

	let made = grant(
		&harness,
		&root.join("src"),
		None,
		Some(TokenFormat::Signed),
		None,
	)
	.await?;
	let token = made.token.clone();
	let id = made.grant.id;
	assert_eq!(made.grant.token_format, TokenFormat::Signed);
	let scope = authorize(
		&harness.core.context,
		&token,
		"test",
		Path::new("main.rs"),
		GrantCap::ReadWrite,
	)
	.await?;
	assert_eq!(scope.path, root.join("src/main.rs"));
	assert_eq!(scope.space_id, Some(workspace.id));
	// The subtree is part of what was signed: the space root is outside it.
	assert!(matches!(
		check(&harness, &token, "../plan.md").await,
		Err(GrantError::Escape(_))
	));

	// The grant is an assertion in the space's store, with the issuer's key.
	let store = store_of(&harness, &workspace).await;
	let row = store.grant(id).await?.expect("grant assertion");
	assert_eq!(row.space_uuid, workspace.id);
	assert_eq!(row.subtree, "src");
	assert_eq!(row.kind, "task");
	assert_eq!(row.token_format, "signed");
	assert_eq!(row.caps, "read_write");
	assert!(!format!("{row:?}").contains(&token[signed::SIGNED_PREFIX.len()..]));
	let claims = signed::unverified_claims(&token)?;
	assert_eq!(claims.grant_id, id);
	assert_eq!(row.issuer, claims.iss);
	let issuer = store.grant_issuer(claims.iss).await?.expect("issuer key");
	let key = signing_key(&harness).await;
	assert_eq!(issuer.public_key, key.verifying_key().to_bytes().to_vec());

	// The assertions round-trip into another store of the same source.
	let sent = store.grant_assertions().await?;
	let copy_dir = tempfile::tempdir()?;
	let copy = sd_store::SourceManager::new(copy_dir.path().join("sources"))
		.ensure("copy", &sd_store::filesystem_schema())
		.await?;
	copy.merge_grant_assertions(&sent).await?;
	assert_eq!(copy.grant_assertions().await?, sent);

	// Tampering: one changed character in the claims.
	let mut tampered = token.clone().into_bytes();
	let at = signed::SIGNED_PREFIX.len() + 10;
	tampered[at] = if tampered[at] == b'A' { b'B' } else { b'A' };
	let tampered = String::from_utf8(tampered)?;
	assert_eq!(
		check(&harness, &tampered, "main.rs").await,
		Err(GrantError::UnknownToken)
	);

	// Another key signing the same claims is no issuer of this space.
	let foreign = signed::sign(&signed::signing_key(&[7; 32]), &claims).expect("signed");
	assert_eq!(
		check(&harness, &foreign, "main.rs").await,
		Err(GrantError::UnknownToken)
	);

	// The wrong space: the genuine key, naming a space whose store never
	// published it.
	let mut elsewhere = claims.clone();
	elsewhere.space_id = folder.id;
	elsewhere.subtree = String::new();
	let elsewhere = signed::sign(&key, &elsewhere).expect("signed");
	assert_eq!(
		check(&harness, &elsewhere, "notes.md").await,
		Err(GrantError::UnknownToken)
	);
	// And a space no store holds at all.
	let mut nowhere = claims.clone();
	nowhere.space_id = Uuid::now_v7();
	let nowhere = signed::sign(&key, &nowhere).expect("signed");
	assert_eq!(
		check(&harness, &nowhere, "main.rs").await,
		Err(GrantError::UnknownToken)
	);

	// Expiry is the signed claim's: a genuine token past it is refused as
	// expired, not as unknown.
	let mut stale = claims.clone();
	stale.exp = (Utc::now() - Duration::seconds(1)).timestamp();
	let stale = signed::sign(&key, &stale).expect("signed");
	assert_eq!(
		check(&harness, &stale, "main.rs").await,
		Err(GrantError::Expired)
	);

	// A signed token carries its expiry, so it cannot be extended.
	assert!(ExtendGrantAction::from_input(ExtendGrantInput {
		id,
		ttl_seconds: None
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.is_err());

	// Revoked here: the next request is refused, and the revocation is an
	// assertion in the space's store.
	RevokeGrantAction::from_input(RevokeGrantInput { id })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), harness.core.context.clone())
		.await?;
	assert_eq!(
		check(&harness, &token, "main.rs").await,
		Err(GrantError::Revoked)
	);
	assert!(store.grant_revocation(id).await?.is_some());

	// Revoked on another device: only the merged assertion knows, for a
	// signed grant and for an opaque grant in a space alike.
	for format in [TokenFormat::Signed, TokenFormat::Opaque] {
		let other = grant(&harness, &root, None, Some(format), None).await?;
		assert!(check(&harness, &other.token, "src/main.rs").await.is_ok());
		let writable = harness
			.core
			.context
			.volume_index()
			.store_for(&root)
			.await
			.expect("writable store");
		writable
			.db()
			.merge_grant_assertions(&sd_store::GrantAssertions {
				revocations: vec![sd_store::GrantRevocation {
					grant_uuid: other.grant.id,
					revoked_at: Utc::now().to_rfc3339(),
					hlc: "9999999999999-0000-remote".to_string(),
					origin_device: Uuid::from_u128(42),
				}],
				..Default::default()
			})
			.await?;
		assert_eq!(
			check(&harness, &other.token, "src/main.rs").await,
			Err(GrantError::Revoked),
			"{format:?}"
		);
	}
	Ok(())
}

#[tokio::test]
async fn grants_without_an_assertion_extend_and_source_removal_revokes_in_the_store(
) -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("signed_grants_backfill")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("docs/notes.md", "notes").await?;
	tracked.track().await?;
	let base = std::fs::canonicalize(harness.temp_path())?;
	let docs = base.join("tracked/docs");

	// A grant made before grants were asserted: it names a space, but the
	// space's store has no record of it.
	let old = grant(&harness, &docs, None, Some(TokenFormat::Opaque), None).await?;
	let folder = space(&harness, SpaceKind::Folder, Some(docs.clone())).await;
	agent_grant::Entity::update_many()
		.col_expr(agent_grant::Column::SpaceId, Expr::value(Some(folder.id)))
		.filter(agent_grant::Column::Uuid.eq(old.grant.id))
		.exec(harness.library.db().conn())
		.await?;
	let store = store_of(&harness, &folder).await;
	assert!(store.grant(old.grant.id).await?.is_none());

	let extended = ExtendGrantAction::from_input(ExtendGrantInput {
		id: old.grant.id,
		ttl_seconds: Some(7200),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let row = store
		.grant(old.grant.id)
		.await?
		.expect("grant assertion rebuilt");
	assert_eq!(row.space_uuid, folder.id);
	assert_eq!(row.subtree, "");
	assert_eq!(row.expires_at, extended.grant.expires_at.to_rfc3339());

	// Removing the source revokes every grant its store asserts, so a copy
	// of the store refuses them too.
	let signed = grant(&harness, &docs, None, Some(TokenFormat::Signed), None).await?;
	revoke_for_source(
		&harness.core.context,
		harness.library.db().conn(),
		folder.source_id,
	)
	.await
	.map_err(anyhow::Error::msg)?;
	assert!(store.grant_revocation(old.grant.id).await?.is_some());
	assert!(store.grant_revocation(signed.grant.id).await?.is_some());
	assert_eq!(
		check(&harness, &signed.token, "notes.md").await,
		Err(GrantError::Revoked)
	);
	Ok(())
}
