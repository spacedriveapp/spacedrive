use std::{collections::BTreeMap, io, path::Path};
use tokio::process::Command;

#[derive(Debug)]
pub struct ExecResult {
	pub code: i32,
	pub stdout: String,
	pub stderr: String,
}

impl ExecResult {
	/// The first meaningful line of output, preferring stderr — where CLI
	/// tools put their refusal reasons.
	pub fn reason(&self) -> String {
		let text = if self.stderr.trim().is_empty() {
			self.stdout.trim()
		} else {
			self.stderr.trim()
		};
		text.lines().next().unwrap_or_default().to_string()
	}
}

/// Run a command to completion, capturing output. The environment entries are
/// applied on top of the inherited environment.
pub async fn exec(
	cmd: &[String],
	cwd: &Path,
	env: &BTreeMap<String, String>,
) -> io::Result<ExecResult> {
	let (program, args) = cmd
		.split_first()
		.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty command"))?;
	let output = Command::new(program)
		.args(args)
		.current_dir(cwd)
		.envs(env)
		.kill_on_drop(true)
		.output()
		.await?;
	Ok(ExecResult {
		code: output.status.code().unwrap_or(-1),
		stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
		stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
	})
}

/// Whether a binary resolves on PATH.
pub fn which(binary: &str) -> bool {
	let Some(path) = std::env::var_os("PATH") else {
		return false;
	};
	std::env::split_paths(&path).any(|dir| {
		let candidate = dir.join(binary);
		candidate.is_file()
	})
}
