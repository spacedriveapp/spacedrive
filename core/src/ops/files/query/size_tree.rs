//! Hierarchical size tree for the analyzer, served from arena rollups.
//!
//! The analyzer is a view over the index: directory sizes come from the
//! incrementally maintained `subtree_bytes` rollups, so this query walks a
//! few levels of the in-memory tree and never touches the filesystem.
//! Depth and per-directory fan-out are capped so a whole-volume tree ships
//! in kilobytes; everything below the cap is aggregated into `other_size`.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
	ops::indexing::state::EntryKind,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;

const MAX_DEPTH: u8 = 5;
const MAX_TOP: u8 = 64;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SizeTreeInput {
	/// Root of the tree; must live under a registered source.
	pub path: PathBuf,
	/// Levels below the root to expand (clamped to 5). Defaults to 2.
	#[serde(default)]
	pub depth: Option<u8>,
	/// Largest children kept per directory (clamped to 64). Defaults to 12.
	#[serde(default)]
	pub top: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SizeNode {
	pub name: String,
	pub path: PathBuf,
	pub size: u64,
	pub is_dir: bool,
	/// Populated for directories within the requested depth.
	pub children: Vec<SizeNode>,
	/// Direct children this directory actually has.
	pub child_count: u32,
	/// Bytes in children beyond the returned top-N.
	pub other_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SizeTreeOutput {
	pub root: SizeNode,
	pub attached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SizeTreeQuery {
	input: SizeTreeInput,
}

impl CoreQuery for SizeTreeQuery {
	type Input = SizeTreeInput;
	type Output = SizeTreeOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let depth = self.input.depth.unwrap_or(2).min(MAX_DEPTH);
		let top = self.input.top.unwrap_or(12).min(MAX_TOP) as usize;
		let path = self.input.path;

		let cache = context.ephemeral_cache();
		cache.ensure_restored(&path).await;
		let attached = !cache.is_detached(&path);

		let index = cache.resolve_index(&path);
		let index = index.read().await;

		fn build(
			index: &crate::ops::indexing::ephemeral::index::EphemeralIndex,
			path: PathBuf,
			depth: u8,
			top: usize,
		) -> SizeNode {
			let name = path
				.file_name()
				.map(|n| n.to_string_lossy().into_owned())
				.unwrap_or_else(|| path.to_string_lossy().into_owned());

			let meta = index.get_entry_ref(&path);
			let is_dir = meta
				.as_ref()
				.map(|m| m.kind == EntryKind::Directory)
				// The source root itself often has no entry row; it is a
				// directory by construction.
				.unwrap_or(true);

			if !is_dir {
				return SizeNode {
					name,
					path: path.clone(),
					size: meta.map(|m| m.size).unwrap_or(0),
					is_dir: false,
					children: Vec::new(),
					child_count: 0,
					other_size: 0,
				};
			}

			let size = index.subtree_size(&path).unwrap_or(0);
			let child_paths = index.list_directory(&path).unwrap_or_default();
			let child_count = child_paths.len() as u32;

			let mut children = Vec::new();
			let mut other_size = 0u64;

			if depth > 0 {
				// Effective size per child: rollup for dirs, own size for files.
				let mut sized: Vec<(PathBuf, u64, bool)> = child_paths
					.into_iter()
					.filter_map(|child| {
						let m = index.get_entry_ref(&child)?;
						let is_dir = m.kind == EntryKind::Directory;
						let sz = if is_dir {
							index.subtree_size(&child).unwrap_or(0)
						} else {
							m.size
						};
						Some((child, sz, is_dir))
					})
					.collect();
				sized.sort_by(|a, b| b.1.cmp(&a.1));

				for (i, (child, sz, child_is_dir)) in sized.into_iter().enumerate() {
					if i < top {
						if child_is_dir {
							children.push(build(index, child, depth - 1, top));
						} else {
							let child_name = child
								.file_name()
								.map(|n| n.to_string_lossy().into_owned())
								.unwrap_or_default();
							children.push(SizeNode {
								name: child_name,
								path: child,
								size: sz,
								is_dir: false,
								children: Vec::new(),
								child_count: 0,
								other_size: 0,
							});
						}
					} else {
						other_size += sz;
					}
				}
			}

			SizeNode {
				name,
				path,
				size,
				is_dir: true,
				children,
				child_count,
				other_size,
			}
		}

		let root = build(&index, path, depth, top);
		Ok(SizeTreeOutput { root, attached })
	}
}

crate::register_core_query!(SizeTreeQuery, "files.size_tree");
