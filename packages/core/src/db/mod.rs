use crate::state;
use crate::{prisma, prisma::PrismaClient};
use anyhow::Result;
use once_cell::sync::OnceCell;
pub mod migrate;

pub static DB: OnceCell<PrismaClient> = OnceCell::new();

pub async fn get() -> Result<&'static PrismaClient, String> {
    if DB.get().is_none() {
        let config = state::client::get();

        let current_library = config.libraries.iter().find(|l| l.library_id == config.current_library_id).unwrap();

        let path = current_library.library_path.clone();
        // TODO: Error handling when brendan adds it to prisma-client-rust

        let client = prisma::new_client_with_url(&format!("file:{}", &path)).await;
        DB.set(client).unwrap_or_default();

        Ok(DB.get().unwrap())
    } else {
        Ok(DB.get().unwrap())
    }
}
