//! # RPC secret
//!
//! The RPC port answers any process that can open a loopback connection, on
//! any OS account, and through the `device` field it reaches every paired
//! device too (threat model F1). At start the daemon writes a random secret
//! to `rpc.secret` in its data directory, readable only by the daemon's own
//! account, and a connection is served only after its first line presents
//! that secret. Reading the file is the proof that a client runs as the
//! daemon's user, so the port stays plain TCP on every platform.
//!
//! The secret changes on every start. Clients read the file for each
//! connection, so a restarted daemon never strands a long-lived client.
//!
//! ## Example
//! ```rust,no_run
//! # async fn example() -> std::io::Result<()> {
//! use std::path::Path;
//! use tokio::io::AsyncWriteExt;
//!
//! let mut stream =
//! 	sd_core::infra::daemon::auth::connect("127.0.0.1:6969", Path::new("/data/spacedrive"))
//! 		.await?;
//! stream.write_all(b"\"Ping\"\n").await?;
//! # Ok(())
//! # }
//! ```

use std::io;
use std::path::{Path, PathBuf};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// File name of the secret inside the daemon's data directory.
pub const SECRET_FILE: &str = "rpc.secret";

/// Longest first line the daemon reads before deciding; a real auth line is
/// about 80 bytes, so anything longer is refused without buffering it.
pub const MAX_AUTH_LINE: u64 = 256;

#[derive(Serialize, Deserialize)]
struct AuthLine {
	auth: String,
}

/// Where the daemon owning `data_dir` keeps its RPC secret.
pub fn secret_path(data_dir: &Path) -> PathBuf {
	data_dir.join(SECRET_FILE)
}

/// The secret a running daemon expects. Debug output never shows it.
pub struct RpcSecret {
	digest: [u8; 32],
}

impl std::fmt::Debug for RpcSecret {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("RpcSecret(..)")
	}
}

impl RpcSecret {
	/// Whether a connection's first line presents this secret.
	///
	/// Both sides are hashed before comparing, so the comparison takes the same
	/// time whatever the presented value is.
	pub fn verify(&self, first_line: &str) -> bool {
		let Ok(line) = serde_json::from_str::<AuthLine>(first_line.trim()) else {
			return false;
		};
		let presented: [u8; 32] = Sha256::digest(line.auth.as_bytes()).into();
		presented
			.iter()
			.zip(self.digest.iter())
			.fold(0u8, |acc, (a, b)| acc | (a ^ b))
			== 0
	}
}

/// Generate a fresh secret and write it where clients look for it.
///
/// The file is written beside its final name with owner-only permissions and
/// renamed into place, so no client ever reads a partial secret and no other
/// account can read it in between. On Windows the file inherits the data
/// directory's ACL, which grants only the user under the default profile.
pub fn issue(data_dir: &Path) -> io::Result<RpcSecret> {
	let mut bytes = [0u8; 32];
	rand::rngs::OsRng.fill_bytes(&mut bytes);
	let secret = hex::encode(bytes);

	std::fs::create_dir_all(data_dir)?;
	let path = secret_path(data_dir);
	let staging = data_dir.join(format!("{SECRET_FILE}.{}.tmp", std::process::id()));
	let _ = std::fs::remove_file(&staging);
	{
		use std::io::Write;
		let mut options = std::fs::OpenOptions::new();
		options.write(true).create_new(true);
		#[cfg(unix)]
		{
			use std::os::unix::fs::OpenOptionsExt;
			options.mode(0o600);
		}
		let mut file = options.open(&staging)?;
		file.write_all(secret.as_bytes())?;
		file.sync_all()?;
	}
	std::fs::rename(&staging, &path)?;

	Ok(RpcSecret {
		digest: Sha256::digest(secret.as_bytes()).into(),
	})
}

/// Read the secret of the daemon owning `data_dir`.
pub async fn read(data_dir: &Path) -> io::Result<String> {
	let path = secret_path(data_dir);
	let secret = tokio::fs::read_to_string(&path).await.map_err(|e| {
		io::Error::new(
			e.kind(),
			format!(
				"cannot read the daemon's RPC secret at {}: {e}; the daemon writes it at start, for its own OS user",
				path.display()
			),
		)
	})?;
	Ok(secret.trim().to_string())
}

/// The first line a client sends on every connection.
pub fn auth_line(secret: &str) -> String {
	let mut line = serde_json::to_string(&AuthLine {
		auth: secret.to_string(),
	})
	.expect("a string field always serializes");
	line.push('\n');
	line
}

/// Connect to the daemon at `socket_addr` and present the secret of the
/// daemon owning `data_dir`. The request goes on the returned stream next.
///
/// Connecting comes first so a stopped daemon reads as a refused connection,
/// as it always has, rather than as a missing secret.
pub async fn connect(socket_addr: &str, data_dir: &Path) -> io::Result<TcpStream> {
	let mut stream = TcpStream::connect(socket_addr).await?;
	let secret = read(data_dir).await?;
	stream.write_all(auth_line(&secret).as_bytes()).await?;
	Ok(stream)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_secret_verifies_only_itself() {
		let dir = tempfile::tempdir().unwrap();
		let issued = issue(dir.path()).unwrap();
		let secret = std::fs::read_to_string(secret_path(dir.path())).unwrap();
		assert_eq!(secret.len(), 64);

		assert!(issued.verify(&auth_line(&secret)));
		assert!(!issued.verify(&auth_line(&secret[..63])));
		assert!(!issued.verify(&auth_line("")));
		assert!(!issued.verify("\"Ping\""));
		assert!(!issued.verify("GET / HTTP/1.1"));
		assert!(!format!("{issued:?}").contains(&secret));
	}

	#[test]
	fn every_start_issues_a_new_secret() {
		let dir = tempfile::tempdir().unwrap();
		let first = issue(dir.path()).unwrap();
		let old = std::fs::read_to_string(secret_path(dir.path())).unwrap();
		issue(dir.path()).unwrap();
		let new = std::fs::read_to_string(secret_path(dir.path())).unwrap();
		assert_ne!(old, new);
		assert!(!first.verify(&auth_line(&new)));
	}

	#[cfg(unix)]
	#[test]
	fn only_the_owner_can_read_the_secret() {
		use std::os::unix::fs::PermissionsExt;
		let dir = tempfile::tempdir().unwrap();
		issue(dir.path()).unwrap();
		let mode = std::fs::metadata(secret_path(dir.path()))
			.unwrap()
			.permissions()
			.mode();
		assert_eq!(mode & 0o777, 0o600);
	}
}
