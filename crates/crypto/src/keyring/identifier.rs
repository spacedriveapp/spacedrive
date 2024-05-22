use crate::hashing::Hasher;

#[derive(Clone)]
pub struct Identifier {
	id: String,
	usage: String,
	application: String,
}

impl Identifier {
	#[inline]
	#[must_use]
	pub fn new(id: &str, usage: &str, application: &str) -> Self {
		Self {
			id: id.to_string(),
			usage: usage.to_string(),
			application: application.to_string(),
		}
	}

	pub fn application(&self) -> String {
		self.application.to_string()
	}

	#[inline]
	#[must_use]
	pub(super) fn hash(&self) -> String {
		format!(
			"{}:{}",
			self.application,
			Hasher::blake3_hex(&[self.id.as_bytes(), self.usage.as_bytes()].concat())
		)
	}

	#[inline]
	#[must_use]
	#[cfg(any(target_os = "ios", target_os = "macos"))]
	pub(super) fn as_apple_identifer(&self) -> String {
		format!("{} - {}", self.id, self.usage)
	}

	#[inline]
	#[must_use]
	#[cfg(all(target_os = "linux", feature = "secret-service"))]
	pub(super) fn as_sec_ser_identifier(&self) -> std::collections::HashMap<&str, &str> {
		std::collections::HashMap::from([(self.id.as_str(), self.usage.as_str())])
	}
}
