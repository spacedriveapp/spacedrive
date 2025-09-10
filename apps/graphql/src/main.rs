use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_graphql::{Context, EmptySubscription, Object, Schema};
use async_graphql_axum::{GraphQL, GraphQLRequest, GraphQLResponse};
use axum::{routing::get, Router};

use sd_core::client::CoreClient;

struct AppState {
	core: CoreClient,
}

struct QueryRoot;

#[Object]
impl QueryRoot {
	async fn libraries(
		&self,
		ctx: &Context<'_>,
	) -> async_graphql::Result<Vec<sd_core::ops::libraries::list::output::LibraryInfo>> {
		let state = ctx.data::<Arc<AppState>>()?;
		let q = sd_core::ops::libraries::list::query::ListLibrariesQuery::basic();
		let out: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = state
			.core
			.query(&q)
			.await
			.map_err(|e| async_graphql::Error::new(e.to_string()))?;
		Ok(out)
	}

	async fn core_status(
		&self,
		ctx: &Context<'_>,
	) -> async_graphql::Result<sd_core::ops::core::status::output::CoreStatus> {
		let state = ctx.data::<Arc<AppState>>()?;
		let q = sd_core::ops::core::status::query::CoreStatusQuery;
		let out: sd_core::ops::core::status::output::CoreStatus = state
			.core
			.query(&q)
			.await
			.map_err(|e| async_graphql::Error::new(e.to_string()))?;
		Ok(out)
	}
}

struct MutationRoot;

#[Object]
impl MutationRoot {
	async fn copy(
		&self,
		ctx: &Context<'_>,
		sources: Vec<String>,
		destination: String,
	) -> async_graphql::Result<bool> {
		let state = ctx.data::<Arc<AppState>>()?;
		let mut input = sd_core::ops::files::copy::input::FileCopyInput::default();
		input.sources = sources.into_iter().map(Into::into).collect();
		input.destination = destination.into();
		state
			.core
			.action(&input)
			.await
			.map_err(|e| async_graphql::Error::new(e.to_string()))?;
		Ok(true)
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	let data_dir = sd_core::config::default_data_dir()?;
	let socket = data_dir.join("daemon/daemon.sock");
	let state = Arc::new(AppState {
		core: CoreClient::new(socket),
	});

	let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription)
		.data(state.clone())
		.finish();

	let app = Router::new()
		.route("/graphql", get(graphiql).post(graphql_handler))
		.with_state(schema.clone());

	axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
		.serve(app.into_make_service())
		.await?;

	Ok(())
}

async fn graphql_handler(
	schema: axum::extract::State<Schema<QueryRoot, MutationRoot, EmptySubscription>>,
	req: GraphQLRequest,
) -> GraphQLResponse {
	schema.execute(req.into_inner()).await.into()
}

async fn graphiql() -> impl axum::response::IntoResponse {
	GraphQL::playground_source(
		async_graphql::http::GraphiQLSource::build()
			.endpoint("/graphql")
			.finish(),
	)
}
