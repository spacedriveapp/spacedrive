//! `sd mcp` as an MCP client sees it.
//!
//! Starts a core with its RPC server in this process, spawns the built
//! `sd-cli mcp` over stdio, completes the MCP initialize handshake, lists the
//! tools, calls a core query and a library query, runs an action and follows
//! its job to completion through `jobs.wait`.

use rmcp::{
	model::{CallToolRequestParams, CallToolResult, Tool},
	service::ServiceExt,
	transport::TokioChildProcess,
};
use sd_core::{infra::daemon::addr::daemon_socket_addr, Core};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::process::Command;

/// A named instance whose derived port is free, so the spawned CLI finds the
/// in-process RPC server where `--instance` tells it to look.
async fn free_instance() -> (String, String) {
	for attempt in 0..50u32 {
		let name = format!("mcp-test-{attempt}");
		let addr = daemon_socket_addr(Some(&name));
		if tokio::net::TcpListener::bind(addr).await.is_ok() {
			return (name, addr.to_string());
		}
	}
	panic!("no free instance port");
}

fn text_of(result: &CallToolResult) -> String {
	result
		.content
		.iter()
		.filter_map(|block| block.as_text().map(|t| t.text.clone()))
		.collect::<Vec<_>>()
		.join("\n")
}

fn json_of(result: &CallToolResult) -> Value {
	let text = text_of(result);
	serde_json::from_str(&text).unwrap_or_else(|e| panic!("tool answered non-JSON {text:?}: {e}"))
}

fn tool<'a>(tools: &'a [Tool], name: &str) -> &'a Tool {
	tools
		.iter()
		.find(|t| t.name == name)
		.unwrap_or_else(|| panic!("tool {name} is not listed"))
}

#[tokio::test(flavor = "multi_thread")]
async fn sd_mcp_exposes_the_registry_over_stdio(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let temp = TempDir::new()?;
	let core = Arc::new(Core::new(temp.path().join("core")).await?);
	let library = core
		.libraries
		.create_library("MCP Library".to_string(), None, core.context.clone())
		.await?;

	let (instance, socket_addr) = free_instance().await;
	let server_core = core.clone();
	let server_addr = socket_addr.clone();
	tokio::spawn(async move {
		let mut server = sd_core::infra::daemon::rpc::RpcServer::new(server_addr, server_core);
		if let Err(e) = server.start().await {
			eprintln!("rpc server stopped: {e}");
		}
	});
	tokio::time::sleep(Duration::from_millis(500)).await;

	let cli_data = temp.path().join("cli");
	let mut command = Command::new(env!("CARGO_BIN_EXE_sd-cli"));
	command
		.arg("--data-dir")
		.arg(&cli_data)
		.arg("--instance")
		.arg(&instance)
		.arg("mcp");
	let client = ().serve(TokioChildProcess::new(command)?).await?;

	let info = client.peer_info().expect("initialize answered");
	assert_eq!(
		info.server_info.as_ref().map(|s| s.name.as_str()),
		Some("spacedrive")
	);
	assert!(
		info.capabilities.tools.is_some(),
		"tools capability advertised"
	);

	let tools = client.list_all_tools().await?;
	assert!(tools.len() > 50, "only {} tools listed", tools.len());
	let libraries_list = tool(&tools, "libraries.list");
	assert_eq!(
		libraries_list
			.annotations
			.as_ref()
			.and_then(|a| a.read_only_hint),
		Some(true)
	);
	assert!(
		libraries_list.input_schema["properties"]["input"]["properties"]["include_stats"]
			.is_object(),
		"input schema carries the op's fields: {}",
		serde_json::to_string_pretty(&libraries_list.input_schema)?
	);
	assert!(
		libraries_list.input_schema["properties"]
			.get("library_id")
			.is_none(),
		"a core op takes no library id"
	);
	let track = tool(&tools, "sources.track");
	assert_eq!(
		track.annotations.as_ref().and_then(|a| a.read_only_hint),
		Some(false)
	);
	assert!(track.input_schema["properties"]["library_id"].is_object());
	assert_eq!(
		track.input_schema["properties"]["input"]["properties"]["path"]["type"],
		"string"
	);
	tool(&tools, "jobs.info");
	tool(&tools, "jobs.wait");
	tool(&tools, "files.copy.validate");

	// A core query.
	let listed = client
		.call_tool(CallToolRequestParams::new("libraries.list").with_arguments(
			rmcp::model::object(json!({ "input": { "include_stats": false } })),
		))
		.await?;
	assert_eq!(listed.is_error, Some(false), "{}", text_of(&listed));
	let libraries = json_of(&listed);
	assert_eq!(libraries[0]["id"], json!(library.id()));
	assert_eq!(libraries[0]["name"], "MCP Library");

	// A library query with no library id: the only library is the default.
	let jobs = client
		.call_tool(CallToolRequestParams::new("jobs.list"))
		.await?;
	assert_eq!(jobs.is_error, Some(false), "{}", text_of(&jobs));
	assert_eq!(json_of(&jobs)["jobs"], json!([]));
	assert!(
		jobs.structured_content.is_some(),
		"an object output is also structured content"
	);

	// An action that starts a job, followed to completion.
	let source_dir = temp.path().join("source");
	tokio::fs::create_dir_all(&source_dir).await?;
	tokio::fs::write(source_dir.join("a.txt"), b"alpha").await?;
	tokio::fs::write(source_dir.join("b.txt"), b"beta").await?;
	let tracked =
		client
			.call_tool(CallToolRequestParams::new("sources.track").with_arguments(
				rmcp::model::object(json!({
					"library_id": library.id(),
					"input": { "path": source_dir },
				})),
			))
			.await?;
	assert_eq!(tracked.is_error, Some(false), "{}", text_of(&tracked));
	let job_id = json_of(&tracked)["job_id"]
		.as_str()
		.expect("tracking a source starts a walk")
		.to_string();

	let waited = client
		.call_tool(
			CallToolRequestParams::new("jobs.wait").with_arguments(rmcp::model::object(json!({
				"job_id": job_id,
				"timeout_seconds": 60,
			}))),
		)
		.await?;
	assert_eq!(waited.is_error, Some(false), "{}", text_of(&waited));
	let job = json_of(&waited);
	assert_eq!(job["id"], job_id);
	assert_eq!(job["status"], "completed", "{job}");

	// A daemon error reaches the client as the tool's error text.
	let missing = client
		.call_tool(
			CallToolRequestParams::new("jobs.info").with_arguments(rmcp::model::object(json!({
				"library_id": uuid::Uuid::new_v4(),
				"input": { "job_id": job_id },
			}))),
		)
		.await?;
	assert_eq!(missing.is_error, Some(true));
	assert!(
		!text_of(&missing).is_empty(),
		"the daemon's message is the error"
	);

	// A name the registry does not hold is a protocol error, not a tool error.
	assert!(client
		.call_tool(CallToolRequestParams::new("files.no_such_op"))
		.await
		.is_err());

	client.cancel().await?;
	core.shutdown().await?;
	Ok(())
}
