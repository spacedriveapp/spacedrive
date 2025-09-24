//! API surface types and operation metadata
//!
//! Types that represent the public API surface and operation metadata
//! for applications consuming the Spacedrive API.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Represents an operation available through the API
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiOperation {
	/// Operation identifier (e.g., "files.copy")
	pub identifier: String,

	/// Wire method for this operation
	pub wire_method: String,

	/// Type of operation
	pub operation_type: OperationType,

	/// Input type information
	pub input_type_name: String,

	/// Output type information
	pub output_type_name: String,

	/// Whether this operation requires authentication
	pub requires_auth: bool,

	/// Whether this operation requires library context
	pub requires_library: bool,
}

/// Classification of operation types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum OperationType {
	/// Library-scoped action (modifies library state)
	LibraryAction,

	/// Core-scoped action (modifies daemon state)
	CoreAction,

	/// Library-scoped query (reads library data)
	LibraryQuery,

	/// Core-scoped query (reads daemon data)
	CoreQuery,
}

/// Complete API surface information
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiSurface {
	/// All available operations
	pub operations: Vec<ApiOperation>,

	/// API version information
	pub version: ApiVersion,

	/// Supported features
	pub features: ApiFeatures,
}

/// API version information
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiVersion {
	/// Major version
	pub major: u32,

	/// Minor version
	pub minor: u32,

	/// Patch version
	pub patch: u32,

	/// Pre-release identifier
	pub pre_release: Option<String>,
}

/// API feature flags
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiFeatures {
	/// User authentication available
	pub user_auth: bool,

	/// Library permissions available
	pub library_permissions: bool,

	/// Network operations available
	pub networking: bool,

	/// Job system available
	pub jobs: bool,

	/// Search capabilities available
	pub search: bool,
}

impl ApiSurface {
	/// Create API surface from discovered operations
	pub fn from_operations(operations: Vec<ApiOperation>) -> Self {
		Self {
			operations,
			version: ApiVersion::current(),
			features: ApiFeatures::current(),
		}
	}

	/// Get operations by type
	pub fn operations_by_type(&self, op_type: OperationType) -> Vec<&ApiOperation> {
		self.operations
			.iter()
			.filter(|op| op.operation_type == op_type)
			.collect()
	}

	/// Find operation by identifier
	pub fn find_operation(&self, identifier: &str) -> Option<&ApiOperation> {
		self.operations
			.iter()
			.find(|op| op.identifier == identifier)
	}
}

impl ApiVersion {
	/// Current API version
	pub fn current() -> Self {
		Self {
			major: 1,
			minor: 0,
			patch: 0,
			pre_release: Some("alpha".to_string()),
		}
	}

	/// Format as semantic version string
	pub fn to_semver(&self) -> String {
		match &self.pre_release {
			Some(pre) => format!("{}.{}.{}-{}", self.major, self.minor, self.patch, pre),
			None => format!("{}.{}.{}", self.major, self.minor, self.patch),
		}
	}
}

impl ApiFeatures {
	/// Current feature set
	pub fn current() -> Self {
		Self {
			user_auth: false,           // Future feature
			library_permissions: false, // Future feature
			networking: true,
			jobs: true,
			search: true,
		}
	}
}
