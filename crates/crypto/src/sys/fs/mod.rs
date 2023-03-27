mod erase;

pub use erase::erase;

#[cfg(feature = "async")]
pub use erase::erase_async;
