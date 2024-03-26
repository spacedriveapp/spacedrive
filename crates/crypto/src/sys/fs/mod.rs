mod erase;

pub use erase::erase;

#[cfg(feature = "tokio")]
pub use erase::erase_async;
