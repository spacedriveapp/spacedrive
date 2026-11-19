//! Acceptance for agent grants (`docs/core/design/agent-grants.md`): a grant
//! lends one directory of a tracked source for a bounded time, every request
//! goes through `authorize`, and nothing an agent sends reaches outside the
//! directory or into the files a writable grant must leave alone.
//!
//! One `Core` over temporary directories. The escape cases a Linux runner can
//! build for real (`..`, symlinks out, absolute spellings, another case of a
//! name on a case-sensitive volume) run here; the firmlink and
//! case-insensitive spellings are stood in for in `ops::grants::scope`'s unit
//! tests, since containment is decided the same way for both.

#![cfg(unix)]

mod helpers;

use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use helpers::*;
use sd_core::{
	infra::{
		action::LibraryAction,
		api::SessionContext,
		db::entities::{agent_grant, grant_audit},
		query::LibraryQuery,
	},
	ops::{
		grants::{
			audit::{GrantAuditInput, GrantAuditQuery, AUDIT_CAPACITY},
			authorize,
			create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
			extend::{ExtendGrantAction, ExtendGrantInput},
			list::{ListGrantsInput, ListGrantsQuery},
			revoke::{RevokeGrantAction, RevokeGrantInput},
			GrantCap, GrantError, GrantStatus,
		},
		sources::delete::action::{DeleteSourceAction, DeleteSourceInput},
	},
};
use sea_orm::{
	sea_query::Expr, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use uuid::Uuid;

async fn create(
	harness: &IndexingHarness,
	path: &Path,
	caps: GrantCap,
	ttl_seconds: Option<u32>,
) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(caps),
		ttl_seconds,
		label: Some("acceptance".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

fn session(harness: &IndexingHarness) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(harness.library.id());
	session
}

async fn check(
	harness: &IndexingHarness,
	token: &str,
	path: impl AsRef<Path>,
	cap: GrantCap,
) -> Result<PathBuf, GrantError> {
	authorize(&harness.core.context, token, "test", path.as_ref(), cap)
		.await
		.map(|scope| scope.path)
}

fn escapes(result: Result<PathBuf, GrantError>) -> bool {
	matches!(result, Err(GrantError::Escape(_)))
}

fn protected(result: Result<PathBuf, GrantError>) -> bool {
	matches!(result, Err(GrantError::Protected(_)))
}

#[tokio::test]
async fn grants_scope_expire_revoke_and_protect() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_grants_acceptance")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("project/docs/readme.md", "readme")
		.await?;
	tracked.write_file("project/AGENTS.md", "rules").await?;
	tracked.write_file("project/docs/SKILL.md", "skill").await?;
	tracked.write_file("project/.git/config", "[core]").await?;
	tracked.write_file("private/secret.txt", "secret").await?;
	let source = tracked.track().await?;
	let outside = harness.create_test_dir("outside").await?;
	outside.write_file("secret.txt", "secret").await?;

	let base = std::fs::canonicalize(harness.temp_path())?;
	let project = base.join("tracked/project");

	// Outside every source there is nothing to grant.
	assert!(
		create(&harness, &base.join("outside"), GrantCap::Read, None)
			.await
			.is_err()
	);
	// Lifetimes outside 5 minutes to 12 hours are refused.
	for ttl in [60, 13 * 3600] {
		assert!(create(&harness, &project, GrantCap::Read, Some(ttl))
			.await
			.is_err());
	}

	// A read grant, default lifetime of an hour.
	let read = create(&harness, &project, GrantCap::Read, None).await?;
	assert!(read.token.starts_with("sdg_"));
	assert_eq!(read.grant.subtree, project);
	assert_eq!(read.grant.source_id, source.id);
	let lifetime = read.grant.expires_at - read.grant.created_at;
	assert_eq!(lifetime, Duration::hours(1));

	// Only the hash is stored.
	let rows = agent_grant::Entity::find()
		.all(harness.library.db().conn())
		.await?;
	assert_eq!(rows.len(), 1);
	assert!(!format!("{rows:?}").contains(&read.token[4..]));

	// Inside the grant, relative and absolute.
	let readme = project.join("docs/readme.md");
	assert_eq!(
		check(&harness, &read.token, "docs/readme.md", GrantCap::Read).await,
		Ok(readme.clone())
	);
	assert_eq!(
		check(&harness, &read.token, &readme, GrantCap::Read).await,
		Ok(readme.clone())
	);
	assert_eq!(
		check(&harness, &read.token, "", GrantCap::Read).await,
		Ok(project.clone())
	);
	// Read grants read instruction files; they only refuse writes.
	assert!(check(&harness, &read.token, "AGENTS.md", GrantCap::Read)
		.await
		.is_ok());

	// A read grant writes nothing.
	assert_eq!(
		check(&harness, &read.token, "docs/new.md", GrantCap::ReadWrite).await,
		Err(GrantError::ReadOnly)
	);

	// Escapes: parent components, absolute paths elsewhere in the source and
	// outside it, symlinks pointing out, a dangling link.
	std::os::unix::fs::symlink(base.join("tracked/private"), project.join("link-out"))?;
	std::os::unix::fs::symlink(base.join("outside/secret.txt"), project.join("leak.txt"))?;
	std::os::unix::fs::symlink(base.join("nowhere"), project.join("dangling"))?;
	for path in [
		PathBuf::from("../private/secret.txt"),
		PathBuf::from("docs/../../private/secret.txt"),
		PathBuf::from("link-out/secret.txt"),
		PathBuf::from("link-out"),
		PathBuf::from("leak.txt"),
		PathBuf::from("dangling"),
		base.join("tracked/private/secret.txt"),
		base.join("outside/secret.txt"),
		base.join("tracked"),
		// Another case of the name is another directory on this volume.
		base.join("tracked/PROJECT/docs/readme.md"),
	] {
		assert!(
			escapes(check(&harness, &read.token, &path, GrantCap::Read).await),
			"{} escaped",
			path.display()
		);
	}
	// Another spelling of the granted directory is that directory, and the
	// answer is spelled under the grant.
	std::os::unix::fs::symlink(&project, base.join("alias"))?;
	assert_eq!(
		check(
			&harness,
			&read.token,
			base.join("alias/docs/readme.md"),
			GrantCap::Read
		)
		.await,
		Ok(readme.clone())
	);

	// A writable grant writes, except to instruction files and .git.
	let write = create(&harness, &project, GrantCap::ReadWrite, Some(30 * 60)).await?;
	assert_eq!(
		check(&harness, &write.token, "notes/new.md", GrantCap::ReadWrite).await,
		Ok(project.join("notes/new.md"))
	);
	std::os::unix::fs::symlink(project.join("AGENTS.md"), project.join("notes.md"))?;
	for path in [
		"AGENTS.md",
		"agents.md",
		"docs/SKILL.md",
		"deep/er/CLAUDE.md",
		".git",
		".git/config",
		".git/hooks/new",
		"notes.md",
	] {
		assert!(
			protected(check(&harness, &write.token, path, GrantCap::ReadWrite).await),
			"{path} was writable"
		);
	}
	assert!(check(&harness, &write.token, ".git/config", GrantCap::Read)
		.await
		.is_ok());

	// Unknown and malformed tokens.
	let forged = format!("sdg_{}", "0".repeat(64));
	for token in [forged.as_str(), "", "Bearer x"] {
		assert_eq!(
			check(&harness, token, "docs/readme.md", GrantCap::Read).await,
			Err(GrantError::UnknownToken)
		);
	}

	// Extension is measured from now and bounded by 12 hours.
	assert!(ExtendGrantAction::from_input(ExtendGrantInput {
		id: write.grant.id,
		ttl_seconds: Some(13 * 3600),
	})
	.is_err());
	let before = Utc::now();
	let extended = ExtendGrantAction::from_input(ExtendGrantInput {
		id: write.grant.id,
		ttl_seconds: Some(12 * 3600),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	assert!(extended.grant.expires_at >= before + Duration::hours(12));
	assert!(extended.grant.expires_at <= Utc::now() + Duration::hours(12));

	// Expiry refuses the next request, and an expired grant is not revived.
	agent_grant::Entity::update_many()
		.col_expr(
			agent_grant::Column::ExpiresAt,
			Expr::value(Utc::now() - Duration::seconds(1)),
		)
		.filter(agent_grant::Column::Uuid.eq(read.grant.id))
		.exec(harness.library.db().conn())
		.await?;
	assert_eq!(
		check(&harness, &read.token, "docs/readme.md", GrantCap::Read).await,
		Err(GrantError::Expired)
	);
	assert!(ExtendGrantAction::from_input(ExtendGrantInput {
		id: read.grant.id,
		ttl_seconds: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.is_err());

	// Revocation refuses the very next request.
	assert!(
		check(&harness, &write.token, "docs/readme.md", GrantCap::Read)
			.await
			.is_ok()
	);
	let revoked = RevokeGrantAction::from_input(RevokeGrantInput { id: write.grant.id })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), harness.core.context.clone())
		.await?;
	assert_eq!(revoked.grant.status, GrantStatus::Revoked);
	assert_eq!(
		check(&harness, &write.token, "docs/readme.md", GrantCap::Read).await,
		Err(GrantError::Revoked)
	);
	assert!(ExtendGrantAction::from_input(ExtendGrantInput {
		id: write.grant.id,
		ttl_seconds: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.is_err());

	// The owner's list: active only by default, everything on request.
	let active = ListGrantsQuery::from_input(ListGrantsInput::default())?
		.execute(harness.core.context.clone(), session(&harness))
		.await?;
	assert!(active.grants.is_empty(), "{:?}", active.grants);
	let all = ListGrantsQuery::from_input(ListGrantsInput {
		include_inactive: true,
	})?
	.execute(harness.core.context.clone(), session(&harness))
	.await?;
	let statuses: Vec<(Uuid, GrantStatus)> = all.grants.iter().map(|g| (g.id, g.status)).collect();
	assert_eq!(
		statuses,
		vec![
			(write.grant.id, GrantStatus::Revoked),
			(read.grant.id, GrantStatus::Expired)
		]
	);
	assert!(all.grants.iter().all(|g| g.last_used_at.is_some()));

	// Every decision against a known grant is in the audit, refusals with
	// their reason.
	let audit = GrantAuditQuery::from_input(GrantAuditInput {
		grant_id: Some(write.grant.id),
		limit: Some(1000),
	})?
	.execute(harness.core.context.clone(), session(&harness))
	.await?;
	let newest = &audit.entries[0];
	assert!(!newest.allowed);
	assert_eq!(newest.reason.as_deref(), Some("the grant was revoked"));
	assert!(audit
		.entries
		.iter()
		.any(|e| e.path == ".git/config" && !e.allowed));
	assert!(audit
		.entries
		.iter()
		.any(|e| e.path == "notes/new.md" && e.allowed));

	// The audit is bounded.
	let conn = harness.library.db().conn();
	let filler = (0..AUDIT_CAPACITY).map(|_| grant_audit::ActiveModel {
		grant_id: Set(read.grant.id),
		op: Set("filler".to_string()),
		path: Set(String::new()),
		allowed: Set(true),
		reason: Set(None),
		at: Set(Utc::now()),
		..Default::default()
	});
	for chunk in filler.collect::<Vec<_>>().chunks(1000) {
		grant_audit::Entity::insert_many(chunk.to_vec())
			.exec(conn)
			.await?;
	}
	let live = create(&harness, &project, GrantCap::Read, None).await?;
	check(&harness, &live.token, "docs/readme.md", GrantCap::Read)
		.await
		.unwrap();
	assert_eq!(
		grant_audit::Entity::find().count(conn).await?,
		AUDIT_CAPACITY as u64
	);

	// Removing the source ends every grant on it.
	DeleteSourceAction::from_input(DeleteSourceInput {
		source_id: source.id.to_string(),
		delete_catalog: false,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	assert_eq!(
		check(&harness, &live.token, "docs/readme.md", GrantCap::Read).await,
		Err(GrantError::SourceGone)
	);

	harness.shutdown().await
}
