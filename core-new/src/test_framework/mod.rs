//! Simple multi-process testing framework for Spacedrive
//! 
//! Provides a flexible abstraction for spawning and managing multiple processes
//! for integration testing scenarios like device pairing, sync, etc.

pub mod simple_runner;
pub mod scenarios;

pub use simple_runner::{SimpleTestRunner, TestProcess};