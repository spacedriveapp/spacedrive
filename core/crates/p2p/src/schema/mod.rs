use serde::{Deserialize, Serialize};

pub mod cloud_services;

/// ALPN for the Spacedrive P2P protocol
///
/// P2P with associated constants for each existing version and an alias for the latest version.
/// This application layer protocol is used when a cloud service needs to devices communicating
/// with each other, like for sending sync keys, or other strictly P2P features, like Spacedrop.
pub struct ALPN;

impl ALPN {
	pub const LATEST: &'static [u8] = Self::V1;
	pub const V1: &'static [u8] = b"sd-p2p/v1";
}

#[derive(Debug, Clone)]
pub struct Service;

impl quic_rpc::Service for Service {
	type Req = Request;

	type Res = Response;
}

#[nested_enum_utils::enum_conversions]
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
	CloudServices(cloud_services::Request),
}

#[nested_enum_utils::enum_conversions]
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
	CloudServices(cloud_services::Response),
}
