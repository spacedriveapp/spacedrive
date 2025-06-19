//! GraphQL API implementation

use async_graphql::{Context, EmptySubscription, Object, Result, Schema as GraphQLSchema, SimpleObject, InputObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::Core;
use std::sync::Arc;

/// Library type for GraphQL
#[derive(SimpleObject)]
pub struct LibraryType {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub total_files: i64,
    pub total_size: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a library
#[derive(InputObject)]
pub struct CreateLibraryInput {
    pub name: String,
    pub description: Option<String>,
    pub location: Option<String>,
}

/// Input for updating library settings  
#[derive(InputObject)]
pub struct UpdateLibrarySettingsInput {
    pub id: Uuid,
    pub generate_thumbnails: Option<bool>,
    pub thumbnail_quality: Option<u8>,
    pub enable_ai_tagging: Option<bool>,
}

/// Root query object
pub struct Query;

#[Object]
impl Query {
    /// Get all libraries
    async fn libraries(&self, ctx: &Context<'_>) -> Result<Vec<LibraryType>> {
        let core = ctx.data::<Arc<Core>>()?;
        let open_libraries = core.libraries.get_open_libraries().await;
        
        let mut libraries = Vec::new();
        for lib in open_libraries {
            let config = lib.config().await;
            libraries.push(LibraryType {
                id: config.id,
                name: config.name,
                path: lib.path().display().to_string(),
                description: config.description,
                total_files: config.statistics.total_files as i64,
                total_size: config.statistics.total_size as i64,
                created_at: config.created_at,
                updated_at: config.updated_at,
            });
        }
        
        Ok(libraries)
    }
    
    /// Get a specific library
    async fn library(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<LibraryType>> {
        let core = ctx.data::<Arc<Core>>()?;
        
        if let Some(lib) = core.libraries.get_library(id).await {
            let config = lib.config().await;
            Ok(Some(LibraryType {
                id: config.id,
                name: config.name,
                path: lib.path().display().to_string(),
                description: config.description,
                total_files: config.statistics.total_files as i64,
                total_size: config.statistics.total_size as i64,
                created_at: config.created_at,
                updated_at: config.updated_at,
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Discover libraries in search paths
    async fn discover_libraries(&self, ctx: &Context<'_>) -> Result<Vec<LibraryType>> {
        let core = ctx.data::<Arc<Core>>()?;
        let discovered = core.libraries.scan_for_libraries().await?;
        
        Ok(discovered.into_iter().map(|d| LibraryType {
            id: d.config.id,
            name: d.config.name,
            path: d.path.display().to_string(),
            description: d.config.description,
            total_files: d.config.statistics.total_files as i64,
            total_size: d.config.statistics.total_size as i64,
            created_at: d.config.created_at,
            updated_at: d.config.updated_at,
        }).collect())
    }
}

/// Root mutation object
pub struct Mutation;

#[Object]
impl Mutation {
    /// Create a new library
    async fn create_library(
        &self,
        ctx: &Context<'_>,
        input: CreateLibraryInput,
    ) -> Result<LibraryType> {
        let core = ctx.data::<Arc<Core>>()?;
        
        let location = input.location.map(|s| s.into());
        let library = core.libraries.create_library(input.name, location).await?;
        
        if let Some(desc) = input.description {
            library.update_config(|config| {
                config.description = Some(desc);
            }).await?;
        }
        
        let config = library.config().await;
        
        Ok(LibraryType {
            id: config.id,
            name: config.name,
            path: library.path().display().to_string(),
            description: config.description,
            total_files: 0,
            total_size: 0,
            created_at: config.created_at,
            updated_at: config.updated_at,
        })
    }
    
    /// Open a library
    async fn open_library(&self, ctx: &Context<'_>, path: String) -> Result<LibraryType> {
        let core = ctx.data::<Arc<Core>>()?;
        let library = core.libraries.open_library(path).await?;
        let config = library.config().await;
        
        Ok(LibraryType {
            id: config.id,
            name: config.name,
            path: library.path().display().to_string(),
            description: config.description,
            total_files: config.statistics.total_files as i64,
            total_size: config.statistics.total_size as i64,
            created_at: config.created_at,
            updated_at: config.updated_at,
        })
    }
    
    /// Close a library
    async fn close_library(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        let core = ctx.data::<Arc<Core>>()?;
        core.libraries.close_library(id).await?;
        Ok(true)
    }
    
    /// Update library settings
    async fn update_library_settings(
        &self,
        ctx: &Context<'_>,
        input: UpdateLibrarySettingsInput,
    ) -> Result<LibraryType> {
        let core = ctx.data::<Arc<Core>>()?;
        
        let library = core.libraries.get_library(input.id).await
            .ok_or_else(|| async_graphql::Error::new("Library not found"))?;
        
        library.update_config(|config| {
            if let Some(gen_thumbs) = input.generate_thumbnails {
                config.settings.generate_thumbnails = gen_thumbs;
            }
            if let Some(quality) = input.thumbnail_quality {
                config.settings.thumbnail_quality = quality;
            }
            if let Some(ai_tag) = input.enable_ai_tagging {
                config.settings.enable_ai_tagging = ai_tag;
            }
        }).await?;
        
        let config = library.config().await;
        
        Ok(LibraryType {
            id: config.id,
            name: config.name,
            path: library.path().display().to_string(),
            description: config.description,
            total_files: config.statistics.total_files as i64,
            total_size: config.statistics.total_size as i64,
            created_at: config.created_at,
            updated_at: config.updated_at,
        })
    }
}

use super::file_ops::FileOpsMutation;

/// Combined mutation root that includes all mutations
#[derive(async_graphql::MergedObject, Default)]
pub struct MutationRoot(Mutation, FileOpsMutation);

/// GraphQL schema type
pub type Schema = GraphQLSchema<Query, MutationRoot, EmptySubscription>;

/// Create the GraphQL schema
pub fn create_schema(core: Arc<Core>) -> Schema {
    GraphQLSchema::build(Query, MutationRoot::default(), EmptySubscription)
        .data(core)
        .finish()
}