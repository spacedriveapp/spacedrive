use std::sync::Arc;

use dashmap::DashMap;
use sd_crypto::Protected;
use thiserror::Error;
use uuid::Uuid;

pub struct SecureTempKeystore {
	data: DashMap<Uuid, Protected<String>>,
}

impl SecureTempKeystore {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			data: DashMap::new(),
		})
	}

	pub fn tokenize(&self, data: String) -> Uuid {
		let uuid = Uuid::new_v4();
		self.data.insert(uuid, Protected::new(data));
		uuid
	}

	pub fn claim(&self, uuid: Uuid) -> Result<String, SecureTempKeystoreError> {
		let value = self
			.data
			.get(&uuid)
			.map(|v| v.value().clone())
			.ok_or(SecureTempKeystoreError::SecureItemNotFound)?;

		let sensitive_value = value.clone().as_str().to_string();

		value.zeroize();

		self.data.remove(&uuid);

		Ok(sensitive_value)
	}
}

#[derive(Error, Debug)]
pub enum SecureTempKeystoreError {
	#[error("Secure item not found")]
	SecureItemNotFound,
}
