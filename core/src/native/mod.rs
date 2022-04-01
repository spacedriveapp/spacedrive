// This module contains the native bindings to the core library.
pub mod methods;

#[cfg(target_os = "macos")]
pub mod swift;
