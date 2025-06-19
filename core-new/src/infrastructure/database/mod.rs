//! Database infrastructure using SeaORM

use sea_orm::{
    ConnectOptions, Database as SeaDatabase, DatabaseConnection, DbErr,
};
use sea_orm_migration::MigratorTrait;
use std::path::Path;
use std::time::Duration;
use tracing::info;

pub mod entities;
pub mod migration;

/// Database wrapper for Spacedrive
pub struct Database {
    /// SeaORM database connection
    conn: DatabaseConnection,
}

impl Database {
    /// Create a new database at the specified path
    pub async fn create(path: &Path) -> Result<Self, DbErr> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DbErr::Custom(format!("Failed to create directory: {}", e)))?;
        }
        
        let db_url = format!("sqlite://{}?mode=rwc", path.display());
        
        let mut opt = ConnectOptions::new(db_url);
        opt.max_connections(10)
            .min_connections(5)
            .connect_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(8))
            .sqlx_logging(false); // We'll use tracing instead
        
        let conn = SeaDatabase::connect(opt).await?;
        
        info!("Created new database at {:?}", path);
        
        Ok(Self { conn })
    }
    
    /// Open an existing database
    pub async fn open(path: &Path) -> Result<Self, DbErr> {
        if !path.exists() {
            return Err(DbErr::Custom(format!(
                "Database does not exist: {}",
                path.display()
            )));
        }
        
        let db_url = format!("sqlite://{}", path.display());
        
        let mut opt = ConnectOptions::new(db_url);
        opt.max_connections(10)
            .min_connections(5)
            .connect_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(8))
            .sqlx_logging(false);
        
        let conn = SeaDatabase::connect(opt).await?;
        
        info!("Opened database at {:?}", path);
        
        Ok(Self { conn })
    }
    
    /// Run migrations
    pub async fn migrate(&self) -> Result<(), DbErr> {
        migration::Migrator::up(&self.conn, None).await?;
        info!("Database migrations completed successfully");
        Ok(())
    }
    
    /// Get the database connection
    pub fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }
}