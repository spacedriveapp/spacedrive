//! Enhanced API using SdPath for true VDFS operations

use crate::{
	operations::file_ops,
	shared::types::{SdPath, SdPathBatch},
	Core,
};
use async_graphql::{Context, InputObject, Object, Result};
use std::sync::Arc;
use uuid::Uuid;

/// File operations mutations for GraphQL
pub struct FileOpsMutation;

#[Object]
impl FileOpsMutation {
	/// Copy files - now with cross-device support!
	async fn copy_files(
		&self,
		ctx: &Context<'_>,
		input: CopyFilesInput,
	) -> Result<CopyFilesResult> {
		let core = ctx.data::<Arc<Core>>()?;

		// Convert input paths to SdPaths
		let sources = SdPathBatch::new(input.sources.into_iter().map(|s| s.into()).collect());

		let destination = input.destination.into();

		let options = file_ops::FileOpOptions {
			overwrite: input.overwrite.unwrap_or(false),
			preserve_timestamps: input.preserve_timestamps.unwrap_or(true),
			update_index: true,
			progress: None,
		};

		// This single call handles:
		// - Local to local
		// - Local to remote
		// - Remote to local
		// - Remote to remote
		let results = file_ops::copy::copy_files(core, sources, destination, options).await?;

		Ok(CopyFilesResult {
			successful: results.iter().filter(|r| r.success).count(),
			failed: results.iter().filter(|r| !r.success).count(),
		})
	}

	/// Move files across devices
	async fn move_files(
		&self,
		ctx: &Context<'_>,
		input: MoveFilesInput,
	) -> Result<MoveFilesResult> {
		// Similar to copy, but removes source after successful transfer
		todo!("Implement cross-device move")
	}

	/// Delete files on any device
	async fn delete_files(
		&self,
		ctx: &Context<'_>,
		input: DeleteFilesInput,
	) -> Result<DeleteFilesResult> {
		// Can delete files on remote devices
		todo!("Implement cross-device delete")
	}
}

/// Input for file operations using SdPath
#[derive(InputObject)]
pub struct SdPathInput {
	/// Device ID (None means current device)
	pub device_id: Option<Uuid>,

	/// Path on that device
	pub path: String,

	/// Library ID (None means current library)
	pub library_id: Option<Uuid>,
}

impl From<SdPathInput> for SdPath {
	fn from(input: SdPathInput) -> Self {
		match input.device_id {
			Some(device_id) => SdPath::new(device_id, input.path),
			None => SdPath::local(input.path),
		}
	}
}

#[derive(InputObject)]
struct CopyFilesInput {
	/// Source files (can be on different devices!)
	sources: Vec<SdPathInput>,

	/// Destination directory
	destination: SdPathInput,

	/// Options
	overwrite: Option<bool>,
	preserve_timestamps: Option<bool>,
}

#[derive(InputObject)]
struct MoveFilesInput {
	sources: Vec<SdPathInput>,
	destination: SdPathInput,
}

#[derive(InputObject)]
struct DeleteFilesInput {
	paths: Vec<SdPathInput>,
	use_trash: Option<bool>,
}

#[derive(async_graphql::SimpleObject)]
struct CopyFilesResult {
	successful: usize,
	failed: usize,
}

#[derive(async_graphql::SimpleObject)]
struct MoveFilesResult {
	successful: usize,
	failed: usize,
}

#[derive(async_graphql::SimpleObject)]
struct DeleteFilesResult {
	successful: usize,
	failed: usize,
}

/// Example GraphQL queries showing the power of SdPath
///
/// ```graphql
/// # Copy file from MacBook to iPhone
/// mutation {
///   copyFiles(input: {
///     sources: [{
///       deviceId: "aaaa-bbbb-cccc-dddd",
///       path: "/Users/jamie/Documents/vacation.mp4"
///     }],
///     destination: {
///       deviceId: "1111-2222-3333-4444",
///       path: "/var/mobile/Documents/Videos"
///     }
///   }) {
///     successful
///     failed
///   }
/// }
///
/// # Copy files from multiple devices to NAS
/// mutation {
///   copyFiles(input: {
///     sources: [
///       { deviceId: "macbook", path: "/Users/jamie/photo1.jpg" },
///       { deviceId: "iphone", path: "/DCIM/photo2.jpg" },
///       { deviceId: "windows-pc", path: "C:\\Users\\Jamie\\photo3.jpg" }
///     ],
///     destination: {
///       deviceId: "nas",
///       path: "/volume1/photos/2024"
///     }
///   }) {
///     successful
///   }
/// }
///
/// # Local operation (no deviceId = current device)
/// mutation {
///   copyFiles(input: {
///     sources: [{ path: "/home/user/file.txt" }],
///     destination: { path: "/home/user/backup" }
///   }) {
///     successful
///   }
/// }
/// ```
pub struct GraphQLExamples;