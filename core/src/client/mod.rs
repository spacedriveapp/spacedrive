use crate::{prisma, state, Core};
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
    .find_unique(prisma::Client::pub_id().equals(config.client_uuid.clone()))
    .exec()
    .await?
  {
    Some(client) => client,
    None => {
      db.client()
        .create_one(
          prisma::Client::pub_id().set(config.client_uuid.clone()),
          prisma::Client::name().set(hostname.clone()),
          vec![
            prisma::Client::platform().set(platform as i32),
            prisma::Client::online().set(true),
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

#[derive(Error, Debug)]
pub enum ClientError {
  #[error("Database error")]
  DatabaseError(#[from] prisma::QueryError),
}
