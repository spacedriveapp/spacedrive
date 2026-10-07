//! # MCP server over the operation registry
//!
//! `sd mcp` speaks the Model Context Protocol over stdio and exposes every
//! registered query, action and preflight as a tool. The tool table is read
//! from the same `inventory` registry the daemon dispatches on, and each
//! tool's input schema is walked from the op's input type, so an agent sees
//! exactly what the CLI's `op` command and the generated clients see. Calls are
//! forwarded to the running daemon over its socket; the server holds no state
//! of its own and adds no permission model beyond reaching the daemon.
//!
//! Library-scoped tools take an optional `library_id`. Without one the call
//! uses the CLI's selected library (`cli.json`), or the first library the
//! daemon lists. Actions that start a job answer with the job id in their
//! output; `jobs.wait` polls `jobs.info` until that job reaches a terminal
//! status so an agent can follow it without a subscription.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use anyhow::Result;
use rmcp::{
	handler::server::ServerHandler,
	model::{
		CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
		Implementation, JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
		ServerInfo, Tool, ToolAnnotations,
	},
	service::{RequestContext, RoleServer, ServiceExt},
};
use sd_client::CoreClient;
use sd_core::infra::{
	daemon::types::{DaemonRequest, DaemonResponse},
	wire::{
		json_schema::{is_object_schema, json_schema},
		type_extraction::{generate_spacedrive_api, OperationScope, QueryScope},
		Preflight,
	},
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::CliConfig;

/// The name of the synthetic tool that follows a job to completion.
const JOBS_WAIT: &str = "jobs.wait";

/// How an op is addressed on the daemon: the registry keys queries, actions
/// and preflights differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
	Query,
	Action,
	Validate,
	Preview,
}

/// One registered op as a tool: how to address it and what it takes.
#[derive(Debug, Clone)]
struct OpTool {
	wire_method: String,
	kind: Kind,
	library_scoped: bool,
	/// Whether the op's input is a JSON object, which decides what an omitted
	/// `input` argument stands for.
	object_input: bool,
	tool: Tool,
}

pub struct McpServer {
	client: CoreClient,
	data_dir: PathBuf,
	tools: BTreeMap<String, OpTool>,
	jobs_wait: Tool,
}

/// Serve MCP over stdin and stdout until the client closes the stream.
///
/// Nothing in this process may print to stdout except the protocol, so logs go
/// to stderr under `RUST_LOG` and nothing else is written.
pub async fn run(data_dir: PathBuf, socket_addr: String, device: Option<String>) -> Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
		)
		.with_writer(std::io::stderr)
		.init();

	let client = CoreClient::new(socket_addr).with_device(device);
	let server = McpServer::new(client, data_dir);
	let running = server.serve(rmcp::transport::stdio()).await?;
	running.waiting().await?;
	Ok(())
}

impl McpServer {
	pub fn new(client: CoreClient, data_dir: PathBuf) -> Self {
		let (operations, queries, preflights, types) = generate_spacedrive_api();
		let mut tools = BTreeMap::new();

		for query in &queries {
			let library_scoped = query.scope == QueryScope::Library;
			let input = json_schema(&query.input_type, &types);
			tools.insert(
				query.identifier.to_string(),
				OpTool {
					wire_method: query.wire_method.clone(),
					kind: Kind::Query,
					library_scoped,
					object_input: is_object_schema(&input),
					tool: build_tool(
						query.identifier,
						Kind::Query,
						library_scoped,
						&query.input_type_name,
						input,
					),
				},
			);
		}
		for op in &operations {
			let library_scoped = op.scope == OperationScope::Library;
			let input = json_schema(&op.input_type, &types);
			tools.insert(
				op.identifier.to_string(),
				OpTool {
					wire_method: op.wire_method.clone(),
					kind: Kind::Action,
					library_scoped,
					object_input: is_object_schema(&input),
					tool: build_tool(
						op.identifier,
						Kind::Action,
						library_scoped,
						&op.input_type_name,
						input,
					),
				},
			);
		}
		for check in &preflights {
			let kind = match check.preflight {
				Preflight::Validate => Kind::Validate,
				Preflight::Preview => Kind::Preview,
			};
			let name = format!("{}.{}", check.identifier, check.preflight.prefix());
			let input = json_schema(&check.input_type, &types);
			tools.insert(
				name.clone(),
				OpTool {
					wire_method: check.wire_method.clone(),
					kind,
					library_scoped: true,
					object_input: is_object_schema(&input),
					tool: build_tool(&name, kind, true, &check.input_type_name, input),
				},
			);
		}

		let jobs_wait = Tool::new(
			JOBS_WAIT,
			"Wait for a job to reach a terminal status (completed, failed or cancelled) and \
			 return its final jobs.info record. Actions that start work answer with the job id \
			 to pass here.",
			rmcp::model::object(json!({
				"type": "object",
				"properties": {
					"job_id": { "type": "string", "format": "uuid" },
					"timeout_seconds": {
						"type": "integer",
						"minimum": 1,
						"description": "Give up after this long; the job keeps running. Default 300."
					},
					"library_id": library_id_schema(),
				},
				"required": ["job_id"],
			})),
		)
		.with_annotations(
			ToolAnnotations::new()
				.read_only(true)
				.destructive(false)
				.idempotent(true)
				.open_world(false),
		);

		Self {
			client,
			data_dir,
			tools,
			jobs_wait,
		}
	}

	/// The library a call runs against: the argument, else the CLI's
	/// selection when the daemon still has it, else the first library the
	/// daemon lists. Read per call so a library created or deleted through
	/// this server is seen without a restart; a stale selection is not
	/// trusted, as `sd` repairs one on every command.
	async fn resolve_library(&self, explicit: Option<Uuid>) -> Result<Option<Uuid>, String> {
		if explicit.is_some() {
			return Ok(explicit);
		}
		let libraries = self
			.send(
				Kind::Query,
				"query:libraries.list".to_string(),
				None,
				json!({ "include_stats": false }),
			)
			.await?;
		let ids: Vec<Uuid> = libraries
			.as_array()
			.map(|libs| {
				libs.iter()
					.filter_map(|lib| lib.get("id").and_then(Value::as_str))
					.filter_map(|id| Uuid::parse_str(id).ok())
					.collect()
			})
			.unwrap_or_default();
		let selected = CliConfig::load(&self.data_dir)
			.ok()
			.and_then(|c| c.current_library_id)
			.filter(|id| ids.contains(id));
		Ok(selected.or_else(|| ids.first().copied()))
	}

	/// One request to the daemon, answered as JSON or as the daemon's own
	/// error message.
	async fn send(
		&self,
		kind: Kind,
		method: String,
		library_id: Option<Uuid>,
		payload: Value,
	) -> Result<Value, String> {
		let device = self.client.device().map(String::from);
		let request = match kind {
			Kind::Action => DaemonRequest::Action {
				method,
				library_id,
				payload,
				device,
			},
			Kind::Query | Kind::Validate | Kind::Preview => DaemonRequest::Query {
				method,
				library_id,
				payload,
				device,
			},
		};
		match self.client.send_raw_request(&request).await {
			Ok(DaemonResponse::JsonOk(value)) => Ok(value),
			Ok(DaemonResponse::Error(err)) => Err(err.to_string()),
			Ok(other) => Err(format!("unexpected daemon response: {other:?}")),
			Err(err) => Err(format!(
				"Spacedrive daemon is not reachable ({err}); start it with `sd start`"
			)),
		}
	}

	async fn call_op(&self, op: &OpTool, arguments: &JsonObject) -> Result<Value, String> {
		let library_id = if op.library_scoped {
			self.resolve_library(parse_library_id(arguments)?).await?
		} else {
			None
		};
		let payload = match arguments.get("input") {
			Some(input) => input.clone(),
			None if op.object_input => json!({}),
			None => Value::Null,
		};
		self.send(op.kind, op.wire_method.clone(), library_id, payload)
			.await
	}

	async fn wait_for_job(&self, arguments: &JsonObject) -> Result<Value, String> {
		let job_id = arguments
			.get("job_id")
			.and_then(Value::as_str)
			.ok_or("job_id is required")?;
		let job_id = Uuid::parse_str(job_id).map_err(|e| format!("job_id: {e}"))?;
		let timeout = arguments
			.get("timeout_seconds")
			.and_then(Value::as_u64)
			.unwrap_or(300);
		let library_id = self.resolve_library(parse_library_id(arguments)?).await?;
		let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout);

		loop {
			let info = self
				.send(
					Kind::Query,
					"query:jobs.info".to_string(),
					library_id,
					json!({ "job_id": job_id }),
				)
				.await?;
			if info.is_null() {
				return Err(format!("job {job_id} not found"));
			}
			let status = info.get("status").and_then(Value::as_str).unwrap_or("");
			if matches!(status, "completed" | "failed" | "cancelled") {
				return Ok(info);
			}
			if tokio::time::Instant::now() >= deadline {
				return Err(format!(
					"job {job_id} still {status} after {timeout}s; call jobs.wait again to keep following it"
				));
			}
			tokio::time::sleep(Duration::from_millis(500)).await;
		}
	}
}

impl ServerHandler for McpServer {
	fn get_info(&self) -> ServerInfo {
		ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
			.with_server_info(
				Implementation::new("spacedrive", env!("CARGO_PKG_VERSION"))
					.with_title("Spacedrive")
					.with_website_url("https://spacedrive.com"),
			)
			.with_instructions(
				"Every tool is one registered Spacedrive operation, forwarded to the local \
				 daemon. Tools named after queries read; tools named after actions change \
				 state and answer at once, usually with a job id, which jobs.wait follows to \
				 completion. A `.validate` or `.preview` tool answers what the matching \
				 action would do without doing it. Library-scoped tools take an optional \
				 library_id and default to the CLI's selected library.",
			)
	}

	async fn list_tools(
		&self,
		_request: Option<PaginatedRequestParams>,
		_context: RequestContext<RoleServer>,
	) -> Result<ListToolsResult, ErrorData> {
		let mut tools: Vec<Tool> = self.tools.values().map(|op| op.tool.clone()).collect();
		tools.push(self.jobs_wait.clone());
		Ok(ListToolsResult::with_all_items(tools))
	}

	async fn call_tool(
		&self,
		request: CallToolRequestParams,
		_context: RequestContext<RoleServer>,
	) -> Result<CallToolResponse, ErrorData> {
		let arguments = request.arguments.unwrap_or_default();
		let outcome = if request.name == JOBS_WAIT {
			self.wait_for_job(&arguments).await
		} else {
			let op = self.tools.get(request.name.as_ref()).ok_or_else(|| {
				ErrorData::invalid_params(format!("unknown tool {}", request.name), None)
			})?;
			self.call_op(op, &arguments).await
		};
		Ok(CallToolResponse::Complete(match outcome {
			Ok(value) => tool_result(value),
			Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
		}))
	}
}

/// A tool's input: the op's own input under `input`, beside `library_id` for
/// library-scoped ops. The op's schema is nested rather than flattened so an
/// input that is not an object (a unit query, a tuple) still has a home and
/// no op field can collide with `library_id`.
fn build_tool(
	name: &str,
	kind: Kind,
	library_scoped: bool,
	input_type_name: &str,
	input: Value,
) -> Tool {
	let (verb, annotations) = match kind {
		Kind::Query => (
			"Query",
			ToolAnnotations::new()
				.read_only(true)
				.destructive(false)
				.idempotent(true)
				.open_world(false),
		),
		Kind::Validate => (
			"Validate (what the action would refuse or warn about, without running it)",
			ToolAnnotations::new()
				.read_only(true)
				.destructive(false)
				.idempotent(true)
				.open_world(false),
		),
		Kind::Preview => (
			"Preview (the plan the action would carry out, without running it)",
			ToolAnnotations::new()
				.read_only(true)
				.destructive(false)
				.idempotent(true)
				.open_world(false),
		),
		Kind::Action => (
			"Action",
			ToolAnnotations::new().read_only(false).open_world(false),
		),
	};
	let scope = if library_scoped { "library" } else { "core" };
	let description = format!("{verb}: {name} ({scope} scope). Input type: {input_type_name}.");

	let input_required = !matches!(input.get("type"), Some(Value::String(t)) if t == "null");
	let mut properties = serde_json::Map::new();
	properties.insert("input".to_string(), input);
	if library_scoped {
		properties.insert("library_id".to_string(), library_id_schema());
	}
	let mut schema = json!({ "type": "object", "properties": properties });
	if input_required {
		schema["required"] = json!(["input"]);
	}

	Tool::new(name.to_string(), description, rmcp::model::object(schema))
		.with_annotations(annotations)
}

fn library_id_schema() -> Value {
	json!({
		"type": "string",
		"format": "uuid",
		"description": "Library to run against. Defaults to the CLI's selected library, then the first library the daemon lists."
	})
}

fn parse_library_id(arguments: &JsonObject) -> Result<Option<Uuid>, String> {
	match arguments.get("library_id") {
		None | Some(Value::Null) => Ok(None),
		Some(Value::String(id)) => Uuid::parse_str(id)
			.map(Some)
			.map_err(|e| format!("library_id: {e}")),
		Some(other) => Err(format!("library_id must be a uuid string, got {other}")),
	}
}

/// The op's output as text for every client, and as structured content where
/// the protocol allows it (an object).
fn tool_result(value: Value) -> CallToolResult {
	let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
	let mut result = CallToolResult::success(vec![ContentBlock::text(text)]);
	if value.is_object() {
		result.structured_content = Some(value);
	}
	result
}
