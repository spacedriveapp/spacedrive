//! notify2: A better file system watcher library for Rust!
//!
// TODO: Put library lints here + do crate docs

mod event;
pub use event::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

// TODO: Use poor mans trait

// const fn assert_fn<T, TRet>(_: fn(T) -> TRet) {}

// const _: () = {
//     assert_fn::<_, String>(TODO::demo);
// };
