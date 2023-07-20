//! A system similar to SSH trusted hosts which stores the hash of trusted public keys

use std::{io, path::Path};

use tokio::{
	fs::{File, OpenOptions},
	io::{AsyncReadExt, AsyncWriteExt},
};

use crate::spacetunnel::RemoteIdentity;

pub struct TrustedHostRegistry {
	file: File,
	trusted: Vec<RemoteIdentity>,
}

impl TrustedHostRegistry {
	pub async fn new(path: impl AsRef<Path>) -> io::Result<Self> {
		let mut file = OpenOptions::new()
			.read(true)
			.write(true)
			.create(true)
			.open(path)
			.await?;
		let mut content = String::new();
		file.read_to_string(&mut content).await?;

		Ok(Self {
			file,
			trusted: content
				.lines()
				.map(|line| line.parse().unwrap())
				.collect::<Vec<_>>(),
		})
	}

	pub fn is_trusted(&self, identity: &RemoteIdentity) -> bool {
		self.trusted.contains(identity)
	}

	pub async fn add_trusted(&mut self, identity: RemoteIdentity) -> io::Result<()> {
		self.file
			.write_all(format!("{}\n", identity.to_string()).as_bytes())
			.await?;
		self.trusted.push(identity);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use tokio::fs;

	use crate::spacetunnel::Identity;

	use super::*;

	use std::path::PathBuf;

	pub struct DirWithCleanup(std::path::PathBuf);

	impl Drop for DirWithCleanup {
		fn drop(&mut self) {
			std::fs::remove_dir_all(&self.0).ok();
		}
	}

	#[tokio::test]
	async fn test_trusted_hosts() {
		let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("_tests");
		fs::create_dir(&dir).await.unwrap();
		let _guard = DirWithCleanup(dir.clone());
		let file_path = dir.join("hosts");

		assert!(!file_path.exists(), "file exists at start of test");

		let mut registry = TrustedHostRegistry::new(dir.join("hosts")).await.unwrap();
		assert!(file_path.exists(), "file was not created");

		let id = Identity::new().to_remote_identity();

		assert!(!registry.is_trusted(&id), "no identities should be trusted");

		registry.add_trusted(id.clone()).await.unwrap();
		assert!(registry.is_trusted(&id), "identity should be trusted now");

		drop(_guard);
	}
}
