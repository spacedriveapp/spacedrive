//! Production-ready mDNS-based device discovery for pairing

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::sync::mpsc;
use mdns_sd::{ServiceDaemon, ServiceInfo, ServiceEvent};

use crate::networking::{DeviceInfo, NetworkError, Result};
use super::{PairingCode, PairingTarget};

/// Production mDNS service for pairing discovery
pub struct PairingDiscovery {
    /// mDNS service daemon
    mdns_daemon: ServiceDaemon,
    /// Current pairing service info
    current_service: Option<ServiceInfo>,
    /// Current pairing code being broadcast
    current_code: Option<PairingCode>,
    /// Device information
    device_info: DeviceInfo,
    /// Discovery event sender
    event_sender: Option<mpsc::UnboundedSender<DiscoveryEvent>>,
    /// Service type for Spacedrive pairing
    service_type: &'static str,
}

/// Discovery events
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// Pairing device discovered
    DeviceFound {
        target: PairingTarget,
        fingerprint: [u8; 16],
    },
    /// Pairing device lost
    DeviceLost {
        address: IpAddr,
    },
    /// Broadcast started
    BroadcastStarted {
        code: PairingCode,
    },
    /// Broadcast stopped
    BroadcastStopped,
    /// Discovery error
    Error {
        error: String,
    },
}

impl PairingDiscovery {
    /// Service type for Spacedrive pairing
    const SERVICE_TYPE: &'static str = "_spacedrive-pairing._tcp.local.";
    
    /// Create new pairing discovery service
    pub fn new(device_info: DeviceInfo) -> Result<Self> {
        let mdns_daemon = ServiceDaemon::new()
            .map_err(|e| NetworkError::TransportError(format!("Failed to create mDNS daemon: {}", e)))?;
        
        Ok(Self {
            mdns_daemon,
            current_service: None,
            current_code: None,
            device_info,
            event_sender: None,
            service_type: Self::SERVICE_TYPE,
        })
    }
    
    /// Set event handler for discovery events
    pub fn set_event_handler(&mut self, sender: mpsc::UnboundedSender<DiscoveryEvent>) {
        self.event_sender = Some(sender);
    }
    
    /// Start broadcasting pairing availability
    pub async fn start_broadcast(
        &mut self,
        code: &PairingCode,
        port: u16,
    ) -> Result<()> {
        self.stop_broadcast().await?;
        
        // Create unique instance name with fingerprint
        let instance_name = format!(
            "{}-{}", 
            self.device_info.device_name.replace(' ', "-").replace('.', "-"),
            hex::encode(&code.discovery_fingerprint[..4])
        );
        
        // Get local IP address for service registration
        let local_ip = self.get_local_ip_address()?;
        let host_name = format!("{}.local.", instance_name);
        
        // Create TXT record properties
        let properties = vec![
            ("fp", hex::encode(code.discovery_fingerprint)),
            ("device", self.device_info.device_name.clone()),
            ("version", "1".to_string()),
            ("expires", code.expires_at.timestamp().to_string()),
            ("device_id", self.device_info.device_id.to_string()),
        ];
        
        // Convert to string tuples for mdns-sd
        let properties_str: Vec<(&str, &str)> = properties.iter()
            .map(|(k, v)| (k.as_ref(), v.as_ref()))
            .collect();
        
        // Create service info
        let service_info = ServiceInfo::new(
            self.service_type,
            &instance_name,
            &host_name,
            &local_ip.to_string(),
            port,
            &properties_str[..],
        ).map_err(|e| NetworkError::TransportError(format!("Failed to create service info: {}", e)))?;
        
        // Register the service
        self.mdns_daemon.register(service_info.clone())
            .map_err(|e| NetworkError::TransportError(format!("Failed to register mDNS service: {}", e)))?;
        
        self.current_service = Some(service_info);
        self.current_code = Some(code.clone());
        
        // Notify event handler
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(DiscoveryEvent::BroadcastStarted {
                code: code.clone(),
            });
        }
        
        tracing::info!(
            "Started mDNS pairing broadcast for device {} on {}:{} with fingerprint {}",
            self.device_info.device_name,
            local_ip,
            port,
            hex::encode(code.discovery_fingerprint)
        );
        
        Ok(())
    }
    
    /// Stop broadcasting pairing availability
    pub async fn stop_broadcast(&mut self) -> Result<()> {
        if let Some(service_info) = self.current_service.take() {
            // Unregister the service
            self.mdns_daemon.unregister(service_info.get_fullname())
                .map_err(|e| NetworkError::TransportError(format!("Failed to unregister mDNS service: {}", e)))?;
            
            tracing::info!("Stopped mDNS pairing broadcast");
            
            // Notify event handler
            if let Some(sender) = &self.event_sender {
                let _ = sender.send(DiscoveryEvent::BroadcastStopped);
            }
        }
        
        self.current_code = None;
        Ok(())
    }
    
    /// Scan for devices broadcasting pairing availability
    pub async fn scan_for_pairing_device(
        &self,
        code: &PairingCode,
        timeout: Duration,
    ) -> Result<PairingTarget> {
        let target_fingerprint = hex::encode(code.discovery_fingerprint);
        
        tracing::info!(
            "Scanning for pairing device with fingerprint {} for {:?}",
            target_fingerprint,
            timeout
        );
        
        // Browse for Spacedrive pairing services
        let receiver = self.mdns_daemon.browse(self.service_type)
            .map_err(|e| NetworkError::TransportError(format!("Failed to start mDNS browse: {}", e)))?;
        
        // Use tokio timeout to limit scanning duration
        let result = tokio::time::timeout(timeout, async {
            loop {
                match receiver.recv() {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        tracing::debug!("Found service: {}", info.get_fullname());
                        
                        // Extract fingerprint from TXT records
                        if let Some(fp_value) = info.get_property_val_str("fp") {
                            if fp_value == target_fingerprint {
                                tracing::info!("Found matching pairing device: {}", info.get_fullname());
                                
                                // Extract device information
                                let device_name = info.get_property_val_str("device")
                                    .unwrap_or("Unknown Device").to_string();
                                
                                let expires_timestamp = info.get_property_val_str("expires")
                                    .and_then(|s| s.parse::<i64>().ok())
                                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));
                                
                                // Get IP address from service info
                                let addresses = info.get_addresses();
                                let address = if let Some(addr) = addresses.iter().next() {
                                    *addr
                                } else {
                                    return Err(NetworkError::TransportError("No IP address in service info".to_string()));
                                };
                                
                                return Ok(PairingTarget {
                                    address,
                                    port: info.get_port(),
                                    device_name,
                                    expires_at: expires_timestamp,
                                });
                            }
                        }
                    }
                    Ok(ServiceEvent::ServiceRemoved(_, _)) => {
                        // Service was removed, continue scanning
                        continue;
                    }
                    Ok(_) => {
                        // Other events, continue scanning
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!("mDNS browse error: {}", e);
                        break;
                    }
                }
            }
            
            Err(NetworkError::DeviceNotFound(uuid::Uuid::nil()))
        }).await;
        
        match result {
            Ok(target_result) => target_result,
            Err(_) => {
                tracing::warn!("mDNS scan timeout after {:?}", timeout);
                Err(NetworkError::DeviceNotFound(uuid::Uuid::nil()))
            }
        }
    }
    
    /// Start continuous scanning for pairing devices
    pub async fn start_continuous_scan(
        &mut self,
    ) -> Result<mpsc::UnboundedReceiver<DiscoveryEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_sender = Some(tx.clone());
        
        // Browse for Spacedrive pairing services
        let receiver = self.mdns_daemon.browse(self.service_type)
            .map_err(|e| NetworkError::TransportError(format!("Failed to start continuous mDNS browse: {}", e)))?;
        
        // Spawn background scanning task
        let scanner_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                match receiver.recv() {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        tracing::debug!("Discovered service: {}", info.get_fullname());
                        
                        // Extract fingerprint from TXT records
                        if let Some(fp_hex) = info.get_property_val_str("fp") {
                            if let Ok(fp_bytes) = hex::decode(&fp_hex) {
                                if fp_bytes.len() >= 16 {
                                    let mut fingerprint = [0u8; 16];
                                    fingerprint.copy_from_slice(&fp_bytes[..16]);
                                    
                                    // Extract device information
                                    let device_name = info.get_property_val_str("device")
                                        .unwrap_or("Unknown Device").to_string();
                                    
                                    let expires_timestamp = info.get_property_val_str("expires")
                                        .and_then(|s| s.parse::<i64>().ok())
                                        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));
                                    
                                    // Get IP address from service info
                                    if let Some(addr) = info.get_addresses().iter().next() {
                                        let target = PairingTarget {
                                            address: *addr,
                                            port: info.get_port(),
                                            device_name,
                                            expires_at: expires_timestamp,
                                        };
                                        
                                        let _ = scanner_tx.send(DiscoveryEvent::DeviceFound {
                                            target,
                                            fingerprint,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                        tracing::debug!("Service removed: {}", fullname);
                        // Could parse IP from fullname if needed for DeviceLost events
                    }
                    Ok(_) => {
                        // Other events, continue
                    }
                    Err(e) => {
                        tracing::warn!("mDNS browse error: {}", e);
                        let _ = scanner_tx.send(DiscoveryEvent::Error {
                            error: format!("mDNS browse error: {}", e),
                        });
                    }
                }
            }
        });
        
        Ok(rx)
    }
    
    /// Get local IP address for service registration
    fn get_local_ip_address(&self) -> Result<Ipv4Addr> {
        use local_ip_address::local_ip;
        
        match local_ip() {
            Ok(IpAddr::V4(ipv4)) => Ok(ipv4),
            Ok(IpAddr::V6(_)) => {
                // Fall back to localhost if only IPv6 is available
                tracing::warn!("Only IPv6 address available, using localhost for mDNS");
                Ok(Ipv4Addr::LOCALHOST)
            }
            Err(e) => {
                tracing::warn!("Failed to get local IP address, using localhost: {}", e);
                Ok(Ipv4Addr::LOCALHOST)
            }
        }
    }
    
    /// Get current broadcast status
    pub fn is_broadcasting(&self) -> bool {
        self.current_service.is_some()
    }
    
    /// Get current pairing code
    pub fn current_code(&self) -> Option<&PairingCode> {
        self.current_code.as_ref()
    }
}

impl Drop for PairingDiscovery {
    fn drop(&mut self) {
        if let Some(service_info) = self.current_service.take() {
            // Unregister the service on drop
            if let Err(e) = self.mdns_daemon.unregister(service_info.get_fullname()) {
                tracing::warn!("Failed to unregister mDNS service on drop: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::{identity::PublicKey, DeviceInfo};
    use uuid::Uuid;

    fn create_test_device_info() -> DeviceInfo {
        crate::networking::test_utils::test_helpers::create_test_device_info()
    }

    #[tokio::test]
    async fn test_discovery_creation() {
        let device_info = create_test_device_info();
        let discovery = PairingDiscovery::new(device_info);
        
        assert!(discovery.is_ok());
        let discovery = discovery.unwrap();
        assert!(!discovery.is_broadcasting());
        assert!(discovery.current_code().is_none());
    }

    #[tokio::test]
    async fn test_broadcast_lifecycle() {
        use super::super::PairingCode;
        
        let device_info = create_test_device_info();
        let mut discovery = PairingDiscovery::new(device_info).unwrap();
        let code = PairingCode::generate().unwrap();
        
        // Note: This test may fail in CI environments without mDNS support
        // In such cases, we'd mock the mDNS service
        let result = discovery.start_broadcast(&code, 12345).await;
        
        if result.is_ok() {
            assert!(discovery.is_broadcasting());
            assert!(discovery.current_code().is_some());
            
            let _ = discovery.stop_broadcast().await;
            assert!(!discovery.is_broadcasting());
        }
    }
}