//! Unit tests for proxy pairing protocol components
//!
//! These tests focus on the protocol-level functionality without requiring
//! full Core instances.

use chrono::Utc;
use sd_core::service::network::device::{DeviceInfo, PairingType, SessionKeys};
use sd_core::service::network::protocol::pairing::{
	VouchPayload, VouchState, VouchStatus, VouchingSession, VouchingSessionState,
};
use uuid::Uuid;

#[test]
fn test_vouching_session_creation() {
	let session_id = Uuid::new_v4();
	let vouchee_device_id = Uuid::new_v4();
	let voucher_device_id = Uuid::new_v4();

	let session = VouchingSession {
		id: session_id,
		vouchee_device_id,
		vouchee_device_name: "Test Device".to_string(),
		voucher_device_id,
		created_at: Utc::now(),
		state: VouchingSessionState::Pending,
		vouches: vec![],
	};

	assert_eq!(session.id, session_id);
	assert_eq!(session.vouchee_device_id, vouchee_device_id);
	assert_eq!(session.voucher_device_id, voucher_device_id);
	assert!(matches!(session.state, VouchingSessionState::Pending));
	assert_eq!(session.vouches.len(), 0);
}

#[test]
fn test_vouch_state_lifecycle() {
	let device_id = Uuid::new_v4();

	// Start as Selected
	let mut vouch = VouchState {
		device_id,
		device_name: "Target Device".to_string(),
		status: VouchStatus::Selected,
		updated_at: Utc::now(),
		reason: None,
	};

	assert!(matches!(vouch.status, VouchStatus::Selected));

	// Move to Queued (offline device)
	vouch.status = VouchStatus::Queued;
	vouch.updated_at = Utc::now();
	assert!(matches!(vouch.status, VouchStatus::Queued));

	// Move to Waiting (vouch sent)
	vouch.status = VouchStatus::Waiting;
	vouch.updated_at = Utc::now();
	assert!(matches!(vouch.status, VouchStatus::Waiting));

	// Final state: Accepted
	vouch.status = VouchStatus::Accepted;
	vouch.updated_at = Utc::now();
	assert!(matches!(vouch.status, VouchStatus::Accepted));
	assert_eq!(vouch.reason, None);
}

#[test]
fn test_vouch_rejection_with_reason() {
	let device_id = Uuid::new_v4();

	let vouch = VouchState {
		device_id,
		device_name: "Target Device".to_string(),
		status: VouchStatus::Rejected,
		updated_at: Utc::now(),
		reason: Some("User rejected proxy pairing".to_string()),
	};

	assert!(matches!(vouch.status, VouchStatus::Rejected));
	assert_eq!(
		vouch.reason.as_ref().unwrap(),
		"User rejected proxy pairing"
	);
}

#[test]
fn test_vouching_session_with_multiple_vouches() {
	let session_id = Uuid::new_v4();
	let vouchee_device_id = Uuid::new_v4();
	let voucher_device_id = Uuid::new_v4();

	let device1 = Uuid::new_v4();
	let device2 = Uuid::new_v4();
	let device3 = Uuid::new_v4();

	let mut session = VouchingSession {
		id: session_id,
		vouchee_device_id,
		vouchee_device_name: "New Device".to_string(),
		voucher_device_id,
		created_at: Utc::now(),
		state: VouchingSessionState::InProgress,
		vouches: vec![
			VouchState {
				device_id: device1,
				device_name: "Device 1".to_string(),
				status: VouchStatus::Accepted,
				updated_at: Utc::now(),
				reason: None,
			},
			VouchState {
				device_id: device2,
				device_name: "Device 2".to_string(),
				status: VouchStatus::Waiting,
				updated_at: Utc::now(),
				reason: None,
			},
			VouchState {
				device_id: device3,
				device_name: "Device 3".to_string(),
				status: VouchStatus::Queued,
				updated_at: Utc::now(),
				reason: None,
			},
		],
	};

	assert_eq!(session.vouches.len(), 3);

	// Count vouches by status
	let accepted = session
		.vouches
		.iter()
		.filter(|v| matches!(v.status, VouchStatus::Accepted))
		.count();
	let waiting = session
		.vouches
		.iter()
		.filter(|v| matches!(v.status, VouchStatus::Waiting))
		.count();
	let queued = session
		.vouches
		.iter()
		.filter(|v| matches!(v.status, VouchStatus::Queued))
		.count();

	assert_eq!(accepted, 1);
	assert_eq!(waiting, 1);
	assert_eq!(queued, 1);

	// Mark session as completed when all vouches are in terminal state
	session.vouches[1].status = VouchStatus::Accepted;
	session.vouches[2].status = VouchStatus::Rejected;
	session.vouches[2].reason = Some("Offline".to_string());

	let all_terminal = session.vouches.iter().all(|v| {
		matches!(
			v.status,
			VouchStatus::Accepted | VouchStatus::Rejected | VouchStatus::Unreachable
		)
	});

	assert!(all_terminal);
	session.state = VouchingSessionState::Completed;
	assert!(matches!(session.state, VouchingSessionState::Completed));
}

#[test]
fn test_vouch_payload_structure() {
	let session_id = Uuid::new_v4();
	let vouchee_device_id = Uuid::new_v4();
	let vouchee_public_key = vec![1, 2, 3, 4, 5];

	let device_info = DeviceInfo {
		device_id: vouchee_device_id,
		device_name: "Test Device".to_string(),
		device_slug: "test-device".to_string(),
		device_type: sd_core::service::network::device::DeviceType::Desktop,
		os_version: "Test OS 1.0".to_string(),
		app_version: "1.0.0".to_string(),
		network_fingerprint: sd_core::service::network::utils::identity::NetworkFingerprint {
			node_id: "test_node_id".to_string(),
			public_key_hash: "abcdef1234567890".to_string(),
		},
		last_seen: Utc::now(),
	};

	let timestamp = Utc::now();

	let payload = VouchPayload {
		vouchee_device_id,
		vouchee_public_key: vouchee_public_key.clone(),
		vouchee_device_info: device_info.clone(),
		timestamp,
		session_id,
	};

	assert_eq!(payload.vouchee_device_id, vouchee_device_id);
	assert_eq!(payload.vouchee_public_key, vouchee_public_key);
	assert_eq!(payload.vouchee_device_info.device_id, vouchee_device_id);
	assert_eq!(payload.session_id, session_id);
}

#[test]
fn test_pairing_type_serialization() {
	use serde_json;

	// Test Direct pairing type
	let direct = PairingType::Direct;
	let direct_json = serde_json::to_string(&direct).unwrap();
	let direct_deserialized: PairingType = serde_json::from_str(&direct_json).unwrap();
	assert!(matches!(direct_deserialized, PairingType::Direct));

	// Test Proxied pairing type
	let proxied = PairingType::Proxied;
	let proxied_json = serde_json::to_string(&proxied).unwrap();
	let proxied_deserialized: PairingType = serde_json::from_str(&proxied_json).unwrap();
	assert!(matches!(proxied_deserialized, PairingType::Proxied));
}

#[test]
fn test_vouching_session_state_transitions() {
	let mut session = VouchingSession {
		id: Uuid::new_v4(),
		vouchee_device_id: Uuid::new_v4(),
		vouchee_device_name: "Test Device".to_string(),
		voucher_device_id: Uuid::new_v4(),
		created_at: Utc::now(),
		state: VouchingSessionState::Pending,
		vouches: vec![],
	};

	// Start as Pending
	assert!(matches!(session.state, VouchingSessionState::Pending));

	// Transition to InProgress when vouching starts
	session.state = VouchingSessionState::InProgress;
	assert!(matches!(session.state, VouchingSessionState::InProgress));

	// Transition to Completed when all vouches are processed
	session.state = VouchingSessionState::Completed;
	assert!(matches!(session.state, VouchingSessionState::Completed));
}

#[test]
fn test_session_keys_for_proxy_pairing() {
	// Simulate session keys derived for proxy pairing
	let shared_secret = vec![42u8; 32];
	let session_keys = SessionKeys::from_shared_secret(shared_secret);

	assert_eq!(session_keys.send_key.len(), 32);
	assert_eq!(session_keys.receive_key.len(), 32);
	assert_ne!(session_keys.send_key, session_keys.receive_key);
}

#[test]
fn test_vouch_status_enum_values() {
	// Ensure all VouchStatus variants are constructible
	let _selected = VouchStatus::Selected;
	let _queued = VouchStatus::Queued;
	let _waiting = VouchStatus::Waiting;
	let _accepted = VouchStatus::Accepted;
	let _rejected = VouchStatus::Rejected;
	let _unreachable = VouchStatus::Unreachable;

	// Test that we can match on terminal states
	let terminal_statuses = vec![
		VouchStatus::Accepted,
		VouchStatus::Rejected,
		VouchStatus::Unreachable,
	];

	for status in terminal_statuses {
		assert!(matches!(
			status,
			VouchStatus::Accepted | VouchStatus::Rejected | VouchStatus::Unreachable
		));
	}
}

#[test]
fn test_vouching_session_cleanup_timing() {
	let session = VouchingSession {
		id: Uuid::new_v4(),
		vouchee_device_id: Uuid::new_v4(),
		vouchee_device_name: "Test Device".to_string(),
		voucher_device_id: Uuid::new_v4(),
		created_at: Utc::now(),
		state: VouchingSessionState::Completed,
		vouches: vec![],
	};

	// Sessions should be cleaned up 1 hour after completion
	// This test just verifies the structure supports timing-based cleanup
	let cleanup_delay = chrono::Duration::hours(1);
	let cleanup_time = session.created_at + cleanup_delay;

	assert!(cleanup_time > session.created_at);
	assert!(cleanup_time > Utc::now() || session.created_at < Utc::now() - cleanup_delay);
}
