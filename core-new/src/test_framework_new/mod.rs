//! Cargo Test Subprocess Framework for Spacedrive
//! 
//! This framework allows test logic to remain in test files while still providing
//! subprocess isolation for multi-device networking tests. It uses `cargo test` 
//! as the subprocess executor, coordinated via environment variables.

pub mod runner;

pub use runner::CargoTestRunner;