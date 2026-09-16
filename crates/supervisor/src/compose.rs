use crate::{
	container::{container_env, ContainerRuntime},
	exec::{exec, ExecResult},
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

// Every compose invocation is addressed at the configured runtime, including
// the read-only ones: `ps` and `logs` against the wrong daemon answer about
// the wrong containers.
async fn compose(
	dir: &Path,
	runtime: &ContainerRuntime,
	args: &[&str],
	env: &BTreeMap<String, String>,
) -> Result<ExecResult, String> {
	let mut cmd: Vec<String> = vec!["docker".into(), "compose".into()];
	cmd.extend(args.iter().map(|arg| arg.to_string()));
	let mut full_env = container_env(runtime);
	full_env.extend(env.iter().map(|(k, v)| (k.clone(), v.clone())));
	exec(&cmd, dir, &full_env).await.map_err(|e| e.to_string())
}

fn must(result: ExecResult, action: &str) -> Result<ExecResult, String> {
	if result.code != 0 {
		return Err(format!(
			"docker compose {action} failed: {}",
			result.reason()
		));
	}
	Ok(result)
}

/// `up -d` is idempotent — already-running containers are left untouched, so
/// this doubles as the adoption path for a live stack. Any command that can
/// (re)create containers must carry the service's interpolation environment —
/// a bare `up` would substitute empty strings and recreate the stack broken.
pub async fn compose_up(
	dir: &Path,
	runtime: &ContainerRuntime,
	env: &BTreeMap<String, String>,
) -> Result<(), String> {
	must(compose(dir, runtime, &["up", "-d"], env).await?, "up")?;
	Ok(())
}

/// Stop, never `down`: containers stay defined and data dirs are never touched.
pub async fn compose_stop(dir: &Path, runtime: &ContainerRuntime) -> Result<(), String> {
	must(
		compose(dir, runtime, &["stop"], &BTreeMap::new()).await?,
		"stop",
	)?;
	Ok(())
}

pub async fn compose_restart(
	dir: &Path,
	runtime: &ContainerRuntime,
	env: &BTreeMap<String, String>,
) -> Result<(), String> {
	must(compose(dir, runtime, &["restart"], env).await?, "restart")?;
	Ok(())
}

pub async fn compose_logs(
	dir: &Path,
	runtime: &ContainerRuntime,
	lines: usize,
) -> Result<Vec<String>, String> {
	let tail = lines.to_string();
	let result = must(
		compose(
			dir,
			runtime,
			&["logs", "--tail", tail.as_str(), "--no-color"],
			&BTreeMap::new(),
		)
		.await?,
		"logs",
	)?;
	let mut out: Vec<String> = result.stdout.split('\n').map(String::from).collect();
	if out.last().is_some_and(|line| line.is_empty()) {
		out.pop();
	}
	Ok(out)
}

#[derive(Deserialize)]
struct ComposeContainer {
	#[serde(rename = "State")]
	state: Option<String>,
}

/// `ps --format json` emits one JSON object per line, one per container.
pub async fn compose_summary(dir: &Path, runtime: &ContainerRuntime) -> Result<String, String> {
	let result = must(
		compose(
			dir,
			runtime,
			&["ps", "--all", "--format", "json"],
			&BTreeMap::new(),
		)
		.await?,
		"ps",
	)?;
	let containers: Vec<ComposeContainer> = result
		.stdout
		.lines()
		.filter(|line| !line.trim().is_empty())
		.filter_map(|line| serde_json::from_str(line).ok())
		.collect();
	if containers.is_empty() {
		return Ok("no containers".to_string());
	}
	let running = containers
		.iter()
		.filter(|c| c.state.as_deref() == Some("running"))
		.count();
	Ok(format!("{running}/{} containers running", containers.len()))
}
