//! `sd grant` as a person uses it: create prints the token once, list shows
//! the grant without it, revoke ends it, and list shows it revoked.
//!
//! Starts a core with its RPC server in this process and drives the built
//! `sd-cli` against it, the same way `mcp_test` does.

use sd_core::{
	infra::{action::LibraryAction, daemon::addr::daemon_socket_addr},
	ops::{
		grants::{authorize, GrantCap, GrantError},
		sources::track::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use serde_json::Value;
use std::{path::Path, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::process::Command;

async fn free_instance() -> (String, String) {
	for attempt in 0..50u32 {
		let name = format!("grant-test-{attempt}");
		let addr = daemon_socket_addr(Some(&name));
		if tokio::net::TcpListener::bind(addr).await.is_ok() {
			return (name, addr.to_string());
		}
	}
	panic!("no free instance port");
}

async fn sd(cli_data: &Path, instance: &str, args: &[&str]) -> (bool, String, String) {
	let output = Command::new(env!("CARGO_BIN_EXE_sd-cli"))
		.arg("--data-dir")
		.arg(cli_data)
		.arg("--instance")
		.arg(instance)
		.args(args)
		.output()
		.await
		.expect("run sd-cli");
	(
		output.status.success(),
		String::from_utf8_lossy(&output.stdout).into_owned(),
		String::from_utf8_lossy(&output.stderr).into_owned(),
	)
}

#[tokio::test(flavor = "multi_thread")]
async fn sd_grant_creates_lists_and_revokes() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
	let temp = TempDir::new()?;
	// The CLI reads the daemon's RPC secret from its own data directory, so
	// the core lives where the CLI resolves this instance, as in a real install.
	let (instance, socket_addr) = free_instance().await;
	let core =
		Arc::new(Core::new(temp.path().join("cli").join("instances").join(&instance)).await?);
	let library = core
		.libraries
		.create_library("Grant Library".to_string(), None, core.context.clone())
		.await?;

	let source_dir = std::fs::canonicalize(temp.path())?.join("source");
	std::fs::create_dir_all(source_dir.join("project"))?;
	std::fs::write(source_dir.join("project/a.txt"), b"alpha")?;
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: source_dir.clone(),
		name: None,
		overrides: Default::default(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	if let Some(job) = tracked.job_id {
		if let Some(walk) = library
			.jobs()
			.get_job(sd_core::infra::job::types::JobId(job))
			.await
		{
			walk.wait().await?;
		}
	}

	let server_core = core.clone();
	tokio::spawn(async move {
		let mut server = sd_core::infra::daemon::rpc::RpcServer::new(socket_addr, server_core);
		if let Err(e) = server.start().await {
			eprintln!("rpc server stopped: {e}");
		}
	});
	tokio::time::sleep(Duration::from_millis(500)).await;

	let cli_data = temp.path().join("cli");
	// The core also opens a default library, so pick ours explicitly.
	let library_id = library.id().to_string();
	let (ok, _, stderr) = sd(
		&cli_data,
		&instance,
		&["library", "switch", "--library-id", &library_id],
	)
	.await;
	assert!(ok, "switch failed: {stderr}");

	let project = source_dir.join("project");
	let project_arg = sd_core::common::utils::strip_windows_extended_prefix(project.clone())
		.to_string_lossy()
		.into_owned();

	// Human output prints the token once.
	let (ok, stdout, stderr) = sd(
		&cli_data,
		&instance,
		&[
			"grant",
			"create",
			&project_arg,
			"--read",
			"--ttl",
			"30m",
			"--label",
			"agent",
		],
	)
	.await;
	assert!(ok, "create failed: {stderr}");
	let token = stdout
		.lines()
		.find_map(|line| line.strip_prefix("Token:"))
		.map(str::trim)
		.expect("the token is printed")
		.to_string();
	assert!(token.starts_with("sdg_"), "{stdout}");
	assert!(authorize(
		&core.context,
		&token,
		"test",
		Path::new("a.txt"),
		GrantCap::Read
	)
	.await
	.is_ok());

	// A lifetime outside the bounds is refused.
	let (ok, _, _) = sd(
		&cli_data,
		&instance,
		&["grant", "create", &project_arg, "--ttl", "13h"],
	)
	.await;
	assert!(!ok, "a 13h grant was created");

	// List names the grant and never the token.
	let (ok, stdout, stderr) =
		sd(&cli_data, &instance, &["--format", "json", "grant", "list"]).await;
	assert!(ok, "list failed: {stderr}");
	assert!(!stdout.contains(&token[4..]), "list leaked the token");
	let listed: Value = serde_json::from_str(&stdout)?;
	let grants = listed["grants"].as_array().expect("grants");
	assert_eq!(grants.len(), 1, "{listed}");
	assert_eq!(grants[0]["label"], "agent");
	assert_eq!(grants[0]["caps"], "read");
	assert_eq!(grants[0]["status"], "active");
	assert_eq!(grants[0]["subtree"], project_arg);
	let id = grants[0]["id"].as_str().expect("id").to_string();

	// Extend, then revoke.
	let (ok, _, stderr) = sd(
		&cli_data,
		&instance,
		&["grant", "extend", &id, "--ttl", "2h"],
	)
	.await;
	assert!(ok, "extend failed: {stderr}");
	let (ok, stdout, stderr) = sd(&cli_data, &instance, &["grant", "revoke", &id]).await;
	assert!(ok, "revoke failed: {stderr}");
	assert!(stdout.contains(&id), "{stdout}");
	assert_eq!(
		authorize(
			&core.context,
			&token,
			"test",
			Path::new("a.txt"),
			GrantCap::Read
		)
		.await,
		Err(GrantError::Revoked)
	);

	let (ok, stdout, _) = sd(&cli_data, &instance, &["--format", "json", "grant", "list"]).await;
	assert!(ok);
	let listed: Value = serde_json::from_str(&stdout)?;
	assert_eq!(
		listed["grants"].as_array().map(Vec::len),
		Some(0),
		"{listed}"
	);
	let (ok, stdout, _) = sd(
		&cli_data,
		&instance,
		&["--format", "json", "grant", "list", "--all"],
	)
	.await;
	assert!(ok);
	let listed: Value = serde_json::from_str(&stdout)?;
	assert_eq!(listed["grants"][0]["status"], "revoked", "{listed}");

	// The audit shows the allowed request and the refused one.
	let (ok, stdout, stderr) = sd(&cli_data, &instance, &["grant", "audit", &id]).await;
	assert!(ok, "audit failed: {stderr}");
	assert!(
		stdout.contains("refused: the grant was revoked"),
		"{stdout}"
	);
	assert!(stdout.contains("allowed"), "{stdout}");

	core.shutdown().await?;
	Ok(())
}
