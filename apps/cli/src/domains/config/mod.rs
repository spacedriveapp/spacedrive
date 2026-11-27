use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::{Path, PathBuf};

use crate::config::CliConfig;
use sd_client::CoreClient;
use sd_core::ops::config::app::{
	get::AppConfigOutput, GetAppConfigQueryInput, UpdateAppConfigInput, UpdateAppConfigOutput,
};
use sd_core::ops::mounts::{MountsReplicationSetPausedInput, MountsReplicationSetPausedOutput};

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
	/// Show all configuration
	Show,
	/// Get a configuration value
	Get {
		/// Configuration key: current_library_id, update.repo,
		/// update.channel, replication.max_bytes_per_sec, replication.paused,
		/// backup.restore_trash_keep, mounts.fuse_enabled, agent_mcp.enabled,
		/// agent_mcp.listen
		key: String,
	},
	/// Set a configuration value
	Set {
		/// Configuration key: update.repo, update.channel,
		/// replication.max_bytes_per_sec (bytes, or K/M/G suffix),
		/// replication.paused (true/false), backup.restore_trash_keep (count),
		/// mounts.fuse_enabled (true/false), agent_mcp.enabled (true/false),
		/// agent_mcp.listen (loopback address)
		key: String,
		/// Configuration value
		value: String,
	},
}

/// Daemon settings read and written through the running daemon, so a
/// change applies to transfers already in flight and lands in
/// spacedrive.json rather than the CLI's own file.
const REPLICATION_MAX_BYTES_PER_SEC: &str = "replication.max_bytes_per_sec";
const REPLICATION_PAUSED: &str = "replication.paused";
const BACKUP_RESTORE_TRASH_KEEP: &str = "backup.restore_trash_keep";
const MOUNTS_FUSE_ENABLED: &str = "mounts.fuse_enabled";
const AGENT_MCP_ENABLED: &str = "agent_mcp.enabled";
const AGENT_MCP_LISTEN: &str = "agent_mcp.listen";

async fn daemon_config(socket_addr: &str, data_dir: &Path) -> Result<AppConfigOutput> {
	let core = CoreClient::new(socket_addr.to_string(), data_dir);
	core.query(&GetAppConfigQueryInput, None)
		.await
		.map_err(|e| anyhow::anyhow!("daemon config unavailable: {e}"))
}

/// Parse a byte rate: a bare count of bytes, or a count with a K, M or G
/// suffix in powers of 1024, so `sd config set replication.max_bytes_per_sec 200K`
/// reads as 200 KiB/s. Zero lifts the cap.
fn parse_byte_rate(value: &str) -> Result<u64> {
	let value = value.trim();
	let (digits, scale) = match value.chars().last() {
		Some('k') | Some('K') => (&value[..value.len() - 1], 1u64 << 10),
		Some('m') | Some('M') => (&value[..value.len() - 1], 1u64 << 20),
		Some('g') | Some('G') => (&value[..value.len() - 1], 1u64 << 30),
		_ => (value, 1),
	};
	let count: u64 = digits
		.trim()
		.parse()
		.map_err(|_| anyhow::anyhow!("'{value}' is not a byte rate (e.g. 0, 500000, 200K, 2M)"))?;
	count
		.checked_mul(scale)
		.ok_or_else(|| anyhow::anyhow!("'{value}' is too large"))
}

pub async fn run(data_dir: PathBuf, socket_addr: String, cmd: ConfigCmd) -> Result<()> {
	let mut config = CliConfig::load(&data_dir)?;

	match cmd {
		ConfigCmd::Show => {
			let mut table = Table::new();
			table.load_preset(UTF8_BORDERS_ONLY);
			table.set_header(vec!["Key", "Value"]);

			// Library settings
			if let Some(lib_id) = config.current_library_id {
				table.add_row(vec!["current_library_id", &lib_id.to_string()]);
			} else {
				table.add_row(vec!["current_library_id", "(not set)"]);
			}

			// Update settings
			table.add_row(vec!["update.repo", &config.update.repo]);
			table.add_row(vec!["update.channel", &config.update.channel]);

			// Daemon settings, when the daemon answers.
			if let Ok(daemon) = daemon_config(&socket_addr, &data_dir).await {
				table.add_row(vec![
					REPLICATION_MAX_BYTES_PER_SEC,
					&format_rate(daemon.replication.max_bytes_per_sec),
				]);
				table.add_row(vec![
					REPLICATION_PAUSED,
					&daemon.replication.paused.to_string(),
				]);
				table.add_row(vec![
					BACKUP_RESTORE_TRASH_KEEP,
					&daemon.backup.restore_trash_keep.to_string(),
				]);
				table.add_row(vec![
					MOUNTS_FUSE_ENABLED,
					&daemon.mounts.fuse_enabled.to_string(),
				]);
				table.add_row(vec![
					AGENT_MCP_ENABLED,
					&daemon.agent_mcp.enabled.to_string(),
				]);
				table.add_row(vec![AGENT_MCP_LISTEN, &daemon.agent_mcp.listen]);
			}

			println!("{}", table);
			println!();
			println!(
				"Config file: {}",
				CliConfig::config_path(&data_dir).display()
			);
		}
		ConfigCmd::Get { key } => {
			let value = match key.as_str() {
				"current_library_id" => config
					.current_library_id
					.map(|id| id.to_string())
					.unwrap_or_else(|| "(not set)".to_string()),
				"update.repo" => config.update.repo.clone(),
				"update.channel" => config.update.channel.clone(),
				REPLICATION_MAX_BYTES_PER_SEC => daemon_config(&socket_addr, &data_dir)
					.await?
					.replication
					.max_bytes_per_sec
					.to_string(),
				REPLICATION_PAUSED => daemon_config(&socket_addr, &data_dir)
					.await?
					.replication
					.paused
					.to_string(),
				BACKUP_RESTORE_TRASH_KEEP => daemon_config(&socket_addr, &data_dir)
					.await?
					.backup
					.restore_trash_keep
					.to_string(),
				MOUNTS_FUSE_ENABLED => daemon_config(&socket_addr, &data_dir)
					.await?
					.mounts
					.fuse_enabled
					.to_string(),
				AGENT_MCP_ENABLED => daemon_config(&socket_addr, &data_dir)
					.await?
					.agent_mcp
					.enabled
					.to_string(),
				AGENT_MCP_LISTEN => {
					daemon_config(&socket_addr, &data_dir)
						.await?
						.agent_mcp
						.listen
				}
				_ => return Err(anyhow::anyhow!("Unknown config key: {}", key)),
			};
			println!("{}", value);
		}
		ConfigCmd::Set { key, value } => match key.as_str() {
			REPLICATION_MAX_BYTES_PER_SEC => {
				let rate = parse_byte_rate(&value)?;
				let core = CoreClient::new(socket_addr.clone(), &data_dir);
				let input = UpdateAppConfigInput {
					replication_max_bytes_per_sec: Some(rate),
					..UpdateAppConfigInput::default()
				};
				let out: UpdateAppConfigOutput = serde_json::from_value(
					core.action(&input, None)
						.await
						.map_err(|e| anyhow::anyhow!("daemon refused the change: {e}"))?,
				)?;
				if !out.success {
					return Err(anyhow::anyhow!(out.message));
				}
				println!(
					"Set {} = {}",
					REPLICATION_MAX_BYTES_PER_SEC,
					format_rate(rate)
				);
			}
			REPLICATION_PAUSED => {
				let paused = parse_bool(&value)?;
				let core = CoreClient::new(socket_addr.clone(), &data_dir);
				let input = MountsReplicationSetPausedInput { paused };
				let out: MountsReplicationSetPausedOutput = serde_json::from_value(
					core.action(&input, None)
						.await
						.map_err(|e| anyhow::anyhow!("daemon refused the change: {e}"))?,
				)?;
				println!("Set {} = {}", REPLICATION_PAUSED, out.paused);
			}
			BACKUP_RESTORE_TRASH_KEEP => {
				let keep: u32 = value.trim().parse().map_err(|_| {
					anyhow::anyhow!("'{value}' is not a count of entries to keep (e.g. 0, 2, 5)")
				})?;
				let core = CoreClient::new(socket_addr.clone(), &data_dir);
				let input = UpdateAppConfigInput {
					backup_restore_trash_keep: Some(keep),
					..UpdateAppConfigInput::default()
				};
				let out: UpdateAppConfigOutput = serde_json::from_value(
					core.action(&input, None)
						.await
						.map_err(|e| anyhow::anyhow!("daemon refused the change: {e}"))?,
				)?;
				if !out.success {
					return Err(anyhow::anyhow!(out.message));
				}
				println!("Set {} = {}", BACKUP_RESTORE_TRASH_KEEP, keep);
			}
			MOUNTS_FUSE_ENABLED => {
				let enabled = parse_bool(&value)?;
				let core = CoreClient::new(socket_addr.clone(), &data_dir);
				let input = UpdateAppConfigInput {
					mounts_fuse_enabled: Some(enabled),
					..UpdateAppConfigInput::default()
				};
				let out: UpdateAppConfigOutput = serde_json::from_value(
					core.action(&input, None)
						.await
						.map_err(|e| anyhow::anyhow!("daemon refused the change: {e}"))?,
				)?;
				if !out.success {
					return Err(anyhow::anyhow!(out.message));
				}
				println!("Set {} = {}", MOUNTS_FUSE_ENABLED, enabled);
			}
			AGENT_MCP_ENABLED | AGENT_MCP_LISTEN => {
				let mut input = UpdateAppConfigInput::default();
				if key == AGENT_MCP_ENABLED {
					input.agent_mcp_enabled = Some(parse_bool(&value)?);
				} else {
					input.agent_mcp_listen = Some(value.trim().to_string());
				}
				let core = CoreClient::new(socket_addr.clone(), &data_dir);
				let out: UpdateAppConfigOutput = serde_json::from_value(
					core.action(&input, None)
						.await
						.map_err(|e| anyhow::anyhow!("daemon refused the change: {e}"))?,
				)?;
				if !out.success {
					return Err(anyhow::anyhow!(out.message));
				}
				println!("Set {} = {}", key, value.trim());
				if out.requires_restart {
					println!("Restart the daemon to apply: sd restart");
				}
			}
			"update.repo" => {
				config.set_update_repo(value.clone(), &data_dir)?;
				println!("Set update.repo = {}", value);
			}
			"update.channel" => {
				config.set_update_channel(value.clone(), &data_dir)?;
				println!("Set update.channel = {}", value);
			}
			_ => return Err(anyhow::anyhow!("Cannot set key: {}", key)),
		},
	}

	Ok(())
}

fn parse_bool(value: &str) -> Result<bool> {
	match value.trim().to_ascii_lowercase().as_str() {
		"true" | "yes" | "on" | "1" => Ok(true),
		"false" | "no" | "off" | "0" => Ok(false),
		_ => Err(anyhow::anyhow!("'{value}' is not a boolean (true/false)")),
	}
}

fn format_rate(bytes_per_sec: u64) -> String {
	if bytes_per_sec == 0 {
		return "0 (unlimited)".to_string();
	}
	format!("{bytes_per_sec} B/s")
}

#[cfg(test)]
mod tests {
	use super::{parse_bool, parse_byte_rate};

	#[test]
	fn pause_switch_takes_common_booleans() {
		assert!(parse_bool("true").unwrap());
		assert!(parse_bool("ON").unwrap());
		assert!(!parse_bool("0").unwrap());
		assert!(parse_bool("maybe").is_err());
	}

	#[test]
	fn byte_rates_take_binary_suffixes() {
		assert_eq!(parse_byte_rate("0").unwrap(), 0);
		assert_eq!(parse_byte_rate("500000").unwrap(), 500_000);
		assert_eq!(parse_byte_rate("200K").unwrap(), 200 << 10);
		assert_eq!(parse_byte_rate("2m").unwrap(), 2 << 20);
		assert_eq!(parse_byte_rate("1G").unwrap(), 1 << 30);
		assert!(parse_byte_rate("fast").is_err());
	}
}
