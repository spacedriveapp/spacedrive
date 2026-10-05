use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::PathBuf;

use crate::config::CliConfig;
use sd_client::CoreClient;
use sd_core::ops::config::app::{
	get::AppConfigOutput, GetAppConfigQueryInput, UpdateAppConfigInput, UpdateAppConfigOutput,
};

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
	/// Show all configuration
	Show,
	/// Get a configuration value
	Get {
		/// Configuration key (e.g., "update.repo", "update.channel",
		/// "replication.max_bytes_per_sec")
		key: String,
	},
	/// Set a configuration value
	Set {
		/// Configuration key
		key: String,
		/// Configuration value
		value: String,
	},
}

/// Daemon settings read and written through the running daemon, so a
/// change applies to transfers already in flight and lands in
/// spacedrive.json rather than the CLI's own file.
const REPLICATION_MAX_BYTES_PER_SEC: &str = "replication.max_bytes_per_sec";

async fn daemon_config(socket_addr: &str) -> Result<AppConfigOutput> {
	let core = CoreClient::new(socket_addr.to_string());
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
			if let Ok(daemon) = daemon_config(&socket_addr).await {
				table.add_row(vec![
					REPLICATION_MAX_BYTES_PER_SEC,
					&format_rate(daemon.replication.max_bytes_per_sec),
				]);
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
				REPLICATION_MAX_BYTES_PER_SEC => daemon_config(&socket_addr)
					.await?
					.replication
					.max_bytes_per_sec
					.to_string(),
				_ => return Err(anyhow::anyhow!("Unknown config key: {}", key)),
			};
			println!("{}", value);
		}
		ConfigCmd::Set { key, value } => match key.as_str() {
			REPLICATION_MAX_BYTES_PER_SEC => {
				let rate = parse_byte_rate(&value)?;
				let core = CoreClient::new(socket_addr.clone());
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

fn format_rate(bytes_per_sec: u64) -> String {
	if bytes_per_sec == 0 {
		return "0 (unlimited)".to_string();
	}
	format!("{bytes_per_sec} B/s")
}

#[cfg(test)]
mod tests {
	use super::parse_byte_rate;

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
