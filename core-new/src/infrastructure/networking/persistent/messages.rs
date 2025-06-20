//! Universal message protocol for persistent device connections
//!
//! Provides a comprehensive message system that supports all types of device-to-device
//! communication including database sync, file transfers, real-time updates, and more.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Universal message protocol for all device communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceMessage {
	// === CORE PROTOCOLS ===
	/// Keep connection alive
	Keepalive,
	/// Response to keepalive
	KeepaliveResponse,
	/// Ping with timestamp for latency measurement
	Ping { timestamp: DateTime<Utc> },
	/// Pong response to ping
	Pong {
		original_timestamp: DateTime<Utc>,
		response_timestamp: DateTime<Utc>,
	},

	// === CONNECTION MANAGEMENT ===
	/// Initial connection establishment
	ConnectionEstablish {
		device_info: crate::networking::DeviceInfo,
		protocol_version: u32,
		capabilities: Vec<String>,
	},
	/// Acknowledge connection establishment
	ConnectionAck {
		accepted: bool,
		protocol_version: u32,
		capabilities: Vec<String>,
		reason: Option<String>,
	},
	/// Graceful connection termination
	ConnectionClose { reason: String },

	// === SESSION MANAGEMENT ===
	/// Request session key rotation
	SessionRefresh {
		new_public_key: Vec<u8>,
		signature: Vec<u8>,
		timestamp: DateTime<Utc>,
	},
	/// Acknowledge session refresh
	SessionRefreshAck {
		accepted: bool,
		new_public_key: Option<Vec<u8>>,
		signature: Option<Vec<u8>>,
		timestamp: DateTime<Utc>,
	},

	// === DATABASE SYNC ===
	/// Database synchronization operations
	// DatabaseSync {
	//     library_id: Uuid,
	//     operation: SyncOperation,
	//     data: Vec<u8>,
	//     timestamp: DateTime<Utc>,
	// },
	/// Response to database sync
	// DatabaseSyncResponse {
	//     library_id: Uuid,
	//     operation_id: Uuid,
	//     result: SyncResult,
	//     timestamp: DateTime<Utc>,
	// },

	// === FILE OPERATIONS ===
	/// Request to transfer a file
	FileTransferRequest {
		transfer_id: Uuid,
		file_path: String,
		file_size: u64,
		checksum: Option<[u8; 32]>,
		metadata: FileMetadata,
	},
	/// Response to file transfer request
	FileTransferResponse {
		transfer_id: Uuid,
		accepted: bool,
		reason: Option<String>,
	},
	/// File data chunk
	FileChunk {
		transfer_id: Uuid,
		chunk_index: u64,
		data: Vec<u8>,
		is_final: bool,
		checksum: Option<[u8; 32]>,
	},
	/// Acknowledge file chunk
	FileChunkAck {
		transfer_id: Uuid,
		chunk_index: u64,
		received: bool,
	},
	/// File transfer completion
	FileTransferComplete {
		transfer_id: Uuid,
		success: bool,
		total_chunks: u64,
		final_checksum: Option<[u8; 32]>,
	},
	/// Cancel file transfer
	FileTransferCancel { transfer_id: Uuid, reason: String },

	// === SPACEDROP INTEGRATION ===
	/// Spacedrop file sharing request
	SpacedropRequest {
		transfer_id: Uuid,
		file_metadata: FileMetadata,
		sender_name: String,
		message: Option<String>,
	},
	/// Response to Spacedrop request
	SpacedropResponse {
		transfer_id: Uuid,
		accepted: bool,
		save_path: Option<String>,
	},
	/// Spacedrop progress update
	SpacedropProgress {
		transfer_id: Uuid,
		bytes_transferred: u64,
		total_bytes: u64,
		estimated_time_remaining: Option<u64>,
	},

	// === REAL-TIME SYNC ===
	/// Location/library changes update
	LocationUpdate {
		location_id: Uuid,
		changes: Vec<LocationChange>,
		timestamp: DateTime<Utc>,
		sequence_number: u64,
	},
	/// Indexer progress notification
	IndexerProgress {
		location_id: Uuid,
		progress: IndexingProgress,
		timestamp: DateTime<Utc>,
	},
	/// File system event notification
	FileSystemEvent {
		location_id: Uuid,
		event: FsEvent,
		timestamp: DateTime<Utc>,
	},

	// === LIBRARY MANAGEMENT ===
	/// Request access to a library
	LibraryAccessRequest {
		library_id: Uuid,
		requested_permissions: Vec<Permission>,
	},
	/// Response to library access request
	LibraryAccessResponse {
		library_id: Uuid,
		granted: bool,
		permissions: Vec<Permission>,
		reason: Option<String>,
	},
	/// Library metadata update
	LibraryUpdate {
		library_id: Uuid,
		metadata: LibraryMetadata,
		timestamp: DateTime<Utc>,
	},

	// === SEARCH AND DISCOVERY ===
	/// Search request across libraries
	SearchRequest {
		query_id: Uuid,
		query: SearchQuery,
		target_libraries: Vec<Uuid>,
	},
	/// Search results
	SearchResults {
		query_id: Uuid,
		results: Vec<SearchResult>,
		is_final: bool,
	},
	/// Cancel search request
	SearchCancel { query_id: Uuid },

	// === COLLABORATION ===
	/// Real-time collaboration event
	CollaborationEvent {
		session_id: Uuid,
		event: CollabEvent,
		timestamp: DateTime<Utc>,
		sequence: u64,
	},
	/// Join collaboration session
	CollaborationJoin {
		session_id: Uuid,
		user_info: UserInfo,
	},
	/// Leave collaboration session
	CollaborationLeave {
		session_id: Uuid,
		reason: Option<String>,
	},

	// === NOTIFICATIONS ===
	/// System notification
	Notification {
		id: Uuid,
		level: NotificationLevel,
		title: String,
		message: String,
		actions: Vec<NotificationAction>,
		timestamp: DateTime<Utc>,
	},
	/// Acknowledge notification
	NotificationAck { id: Uuid, action: Option<String> },

	// === EXTENSIBLE PROTOCOL ===
	/// Custom protocol message for future extensions
	Custom {
		protocol: String, // "database-sync", "file-transfer", "spacedrop", etc.
		version: u32,
		payload: Vec<u8>,
		metadata: HashMap<String, String>,
	},
	/// Error response for any message
	Error {
		request_id: Option<Uuid>,
		error_code: String,
		message: String,
		details: Option<HashMap<String, String>>,
	},
}

/// Database synchronization operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncOperation {
	/// Push local changes to remote
	Push {
		operation_id: Uuid,
		entries: Vec<SyncEntry>,
		last_sync_timestamp: Option<DateTime<Utc>>,
	},
	/// Request changes from remote since timestamp
	Pull {
		operation_id: Uuid,
		after: DateTime<Utc>,
		limit: Option<u32>,
	},
	/// Handle sync conflict
	Conflict {
		operation_id: Uuid,
		local: SyncEntry,
		remote: SyncEntry,
		resolution_strategy: ConflictResolution,
	},
	/// Provide conflict resolution
	Resolution {
		operation_id: Uuid,
		entry: SyncEntry,
		resolved_conflicts: Vec<Uuid>,
	},
	/// Full synchronization request
	FullSync {
		operation_id: Uuid,
		since: Option<DateTime<Utc>>,
	},
}

/// Sync operation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResult {
	Success {
		entries_processed: u32,
		conflicts: Vec<SyncConflict>,
	},
	Error {
		message: String,
		retry_after: Option<DateTime<Utc>>,
	},
	PartialSuccess {
		entries_processed: u32,
		failed_entries: Vec<SyncError>,
		conflicts: Vec<SyncConflict>,
	},
}

/// File metadata for transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
	pub name: String,
	pub size: u64,
	pub mime_type: Option<String>,
	pub modified_at: Option<DateTime<Utc>>,
	pub created_at: Option<DateTime<Utc>>,
	pub is_directory: bool,
	pub permissions: Option<u32>,
	pub checksum: Option<[u8; 32]>,
	pub extended_attributes: HashMap<String, String>,
}

/// Location/library change events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocationChange {
	FileAdded {
		path: String,
		metadata: FileMetadata,
	},
	FileModified {
		path: String,
		old_metadata: FileMetadata,
		new_metadata: FileMetadata,
	},
	FileRemoved {
		path: String,
		was_directory: bool,
	},
	DirectoryAdded {
		path: String,
	},
	DirectoryRemoved {
		path: String,
	},
}

/// Indexing progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingProgress {
	pub total_files: u64,
	pub processed_files: u64,
	pub current_file: Option<String>,
	pub bytes_processed: u64,
	pub total_bytes: u64,
	pub estimated_time_remaining: Option<u64>,
	pub errors: Vec<String>,
}

/// File system events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FsEvent {
	Create { path: String },
	Modify { path: String },
	Delete { path: String },
	Rename { old_path: String, new_path: String },
	Permission { path: String, mode: u32 },
}

/// Library permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Permission {
	Read,
	Write,
	Delete,
	Admin,
	Share,
	Sync,
}

/// Library metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryMetadata {
	pub name: String,
	pub description: Option<String>,
	pub total_files: u64,
	pub total_size: u64,
	pub last_modified: DateTime<Utc>,
	pub locations: Vec<LocationInfo>,
}

/// Location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
	pub id: Uuid,
	pub name: String,
	pub path: String,
	pub is_online: bool,
	pub total_files: u64,
	pub total_size: u64,
}

/// Search query structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
	pub text: Option<String>,
	pub filters: HashMap<String, String>,
	pub sort_by: Option<String>,
	pub sort_order: SortOrder,
	pub limit: Option<u32>,
	pub offset: Option<u32>,
}

/// Sort order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
	Ascending,
	Descending,
}

/// Search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
	pub id: Uuid,
	pub title: String,
	pub path: String,
	pub file_type: String,
	pub size: Option<u64>,
	pub modified_at: Option<DateTime<Utc>>,
	pub relevance_score: f64,
	pub snippet: Option<String>,
}

/// Collaboration events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CollabEvent {
	CursorMove {
		user_id: Uuid,
		x: f64,
		y: f64,
	},
	Selection {
		user_id: Uuid,
		start: u64,
		end: u64,
	},
	TextEdit {
		user_id: Uuid,
		position: u64,
		insert: String,
		delete: u64,
	},
	FileOpen {
		user_id: Uuid,
		file_path: String,
	},
	FileClose {
		user_id: Uuid,
		file_path: String,
	},
}

/// User information for collaboration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
	pub id: Uuid,
	pub name: String,
	pub avatar_url: Option<String>,
	pub color: String,
}

/// Notification levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationLevel {
	Info,
	Warning,
	Error,
	Success,
}

/// Notification actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
	pub id: String,
	pub label: String,
	pub style: ActionStyle,
}

/// Action styles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionStyle {
	Primary,
	Secondary,
	Destructive,
}

/// Sync entry for database operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntry {
	pub id: Uuid,
	pub table: String,
	pub operation: CrudOperation,
	pub data: Vec<u8>,
	pub timestamp: DateTime<Utc>,
	pub device_id: Uuid,
	pub checksum: [u8; 32],
}

/// CRUD operations for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrudOperation {
	Create,
	Update,
	Delete,
}

/// Conflict resolution strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolution {
	UseLocal,
	UseRemote,
	Merge,
	Manual,
}

/// Sync conflicts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
	pub id: Uuid,
	pub table: String,
	pub record_id: Uuid,
	pub local_entry: SyncEntry,
	pub remote_entry: SyncEntry,
	pub resolution: Option<ConflictResolution>,
}

/// Sync errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncError {
	pub entry_id: Uuid,
	pub error: String,
	pub retryable: bool,
}

impl DeviceMessage {
	/// Get message type as string for logging/debugging
	pub fn message_type(&self) -> &'static str {
		match self {
			DeviceMessage::Keepalive => "keepalive",
			DeviceMessage::KeepaliveResponse => "keepalive_response",
			DeviceMessage::Ping { .. } => "ping",
			DeviceMessage::Pong { .. } => "pong",
			DeviceMessage::ConnectionEstablish { .. } => "connection_establish",
			DeviceMessage::ConnectionAck { .. } => "connection_ack",
			DeviceMessage::ConnectionClose { .. } => "connection_close",
			DeviceMessage::SessionRefresh { .. } => "session_refresh",
			DeviceMessage::SessionRefreshAck { .. } => "session_refresh_ack",
			// Database sync messages are currently commented out
			// DeviceMessage::DatabaseSync { .. } => "database_sync",
			// DeviceMessage::DatabaseSyncResponse { .. } => "database_sync_response",
			DeviceMessage::FileTransferRequest { .. } => "file_transfer_request",
			DeviceMessage::FileTransferResponse { .. } => "file_transfer_response",
			DeviceMessage::FileChunk { .. } => "file_chunk",
			DeviceMessage::FileChunkAck { .. } => "file_chunk_ack",
			DeviceMessage::FileTransferComplete { .. } => "file_transfer_complete",
			DeviceMessage::FileTransferCancel { .. } => "file_transfer_cancel",
			DeviceMessage::SpacedropRequest { .. } => "spacedrop_request",
			DeviceMessage::SpacedropResponse { .. } => "spacedrop_response",
			DeviceMessage::SpacedropProgress { .. } => "spacedrop_progress",
			DeviceMessage::LocationUpdate { .. } => "location_update",
			DeviceMessage::IndexerProgress { .. } => "indexer_progress",
			DeviceMessage::FileSystemEvent { .. } => "fs_event",
			DeviceMessage::LibraryAccessRequest { .. } => "library_access_request",
			DeviceMessage::LibraryAccessResponse { .. } => "library_access_response",
			DeviceMessage::LibraryUpdate { .. } => "library_update",
			DeviceMessage::SearchRequest { .. } => "search_request",
			DeviceMessage::SearchResults { .. } => "search_results",
			DeviceMessage::SearchCancel { .. } => "search_cancel",
			DeviceMessage::CollaborationEvent { .. } => "collaboration_event",
			DeviceMessage::CollaborationJoin { .. } => "collaboration_join",
			DeviceMessage::CollaborationLeave { .. } => "collaboration_leave",
			DeviceMessage::Notification { .. } => "notification",
			DeviceMessage::NotificationAck { .. } => "notification_ack",
			DeviceMessage::Custom { .. } => "custom",
			DeviceMessage::Error { .. } => "error",
		}
	}

	/// Check if message requires authentication
	pub fn requires_auth(&self) -> bool {
		!matches!(
			self,
			DeviceMessage::Keepalive
				| DeviceMessage::KeepaliveResponse
				| DeviceMessage::Ping { .. }
				| DeviceMessage::Pong { .. }
				| DeviceMessage::ConnectionEstablish { .. }
				| DeviceMessage::ConnectionAck { .. }
		)
	}

	/// Check if message is high priority (should be sent immediately)
	pub fn is_high_priority(&self) -> bool {
		matches!(
			self,
			DeviceMessage::Keepalive
				| DeviceMessage::KeepaliveResponse
				| DeviceMessage::SessionRefresh { .. }
				| DeviceMessage::SessionRefreshAck { .. }
				| DeviceMessage::ConnectionClose { .. }
				| DeviceMessage::Error { .. }
		)
	}

	/// Get estimated message size for bandwidth planning
	pub fn estimated_size(&self) -> usize {
		match self {
			DeviceMessage::FileChunk { data, .. } => data.len() + 100,
			// Database sync messages are currently commented out
			// DeviceMessage::DatabaseSync { data, .. } => data.len() + 200,
			DeviceMessage::Custom { payload, .. } => payload.len() + 150,
			_ => 200, // Conservative estimate for other message types
		}
	}
}
