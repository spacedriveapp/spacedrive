use crate::{Protected, Result};

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "ios")))]
pub mod portable;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "ios")))]
pub use portable::PortableKeyring as KeyringInterface;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxKeyring as KeyringInterface;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use apple::AppleKeyring as KeyringInterface;

/// This identifier is platform-agnostic and is used for identifying keys within OS keyrings
#[derive(Clone, Copy)]
pub struct Identifier<'a> {
	pub id: &'a str,
	pub application: &'a str,
	pub usage: &'a str,
}

impl<'a> Identifier<'a> {
	#[must_use]
	pub const fn new(id: &'static str, application: &'static str, usage: &'static str) -> Self {
		Self {
			id,
			application,
			usage,
		}
	}
}

pub trait Keyring {
	fn new() -> Result<Self>
	where
		Self: Sized;
	fn insert(&self, identifier: Identifier<'_>, value: Protected<Vec<u8>>) -> Result<()>;
	fn retrieve(&self, identifier: Identifier<'_>) -> Result<Protected<Vec<u8>>>;
	fn delete(&self, identifier: Identifier<'_>) -> Result<()>;
}
