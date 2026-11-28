//! `sd grant`: lend a folder to an agent with a scoped, expiring token.

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use uuid::Uuid;

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::config::app::{
	get::{AgentMcpConfigOutput, AppConfigOutput},
	GetAppConfigQueryInput,
};
use sd_core::ops::grants::{
	audit::{GrantAuditEntry, GrantAuditInput, GrantAuditOutput},
	create::{CreateGrantInput, CreateGrantOutput},
	extend::{ExtendGrantInput, ExtendGrantOutput},
	list::{ListGrantsInput, ListGrantsOutput},
	revoke::{RevokeGrantInput, RevokeGrantOutput},
	GrantCap, GrantInfo,
};

#[derive(Subcommand, Debug)]
pub enum GrantCmd {
	/// Grant a folder to an agent and print its token once
	Create(GrantCreateArgs),
	/// List grants; active ones unless --all
	List(GrantListArgs),
	/// End a grant now; its next request is refused
	Revoke(GrantIdArgs),
	/// Give an active grant a new lifetime, measured from now
	Extend(GrantExtendArgs),
	/// Show the requests made with grants, newest first
	#[command(alias = "log")]
	Audit(GrantAuditArgs),
}

#[derive(Args, Debug)]
pub struct GrantCreateArgs {
	/// The folder to grant. It must be inside a tracked source.
	pub path: PathBuf,
	/// Read-only access (the default)
	#[arg(long, conflicts_with = "read_write")]
	pub read: bool,
	/// Read and write access. AGENTS.md, CLAUDE.md, SKILL.md and .git stay
	/// read-only.
	#[arg(long)]
	pub read_write: bool,
	/// Lifetime, from 5m to 12h, such as 30m, 1h or 2h30m
	#[arg(long, default_value = "1h")]
	pub ttl: String,
	/// A name for the grant, such as the agent it is for
	#[arg(long)]
	pub label: Option<String>,
	/// Also print MCP client config for the agent endpoint. The config reads
	/// the token from SPACEDRIVE_GRANT_TOKEN unless --inline-token is given
	#[arg(long)]
	pub print_mcp_config: bool,
	/// Write the token itself into the printed MCP config
	#[arg(long, requires = "print_mcp_config")]
	pub inline_token: bool,
}

#[derive(Args, Debug)]
pub struct GrantListArgs {
	/// Include expired and revoked grants
	#[arg(long)]
	pub all: bool,
}

#[derive(Args, Debug)]
pub struct GrantIdArgs {
	pub id: Uuid,
}

#[derive(Args, Debug)]
pub struct GrantExtendArgs {
	pub id: Uuid,
	/// New lifetime from now, from 5m to 12h
	#[arg(long, default_value = "1h")]
	pub ttl: String,
}

#[derive(Args, Debug)]
pub struct GrantAuditArgs {
	/// Only this grant's requests
	pub id: Option<Uuid>,
	/// At most this many entries
	#[arg(long, default_value_t = 50)]
	pub limit: u32,
}

pub async fn run(ctx: &Context, cmd: GrantCmd) -> Result<()> {
	match cmd {
		GrantCmd::Create(args) => {
			// A path for another device is that device's to resolve, and a
			// bucket URL is the daemon's.
			let bucket = sd_core::ops::grants::bucket::BucketUrl::parse(&args.path).is_some();
			let path = match ctx.core.device() {
				_ if bucket => args.path,
				Some(_) => args.path,
				None => args
					.path
					.canonicalize()
					.map(sd_core::common::utils::strip_windows_extended_prefix)
					.map_err(|e| anyhow::anyhow!("{}: {e}", args.path.display()))?,
			};
			let input = CreateGrantInput {
				path,
				caps: Some(if args.read_write {
					GrantCap::ReadWrite
				} else {
					GrantCap::Read
				}),
				ttl_seconds: Some(parse_ttl(&args.ttl)?),
				label: args.label,
			};
			let out: CreateGrantOutput = execute_action!(ctx, input);
			let endpoint = if args.print_mcp_config {
				let config: AppConfigOutput = execute_core_query!(ctx, GetAppConfigQueryInput);
				Some(config.agent_mcp)
			} else {
				None
			};
			print_output!(ctx, &out, |o: &CreateGrantOutput| {
				println!(
					"Granted {} ({}) until {}",
					o.grant.subtree.display(),
					o.grant.caps.as_str(),
					o.grant.expires_at.to_rfc3339()
				);
				println!("Grant id: {}", o.grant.id);
				println!("Token:    {}", o.token);
				println!("The token is shown once. Spacedrive keeps only its hash.");
				if let Some(endpoint) = &endpoint {
					if !endpoint.enabled {
						eprintln!(
							"The agent MCP endpoint is off. Run `sd config set agent_mcp.enabled true` and restart the daemon."
						);
					}
					let url = mcp_url(endpoint);
					println!();
					print!("{}", mcp_config(&url, &o.token, args.inline_token));
				}
			});
		}
		GrantCmd::List(args) => {
			let input = ListGrantsInput {
				include_inactive: args.all,
			};
			let out: ListGrantsOutput = execute_query!(ctx, input);
			print_output!(ctx, &out, |o: &ListGrantsOutput| {
				if o.grants.is_empty() {
					println!("No grants");
					return;
				}
				let mut table = Table::new();
				table.load_preset(UTF8_BORDERS_ONLY);
				table.set_header(vec![
					"Id",
					"Path",
					"Caps",
					"Status",
					"Expires",
					"Last used",
					"Requests",
					"Label",
				]);
				for grant in &o.grants {
					table.add_row(row(grant));
				}
				println!("{table}");
			});
		}
		GrantCmd::Revoke(args) => {
			let out: RevokeGrantOutput = execute_action!(ctx, RevokeGrantInput { id: args.id });
			print_output!(ctx, &out, |o: &RevokeGrantOutput| {
				println!("Revoked grant {}", o.grant.id);
			});
		}
		GrantCmd::Extend(args) => {
			let input = ExtendGrantInput {
				id: args.id,
				ttl_seconds: Some(parse_ttl(&args.ttl)?),
			};
			let out: ExtendGrantOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &ExtendGrantOutput| {
				println!(
					"Grant {} now expires at {}",
					o.grant.id,
					o.grant.expires_at.to_rfc3339()
				);
			});
		}
		GrantCmd::Audit(args) => {
			let input = GrantAuditInput {
				grant_id: args.id,
				limit: Some(args.limit),
			};
			let out: GrantAuditOutput = execute_query!(ctx, input);
			print_output!(ctx, &out, |o: &GrantAuditOutput| {
				if o.entries.is_empty() && o.tallies.is_empty() {
					println!("No requests recorded");
					return;
				}
				if !o.entries.is_empty() {
					let mut table = Table::new();
					table.load_preset(UTF8_BORDERS_ONLY);
					table.set_header(vec![
						"Time", "Grant", "Label", "Client", "Session", "Op", "Path", "Pattern",
						"Bytes", "Result", "Ms", "Reason",
					]);
					for entry in &o.entries {
						table.add_row(audit_row(entry));
					}
					println!("{table}");
				}
				if !o.tallies.is_empty() {
					let mut table = Table::new();
					table.load_preset(UTF8_BORDERS_ONLY);
					table.set_header(vec!["Minute", "Kind", "Subject", "Client", "Count"]);
					for tally in &o.tallies {
						table.add_row(vec![
							tally.minute.to_rfc3339(),
							tally.kind.as_str().to_string(),
							tally.subject.clone(),
							tally.client.clone().unwrap_or_default(),
							tally.count.to_string(),
						]);
					}
					println!("{table}");
				}
			});
		}
	}
	Ok(())
}

fn audit_row(entry: &GrantAuditEntry) -> Vec<String> {
	let text = |value: &Option<String>| value.clone().unwrap_or_default();
	let number = |value: Option<u64>| value.map(|n| n.to_string()).unwrap_or_default();
	// Rows from before results were kept only know whether they were allowed.
	let result = match entry.result {
		Some(result) => result.as_str(),
		None if entry.allowed => "ok",
		None => "denied",
	};
	vec![
		entry.at.to_rfc3339(),
		entry.grant_id.to_string(),
		text(&entry.grant_label),
		text(&entry.client),
		entry
			.session
			.as_deref()
			.map(|s| s.chars().take(8).collect())
			.unwrap_or_default(),
		entry.op.clone(),
		entry.path.clone(),
		text(&entry.pattern),
		number(entry.bytes),
		result.to_string(),
		number(entry.duration_ms),
		text(&entry.reason),
	]
}

fn row(grant: &GrantInfo) -> Vec<String> {
	vec![
		grant.id.to_string(),
		grant.subtree.display().to_string(),
		grant.caps.as_str().to_string(),
		format!("{:?}", grant.status).to_lowercase(),
		grant.expires_at.to_rfc3339(),
		grant
			.last_used_at
			.map(|at| at.to_rfc3339())
			.unwrap_or_else(|| "never".to_string()),
		grant.request_count.to_string(),
		grant.label.clone().unwrap_or_default(),
	]
}

/// The environment variable printed MCP configs read the token from.
const TOKEN_ENV: &str = "SPACEDRIVE_GRANT_TOKEN";

/// The URL MCP clients connect to for the endpoint `endpoint` describes:
/// https with the certificate's host name when TLS is configured.
fn mcp_url(endpoint: &AgentMcpConfigOutput) -> String {
	let tls = endpoint.tls_cert.is_some() && endpoint.tls_key.is_some();
	sd_core::service::agent_mcp::url(&endpoint.listen, tls, &endpoint.host_names)
}

/// Ready-to-paste config for Claude Code and Codex.
///
/// The token stays in an environment variable by default, since a config
/// file with a literal token is one commit away from a public repository.
fn mcp_config(url: &str, token: &str, inline: bool) -> String {
	let header_value = if inline {
		format!("Bearer {token}")
	} else {
		format!("Bearer ${{{TOKEN_ENV}}}")
	};
	let mut out = String::new();
	out.push_str(&format!("MCP endpoint: {url}\n\n"));
	if !inline {
		out.push_str("Put the token in the environment of the agent, not in a file you commit:\n");
		out.push_str(&format!("  export {TOKEN_ENV}='{token}'\n\n"));
	}
	out.push_str("Claude Code:\n");
	out.push_str(&format!(
		"  claude mcp add --transport http spacedrive {url} --header 'Authorization: {header_value}'\n\n"
	));
	out.push_str("Claude Code project file (.mcp.json):\n");
	let json = serde_json::json!({
		"mcpServers": {
			"spacedrive": {
				"type": "http",
				"url": url,
				"headers": { "Authorization": header_value },
			}
		}
	});
	for line in serde_json::to_string_pretty(&json)
		.unwrap_or_default()
		.lines()
	{
		out.push_str(&format!("  {line}\n"));
	}
	out.push_str("\nCodex (~/.codex/config.toml):\n");
	out.push_str("  [mcp_servers.spacedrive]\n");
	out.push_str(&format!("  url = \"{url}\"\n"));
	if inline {
		out.push_str(&format!(
			"  http_headers = {{ \"Authorization\" = \"Bearer {token}\" }}\n"
		));
	} else {
		out.push_str(&format!("  bearer_token_env_var = \"{TOKEN_ENV}\"\n"));
	}
	out
}

/// Seconds in a lifetime written as `90s`, `30m`, `1h` or `2h30m`.
fn parse_ttl(text: &str) -> Result<u32> {
	let mut total: u64 = 0;
	let mut digits = String::new();
	for c in text.trim().chars() {
		if c.is_ascii_digit() {
			digits.push(c);
			continue;
		}
		let unit = match c {
			's' => 1,
			'm' => 60,
			'h' => 3600,
			_ => bail!("unknown unit {c:?} in {text:?}; use s, m or h"),
		};
		if digits.is_empty() {
			bail!("missing number before {c:?} in {text:?}");
		}
		total = digits
			.parse::<u64>()
			.ok()
			.and_then(|n| n.checked_mul(unit))
			.and_then(|seconds| total.checked_add(seconds))
			.ok_or_else(|| anyhow::anyhow!("lifetime {text:?} is too long"))?;
		digits.clear();
	}
	if !digits.is_empty() || total == 0 {
		bail!("write a lifetime with units, such as 30m or 1h, not {text:?}");
	}
	Ok(u32::try_from(total)?)
}

#[cfg(test)]
mod tests {
	use super::{mcp_config, mcp_url, parse_ttl, AgentMcpConfigOutput};

	fn endpoint(listen: &str) -> AgentMcpConfigOutput {
		AgentMcpConfigOutput {
			enabled: true,
			listen: listen.to_string(),
			tls_cert: None,
			tls_key: None,
			host_names: Vec::new(),
			block_bytes_per_hour: sd_core::service::agent_mcp::DEFAULT_BLOCK_BYTES_PER_HOUR,
		}
	}

	#[test]
	fn mcp_urls_use_https_and_the_host_name_with_tls() {
		let tls = AgentMcpConfigOutput {
			tls_cert: Some("cert.pem".into()),
			tls_key: Some("key.pem".into()),
			host_names: vec!["nas.example.ts.net".into()],
			..endpoint("100.64.0.1:8443")
		};
		assert_eq!(mcp_url(&tls), "https://nas.example.ts.net:8443/mcp");
	}

	#[test]
	fn mcp_config_keeps_the_token_in_the_environment_by_default() {
		let url = mcp_url(&endpoint("127.0.0.1:7765"));
		assert_eq!(url, "http://127.0.0.1:7765/mcp");
		let token = format!("sdg_{}", "a".repeat(64));
		let config = mcp_config(&url, &token, false);
		assert!(config.contains("Bearer ${SPACEDRIVE_GRANT_TOKEN}"));
		assert!(config.contains("bearer_token_env_var = \"SPACEDRIVE_GRANT_TOKEN\""));
		assert_eq!(config.matches(&token).count(), 1, "only the export line");
		let inline = mcp_config(&url, &token, true);
		assert!(!inline.contains("SPACEDRIVE_GRANT_TOKEN"));
		assert!(inline.contains(&format!("Bearer {token}")));
	}

	#[test]
	fn lifetimes_parse() {
		assert_eq!(parse_ttl("1h").unwrap(), 3600);
		assert_eq!(parse_ttl("30m").unwrap(), 1800);
		assert_eq!(parse_ttl("2h30m").unwrap(), 9000);
		assert_eq!(parse_ttl("90s").unwrap(), 90);
		assert!(parse_ttl("60").is_err());
		assert!(parse_ttl("1d").is_err());
		assert!(parse_ttl("h").is_err());
		assert!(parse_ttl("").is_err());
		assert!(parse_ttl("5124095576030432h").is_err());
		assert!(parse_ttl("99999999999999999999h").is_err());
	}
}
