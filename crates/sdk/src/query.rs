//! Query context for user queries
//!
//! Stubs for type-checking. Implementation will call WASM host functions.

use crate::agent::MemoryHandle;
use crate::vdfs::VdfsContext;
use std::marker::PhantomData;

/// Query execution context
pub struct QueryContext<M> {
	_phantom: PhantomData<M>,
}

impl<M: crate::agent::AgentMemory> QueryContext<M> {
	/// Access VDFS
	pub fn vdfs(&self) -> VdfsContext {
		VdfsContext
	}

	/// Access agent memory
	pub fn memory(&self) -> MemoryHandle<M> {
		MemoryHandle::new()
	}
}


