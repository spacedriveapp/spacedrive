//! Session persistence for pairing protocol

use super::types::{PairingSession, PairingState};
use crate::service::network::{NetworkingError, Result};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// Serializable version of PairingSession for persistence
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct SerializablePairingSession {
	pub id: Uuid,
	pub state: SerializablePairingState,
	pub remote_device_id: Option<Uuid>,
	pub remote_public_key: Option<Vec<u8>>,
	pub shared_secret: Option<Vec<u8>>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Serializable version of PairingState
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
enum SerializablePairingState {
	WaitingForConnection,
	Scanning,
	ChallengeReceived { challenge: Vec<u8> },
	ResponseSent,
	Completed,
	Failed { reason: String },
}

impl From<&PairingSession> for SerializablePairingSession {
	fn from(session: &PairingSession) -> Self {
		Self {
			id: session.id,
			state: match &session.state {
				PairingState::WaitingForConnection => {
					SerializablePairingState::WaitingForConnection
				}
				PairingState::Scanning => SerializablePairingState::Scanning,
				PairingState::ChallengeReceived { challenge } => {
					SerializablePairingState::ChallengeReceived {
						challenge: challenge.clone(),
					}
				}
				PairingState::ResponseSent => SerializablePairingState::ResponseSent,
				PairingState::Completed => SerializablePairingState::Completed,
				PairingState::Failed { reason } => SerializablePairingState::Failed {
					reason: reason.clone(),
				},
				// Skip non-serializable states
				_ => SerializablePairingState::Failed {
					reason: "State not serializable".to_string(),
				},
			},
			remote_device_id: session.remote_device_id,
			remote_public_key: session.remote_public_key.clone(),
			shared_secret: session.shared_secret.clone(),
			created_at: session.created_at,
		}
	}
}

impl From<SerializablePairingSession> for PairingSession {
	fn from(serializable: SerializablePairingSession) -> Self {
		Self {
			id: serializable.id,
			state: match serializable.state {
				SerializablePairingState::WaitingForConnection => {
					PairingState::WaitingForConnection
				}
				SerializablePairingState::Scanning => PairingState::Scanning,
				SerializablePairingState::ChallengeReceived { challenge } => {
					PairingState::ChallengeReceived { challenge }
				}
				SerializablePairingState::ResponseSent => PairingState::ResponseSent,
				SerializablePairingState::Completed => PairingState::Completed,
				SerializablePairingState::Failed { reason } => PairingState::Failed { reason },
			},
			remote_device_id: serializable.remote_device_id,
			remote_device_info: None, // Will be restored from device registry
			remote_public_key: serializable.remote_public_key,
			shared_secret: serializable.shared_secret,
			created_at: serializable.created_at,
		}
	}
}

/// Persisted pairing sessions data
#[derive(Debug, Serialize, Deserialize)]
struct PersistedPairingSessions {
	sessions: HashMap<Uuid, SerializablePairingSession>,
	last_saved: chrono::DateTime<chrono::Utc>,
}

/// Session persistence manager
pub struct PairingPersistence {
	data_dir: PathBuf,
	sessions_file: PathBuf,
}

impl PairingPersistence {
	/// Create a new persistence manager
	pub fn new(data_dir: impl AsRef<Path>) -> Self {
		let data_dir = data_dir.as_ref().to_path_buf();
		let networking_dir = data_dir.join("networking");
		let sessions_file = networking_dir.join("pairing_sessions.json");

		Self {
			data_dir: networking_dir,
			sessions_file,
		}
	}

	/// Save active sessions to disk
	pub async fn save_sessions(&self, sessions: &HashMap<Uuid, PairingSession>) -> Result<()> {
		// Ensure data directory exists
		if let Some(parent) = self.sessions_file.parent() {
			fs::create_dir_all(parent)
				.await
				.map_err(NetworkingError::Io)?;
		}

		// Convert to serializable format, filtering out transient states
		let serializable_sessions: HashMap<Uuid, SerializablePairingSession> = sessions
			.iter()
			.filter_map(|(id, session)| {
				// Only persist certain states
				match &session.state {
					PairingState::WaitingForConnection
					| PairingState::Scanning
					| PairingState::ChallengeReceived { .. }
					| PairingState::ResponseSent
					| PairingState::Completed => Some((*id, session.into())),
					// Don't persist transient or failed states
					_ => None,
				}
			})
			.collect();

		let persisted = PersistedPairingSessions {
			sessions: serializable_sessions,
			last_saved: chrono::Utc::now(),
		};

		// Write to temporary file first, then rename for atomic operation
		let temp_file = self.sessions_file.with_extension("tmp");
		let json_data = serde_json::to_string_pretty(&persisted)
			.map_err(|e| NetworkingError::Serialization(e))?;

		fs::write(&temp_file, json_data)
			.await
			.map_err(NetworkingError::Io)?;

		fs::rename(&temp_file, &self.sessions_file)
			.await
			.map_err(NetworkingError::Io)?;

		Ok(())
	}

	/// Load sessions from disk
	pub async fn load_sessions(&self) -> Result<HashMap<Uuid, PairingSession>> {
		if !self.sessions_file.exists() {
			return Ok(HashMap::new());
		}

		let json_data = match fs::read_to_string(&self.sessions_file).await {
			Ok(data) => data,
			Err(e) => {
				eprintln!("Failed to read pairing sessions file: {}", e);
				return Ok(HashMap::new());
			}
		};

		// Handle empty files
		if json_data.trim().is_empty() {
			eprintln!("Pairing sessions file is empty, returning empty sessions");
			return Ok(HashMap::new());
		}

		let persisted: PersistedPairingSessions = match serde_json::from_str(&json_data) {
			Ok(p) => p,
			Err(e) => {
				eprintln!(
					"Failed to parse pairing sessions JSON: {}. File may be corrupted.",
					e
				);
				// Try to rename the corrupted file for debugging
				let backup_path = self.sessions_file.with_extension("json.corrupted");
				let _ = fs::rename(&self.sessions_file, &backup_path).await;
				eprintln!("Renamed corrupted file to: {:?}", backup_path);
				return Ok(HashMap::new());
			}
		};

		// Filter out expired sessions (older than 1 hour)
		let now = chrono::Utc::now();
		let max_age = chrono::Duration::hours(1);

		let sessions: HashMap<Uuid, PairingSession> = persisted
			.sessions
			.into_iter()
			.filter_map(|(id, serializable)| {
				let age = now.signed_duration_since(serializable.created_at);
				if age <= max_age {
					Some((id, serializable.into()))
				} else {
					None
				}
			})
			.collect();

		Ok(sessions)
	}

	/// Clean up expired sessions from disk
	pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
		let sessions = self.load_sessions().await?;
		let initial_count = sessions.len();

		// Save the filtered sessions back
		self.save_sessions(&sessions).await?;

		Ok(initial_count - sessions.len())
	}

	/// Delete all persisted sessions
	pub async fn clear_all_sessions(&self) -> Result<()> {
		if self.sessions_file.exists() {
			fs::remove_file(&self.sessions_file)
				.await
				.map_err(NetworkingError::Io)?;
		}
		Ok(())
	}

	/// Get the path to the sessions file
	pub fn sessions_file_path(&self) -> &Path {
		&self.sessions_file
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	async fn create_test_persistence() -> (PairingPersistence, TempDir) {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let persistence = PairingPersistence::new(temp_dir.path());
		(persistence, temp_dir)
	}

	#[tokio::test]
	async fn test_save_and_load_sessions() {
		let (persistence, _temp_dir) = create_test_persistence().await;

		// Create test sessions
		let mut sessions = HashMap::new();
		let session_id = Uuid::new_v4();
		let session = PairingSession {
			id: session_id,
			state: PairingState::WaitingForConnection,
			remote_device_id: Some(Uuid::new_v4()),
			remote_device_info: None,
			remote_public_key: None,
			shared_secret: Some(vec![1, 2, 3, 4]),
			created_at: chrono::Utc::now(),
		};
		sessions.insert(session_id, session);

		// Save sessions
		persistence.save_sessions(&sessions).await.unwrap();

		// Load sessions
		let loaded_sessions = persistence.load_sessions().await.unwrap();

		assert_eq!(loaded_sessions.len(), 1);
		assert!(loaded_sessions.contains_key(&session_id));

		let loaded_session = &loaded_sessions[&session_id];
		assert_eq!(loaded_session.id, session_id);
		assert!(matches!(
			loaded_session.state,
			PairingState::WaitingForConnection
		));
	}

	#[tokio::test]
	async fn test_load_nonexistent_file() {
		let (persistence, _temp_dir) = create_test_persistence().await;

		let sessions = persistence.load_sessions().await.unwrap();
		assert!(sessions.is_empty());
	}

	#[tokio::test]
	async fn test_clear_sessions() {
		let (persistence, _temp_dir) = create_test_persistence().await;

		// Create and save sessions
		let mut sessions = HashMap::new();
		sessions.insert(
			Uuid::new_v4(),
			PairingSession {
				id: Uuid::new_v4(),
				state: PairingState::Completed,
				remote_device_id: None,
				remote_device_info: None,
				remote_public_key: None,
				shared_secret: None,
				created_at: chrono::Utc::now(),
			},
		);

		persistence.save_sessions(&sessions).await.unwrap();
		assert!(persistence.sessions_file.exists());

		// Clear sessions
		persistence.clear_all_sessions().await.unwrap();
		assert!(!persistence.sessions_file.exists());
	}
}
