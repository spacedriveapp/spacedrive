//! Example GraphQL API server

use sd_core_new::{Core, infrastructure::api};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::Extension,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create core
    let core = Arc::new(Core::new().await?);
    
    // Create GraphQL schema
    let schema = api::create_schema(core.clone());
    
    // Build router
    let app = Router::new()
        .route("/", get(graphql_playground))
        .route("/graphql", post(graphql_handler))
        .layer(Extension(schema));
    
    println!("🚀 GraphQL API running at http://localhost:8080");
    println!("📊 GraphQL Playground at http://localhost:8080");
    
    // Run server
    axum::Server::bind(&"0.0.0.0:8080".parse()?)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

/// GraphQL Playground UI
async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

/// GraphQL request handler
async fn graphql_handler(
    schema: Extension<api::graphql::Schema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

// Example queries you can run in the playground:
const EXAMPLE_QUERIES: &str = r#"
# Create a new library
mutation CreateLibrary {
  createLibrary(input: {
    name: "My Photos"
    description: "Personal photo collection"
  }) {
    id
    name
    path
    createdAt
  }
}

# List all libraries
query GetLibraries {
  libraries {
    id
    name
    path
    totalFiles
    totalSize
  }
}

# Discover libraries
query DiscoverLibraries {
  discoverLibraries {
    id
    name
    path
  }
}

# Copy files (cross-device!)
mutation CopyFiles {
  copyFiles(input: {
    sources: [
      { path: "/Users/me/photo1.jpg" },
      { path: "/Users/me/photo2.jpg" }
    ]
    destination: { path: "/Users/me/backup" }
  }) {
    successful
    failed
  }
}

# Copy from MacBook to iPhone
mutation CrossDeviceCopy {
  copyFiles(input: {
    sources: [{
      deviceId: "aaaa-bbbb-cccc-dddd"
      path: "/Users/me/vacation.mp4"
    }]
    destination: {
      deviceId: "1111-2222-3333-4444"
      path: "/Documents/Videos"
    }
  }) {
    successful
    failed
  }
}
"#;