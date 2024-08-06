use std::path::Path;

use keyring::Entry;
use regex::Regex;
use tokio::{fs, io};
use tracing::{debug, error};

mod invalidate;
mod library;

pub use invalidate::*;
pub(crate) use library::*;

/// Returns the size of the file or directory
pub async fn get_size(path: impl AsRef<Path>) -> Result<u64, io::Error> {
	let path = path.as_ref();
	let metadata = fs::metadata(path).await?;

	if metadata.is_dir() {
		let mut result = 0;
		let mut to_walk = vec![path.to_path_buf()];

		while let Some(path) = to_walk.pop() {
			let mut read_dir = fs::read_dir(&path).await?;

			while let Some(entry) = read_dir.next_entry().await? {
				let metadata = entry.metadata().await?;
				if metadata.is_dir() {
					to_walk.push(entry.path())
				} else {
					result += metadata.len()
				}
			}
		}

		Ok(result)
	} else {
		Ok(metadata.len())
	}
}

pub fn get_access_token() -> Result<String, rspc::Error> {
	let username = whoami::username();
	let entry = match Entry::new("spacedrive-auth-service", username.as_str()) {
		Ok(entry) => entry,
		Err(e) => {
			error!("Error creating entry: {}", e);
			return Err(rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Error creating entry".to_string(),
			));
		}
	};

	let data = match entry.get_password() {
		Ok(key) => key,
		Err(e) => {
			error!("Error retrieving key: {}. Does the key exist yet?", e);
			return Ok("".to_string());
		}
	};

	let re = match Regex::new(r#"st-access-token=([^;]+)"#) {
		Ok(re) => re,
		Err(e) => {
			error!("Error creating regex: {}", e);
			return Ok("".to_string());
		}
	};

	let token = match re.captures(&data) {
		Some(captures) => match captures.get(1) {
			Some(token) => token.as_str(),
			None => {
				error!("Error parsing Cookie String value: {}", "No token found");
				return Ok("".to_string());
			}
		},
		None => {
			error!("Error parsing Cookie String value: {}", "No token cookie string found");
			return Ok("".to_string());
		}
	};

	Ok(token.to_string())
}
