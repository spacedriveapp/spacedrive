use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OAuthToken {
	pub access_token: String,
	pub refresh_token: String,
	pub token_type: String,
	pub expires_in: i32,
}

impl OAuthToken {
	pub fn to_header(&self) -> String {
		format!("{} {}", self.token_type, self.access_token)
	}
}

pub const DEVICE_CODE_URN: &str = "urn:ietf:params:oauth:grant-type:device_code";
