use crate::{
	prisma::{self, node},
	Node,
};
use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use log::info;
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;
use ts_rs::TS;

mod state;

pub use state::*;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LibraryNode {
	pub uuid: String,
	pub name: String,
	pub platform: Platform,
	pub last_seen: DateTime<Utc>,
}

impl From<node::Data> for LibraryNode {
	fn from(data: node::Data) -> Self {
		Self {
			uuid: data.pub_id,
			name: data.name,
			platform: IntEnum::from_int(data.platform).unwrap(),
			last_seen: data.last_seen.into(),
		}
	}
}

impl From<Box<node::Data>> for LibraryNode {
	fn from(data: Box<node::Data>) -> Self {
		Self::from(*data)
	}
}

#[allow(clippy::upper_case_acronyms)]
#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum Platform {
	Unknown = 0,
	Windows = 1,
	MacOS = 2,
	Linux = 3,
	IOS = 4,
	Android = 5,
}

impl LibraryNode {
	pub async fn create(node: &Node) -> Result<(), NodeError> {
		info!("Creating node...");
		let mut config = state::get_nodestate();

		let hostname = match hostname::get() {
			Ok(hostname) => hostname.to_str().unwrap_or_default().to_owned(),
			Err(_) => "unknown".to_owned(),
		};

		let platform = match env::consts::OS {
			"windows" => Platform::Windows,
			"macos" => Platform::MacOS,
			"linux" => Platform::Linux,
			_ => Platform::Unknown,
		};

		let node = if let Some(node) = node
			.database
			.node()
			.find_unique(node::pub_id::equals(config.node_pub_id.clone()))
			.exec()
			.await?
		{
			node
		} else {
			node.database
				.node()
				.create(
					node::pub_id::set(config.node_pub_id.clone()),
					node::name::set(hostname.clone()),
					vec![node::platform::set(platform as i32)],
				)
				.exec()
				.await?
		};

		config.node_name = hostname;
		config.node_id = node.id;
		config.save().await;

		info!("node: {:?}", node);

		Ok(())
	}

	// pub async fn get_nodes(ctx: &CoreContext) -> Result<Vec<node::Data>, NodeError> {
	// 	let db = &ctx.database;

	// 	let _node = db.node().find_many(vec![]).exec().await?;

	// 	Ok(_node)
	// }
}

#[derive(Error, Debug)]
pub enum NodeError {
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
}
