use crate::{db, prisma::Client, state};
use anyhow::Result;
use std::env;

pub enum Platform {
    Unknown = 0,
    Windows,
    MacOS,
    Linux,
    IOS,
    Android,
}

pub async fn create() -> Result<()> {
    println!("Creating client...");
    let mut config = state::client::get();

    let db = db::get().await.expect("Could not connect to database");

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
        .find_unique(Client::uuid().equals(config.client_id.clone()))
        .exec()
        .await
    {
        Some(client) => client,
        None => {
            db.client()
                .create_one(
                    Client::uuid().set(config.client_id.clone()),
                    Client::name().set(hostname.clone()),
                    vec![
                        Client::platform().set(platform as i64),
                        Client::online().set(true),
                    ],
                )
                .exec()
                .await
        }
    };

    config.client_name = hostname;
    config.save();

    println!("Client: {:?}", &client);

    Ok(())
}
