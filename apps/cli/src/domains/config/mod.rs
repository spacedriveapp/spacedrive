use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::PathBuf;

use crate::config::CliConfig;

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
	/// Show all configuration
	Show,
	/// Get a configuration value
	Get {
		/// Configuration key (e.g., "update.repo", "update.channel")
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

pub async fn run(data_dir: PathBuf, cmd: ConfigCmd) -> Result<()> {
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
				_ => return Err(anyhow::anyhow!("Unknown config key: {}", key)),
			};
			println!("{}", value);
		}
		ConfigCmd::Set { key, value } => match key.as_str() {
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
