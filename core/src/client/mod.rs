use crate::{
	prisma::{self, client},
	state, Core, CoreContext,
};
use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Client {
	pub uuid: String,
	pub name: String,
	pub platform: Platform,
	pub tcp_address: String,
	#[ts(type = "string")]
	pub last_seen: DateTime<Utc>,
	#[ts(type = "string")]
	pub last_synchronized: DateTime<Utc>,
}

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

// impl Into<Client> for client::Data {
//   fn into(self) -> Client {
//     Client {
//       uuid: self.pub_id,
//       name: self.name,
//       platform: ,
//       tcp_address: self.tcp_address,
//       last_seen: self.last_seen,
//       last_synchronized: self.last_synchronized,
//     }
//   }
// }

pub async fn create(core: &Core) -> Result<(), ClientError> {
	println!("Creating client...");
	let mut config = state::client::get();

	let db = &core.database;

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

	let client = match db
		.client()
		.find_unique(client::pub_id::equals(config.client_uuid.clone()))
		.exec()
		.await?
	{
		Some(client) => client,
		None => {
			db.client()
				.create(
					client::pub_id::set(config.client_uuid.clone()),
					client::name::set(hostname.clone()),
					vec![
						client::platform::set(platform as i32),
						client::online::set(Some(true)),
					],
				)
				.exec()
				.await?
		}
	};

	config.client_name = hostname;
	config.client_id = client.id;
	config.save();

	println!("Client: {:?}", &client);

	Ok(())
}

pub async fn get_clients(ctx: &CoreContext) -> Result<Vec<client::Data>, ClientError> {
	let db = &ctx.database;

	let client = db.client().find_many(vec![]).exec().await?;

	Ok(client)
}

#[derive(Error, Debug)]
pub enum ClientError {
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
	#[error("Client not found error")]
	ClientNotFound,
}
