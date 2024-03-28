mod ephemeral;
#[cfg(feature = "experimental")]
mod persistent;

pub use ephemeral::EphemeralVault;
