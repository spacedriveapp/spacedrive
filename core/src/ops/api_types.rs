//! API-safe type wrappers for Swift export
//!
//! This module provides wrappers that convert internal types to
//! Swift-exportable types automatically.

use crate::infra::job::handle::{JobHandle, JobReceipt};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Wrapper that converts JobHandle to JobReceipt for API export
/// This allows library actions to keep returning JobHandle internally
/// while exporting JobReceipt to Swift
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiJobHandle(pub JobReceipt);

impl From<JobHandle> for ApiJobHandle {
	fn from(handle: JobHandle) -> Self {
		Self(handle.into())
	}
}

impl From<&JobHandle> for ApiJobHandle {
	fn from(handle: &JobHandle) -> Self {
		Self(handle.into())
	}
}

/// Trait to convert internal types to API-safe types
pub trait ToApiType {
	type ApiType: Type + Serialize + for<'de> Deserialize<'de>;

	fn to_api_type(self) -> Self::ApiType;
}

impl ToApiType for JobHandle {
	type ApiType = ApiJobHandle;

	fn to_api_type(self) -> Self::ApiType {
		self.into()
	}
}
