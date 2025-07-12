//! Network operation command handlers

use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::Core;
use crate::infrastructure::cli::daemon::services::StateService;
use crate::infrastructure::cli::daemon::types::{
	ConnectedDeviceInfo, DaemonCommand, DaemonResponse, PairingRequestInfo,
};

use super::CommandHandler;

/// Handler for network operation commands
pub struct NetworkHandler;

#[async_trait]
impl CommandHandler for NetworkHandler {
	async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		_state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::InitNetworking => {
				// Check if networking is already initialized
				if core.networking().is_some() {
					DaemonResponse::Ok // Networking is already available
				} else {
					// Networking not available - daemon needs to be restarted with networking
					DaemonResponse::Error(
						"Networking not available. Restart daemon with: spacedrive start --enable-networking".to_string()
					)
				}
			}

			DaemonCommand::StartNetworking => match core.start_networking().await {
				Ok(_) => DaemonResponse::Ok,
				Err(e) => DaemonResponse::Error(e.to_string()),
			},

			DaemonCommand::StopNetworking => {
				// TODO: Implement networking stop when available
				DaemonResponse::Error("Stop networking not yet implemented".to_string())
			}

			DaemonCommand::ListConnectedDevices => match core.get_connected_devices_info().await {
				Ok(devices) => {
					let connected_devices: Vec<ConnectedDeviceInfo> = devices
						.into_iter()
						.map(|device| {
							// Get connection status from networking service
							let (
								peer_id,
								connection_active,
								connected_at,
								bytes_sent,
								bytes_received,
							) = if let Some(_networking) = core.networking() {
								// Try to get connection details - this is a simplified version
								// In a real implementation, we'd access the connection registry
								("unknown".to_string(), true, Some("now".to_string()), 0, 0)
							} else {
								("unavailable".to_string(), false, None, 0, 0)
							};

							ConnectedDeviceInfo {
								device_id: device.device_id,
								device_name: device.device_name,
								device_type: format!("{:?}", device.device_type),
								os_version: device.os_version,
								app_version: device.app_version,
								peer_id,
								status: "connected".to_string(),
								connection_active,
								last_seen: device
									.last_seen
									.format("%Y-%m-%d %H:%M:%S UTC")
									.to_string(),
								connected_at,
								bytes_sent,
								bytes_received,
							}
						})
						.collect();

					DaemonResponse::ConnectedDevices(connected_devices)
				}
				Err(e) => DaemonResponse::Error(e.to_string()),
			},

			DaemonCommand::RevokeDevice { device_id } => {
				if let Some(networking) = core.networking() {
					let service = &*networking;
					let device_registry = service.device_registry();
					let result = {
						let mut registry = device_registry.write().await;
						registry.remove_device(device_id)
					};
					match result {
						Ok(_) => DaemonResponse::Ok,
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("Networking not initialized".to_string())
				}
			}

			DaemonCommand::SendSpacedrop {
				device_id,
				file_path,
				sender_name,
				message,
			} => {
				if let Some(networking) = core.networking() {
					let service = &*networking;

					// Create spacedrop request message
					let transfer_id = uuid::Uuid::new_v4();
					let spacedrop_request = serde_json::json!({
						"transfer_id": transfer_id,
						"file_path": file_path,
						"sender_name": sender_name,
						"message": message,
						"file_size": std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0)
					});

					match service
						.send_message(
							device_id,
							"spacedrop",
							serde_json::to_vec(&spacedrop_request).unwrap_or_default(),
						)
						.await
					{
						Ok(_) => DaemonResponse::SpacedropStarted { transfer_id },
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("Networking not initialized".to_string())
				}
			}

			// Pairing commands
			DaemonCommand::StartPairingAsInitiator => {
				if let Some(networking) = core.networking() {
					let service = &*networking;
					match service.start_pairing_as_initiator().await {
						Ok((code, expires_in_seconds)) => DaemonResponse::PairingCodeGenerated {
							code,
							expires_in_seconds,
						},
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("Networking not initialized".to_string())
				}
			}

			DaemonCommand::StartPairingAsJoiner { code } => {
				if let Some(networking) = core.networking() {
					let service = &*networking;
					match service.start_pairing_as_joiner(&code).await {
						Ok(_) => DaemonResponse::PairingInProgress,
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("Networking not initialized".to_string())
				}
			}

			DaemonCommand::GetPairingStatus => {
				if let Some(networking) = core.networking() {
					let service = &*networking;
					match service.get_pairing_status().await {
						Ok(sessions) => {
							// Convert sessions to status format for compatibility
							if let Some(session) = sessions.first() {
								let status = match &session.state {
									crate::networking::PairingState::Idle => "idle",
									crate::networking::PairingState::GeneratingCode => {
										"generating_code"
									}
									crate::networking::PairingState::Broadcasting => "broadcasting",
									crate::networking::PairingState::Scanning => "scanning",
									crate::networking::PairingState::WaitingForConnection => {
										"waiting_for_connection"
									}
									crate::networking::PairingState::Connecting => "connecting",
									crate::networking::PairingState::Authenticating => "authenticating",
									crate::networking::PairingState::ExchangingKeys => {
										"exchanging_keys"
									}
									crate::networking::PairingState::AwaitingConfirmation => {
										"awaiting_confirmation"
									}
									crate::networking::PairingState::EstablishingSession => {
										"establishing_session"
									}
									crate::networking::PairingState::ChallengeReceived { .. } => {
										"authenticating"
									}
									crate::networking::PairingState::ResponseSent => "authenticating",
									crate::networking::PairingState::Completed => "completed",
									crate::networking::PairingState::Failed { .. } => "failed",
									crate::networking::PairingState::ResponsePending { .. } => {
										"responding"
									}
								}
								.to_string();

								DaemonResponse::PairingStatus {
									status,
									remote_device: None, // No device info available yet in new system
								}
							} else {
								DaemonResponse::PairingStatus {
									status: "no_active_pairing".to_string(),
									remote_device: None,
								}
							}
						}
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("Networking not initialized".to_string())
				}
			}

			DaemonCommand::ListPendingPairings => {
				if let Some(networking) = core.networking() {
					let service = &*networking;
					match service.get_pairing_status().await {
						Ok(sessions) => {
							// Convert active pairing sessions to pending requests
							let pairing_requests: Vec<PairingRequestInfo> = sessions
								.into_iter()
								.filter(|session| {
									matches!(
										session.state,
										crate::networking::PairingState::WaitingForConnection
									)
								})
								.map(|session| PairingRequestInfo {
									request_id: session.id,
									device_id: session.remote_device_id.unwrap_or(session.id),
									device_name: "Unknown Device".to_string(),
									received_at: session.created_at.to_string(),
								})
								.collect();
							DaemonResponse::PendingPairings(pairing_requests)
						}
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("Networking not initialized".to_string())
				}
			}

			DaemonCommand::AcceptPairing {
				request_id: _request_id,
			} => {
				// Pairing acceptance is handled automatically in the new system
				DaemonResponse::Ok
			}

			DaemonCommand::RejectPairing {
				request_id: _request_id,
			} => {
				// For now, just acknowledge - in full implementation we'd cancel the session
				DaemonResponse::Ok
			}

			_ => DaemonResponse::Error("Invalid command for network handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(
			cmd,
			DaemonCommand::InitNetworking
				| DaemonCommand::StartNetworking
				| DaemonCommand::StopNetworking
				| DaemonCommand::ListConnectedDevices
				| DaemonCommand::RevokeDevice { .. }
				| DaemonCommand::SendSpacedrop { .. }
				| DaemonCommand::StartPairingAsInitiator
				| DaemonCommand::StartPairingAsJoiner { .. }
				| DaemonCommand::GetPairingStatus
				| DaemonCommand::ListPendingPairings
				| DaemonCommand::AcceptPairing { .. }
				| DaemonCommand::RejectPairing { .. }
		)
	}
}