//! `sd grant`: lend a folder to an agent with a scoped, expiring token.

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use uuid::Uuid;

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::grants::{
	audit::{GrantAuditInput, GrantAuditOutput},
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
			// A path for another device is that device's to resolve.
			let path = match ctx.core.device() {
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
				if o.entries.is_empty() {
					println!("No requests recorded");
					return;
				}
				for entry in &o.entries {
					let result = match &entry.reason {
						None => "allowed".to_string(),
						Some(reason) => format!("refused: {reason}"),
					};
					println!(
						"{} {} {} {} {}",
						entry.at.to_rfc3339(),
						entry.grant_id,
						entry.op,
						entry.path,
						result
					);
				}
			});
		}
	}
	Ok(())
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
		grant.label.clone().unwrap_or_default(),
	]
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
	use super::parse_ttl;

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
