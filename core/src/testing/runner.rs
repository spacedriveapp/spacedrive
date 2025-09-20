//! Cargo test subprocess runner implementation

use std::collections::HashMap;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::interval;

/// A single subprocess in a cargo test-based test
pub struct TestProcess {
	pub name: String,
	pub test_function_name: String,
	pub data_dir: TempDir,
	pub child: Option<Child>,
	pub output: String,
}

/// Cargo test-based multi-process test runner
pub struct CargoTestRunner {
	processes: Vec<TestProcess>,
	global_timeout: Duration,
	test_file_name: String,
}

impl CargoTestRunner {
	/// Create a new cargo test runner
	pub fn new() -> Self {
		Self {
			processes: Vec::new(),
			global_timeout: Duration::from_secs(60),
			test_file_name: "test_core_pairing".to_string(),
		}
	}

	/// Create a new cargo test runner for a specific test file
	pub fn for_test_file(test_file_name: impl Into<String>) -> Self {
		Self {
			processes: Vec::new(),
			global_timeout: Duration::from_secs(60),
			test_file_name: test_file_name.into(),
		}
	}

	/// Set global timeout for all operations
	pub fn with_timeout(mut self, timeout: Duration) -> Self {
		self.global_timeout = timeout;
		self
	}

	/// Add a subprocess with a test function name
	pub fn add_subprocess(
		mut self,
		name: impl Into<String>,
		test_function_name: impl Into<String>,
	) -> Self {
		let name = name.into();
		let test_function_name = test_function_name.into();
		let data_dir = TempDir::new().expect("Failed to create temp dir");

		let process = TestProcess {
			name,
			test_function_name,
			data_dir,
			child: None,
			output: String::new(),
		};

		self.processes.push(process);
		self
	}

	/// Run all subprocesses and wait until success condition is met
	pub async fn run_until_success<F>(&mut self, condition: F) -> Result<(), String>
	where
		F: Fn(&HashMap<String, String>) -> bool,
	{
		// Spawn all subprocesses
		self.spawn_all_processes().await?;

		// Wait for success condition
		self.wait_until_condition(condition).await?;

		// Cleanup
		self.kill_all().await;

		Ok(())
	}

	/// Spawn a single subprocess by name
	pub async fn spawn_single_process(&mut self, name: &str) -> Result<(), String> {
		let process = self
			.processes
			.iter_mut()
			.find(|p| p.name == name)
			.ok_or_else(|| format!("Process '{}' not found", name))?;

		let mut command = Command::new("cargo");
		command
			.args(&[
				"test",
				&process.test_function_name,
				"--test",
				&self.test_file_name,
				"--",
				"--nocapture",
				"--ignored", // Run ignored tests
			])
			.env("TEST_ROLE", &process.name)
			.env("TEST_DATA_DIR", process.data_dir.path().to_str().unwrap());

		let child = command
			.stdout(Stdio::inherit())
			.stderr(Stdio::inherit())
			.spawn()
			.map_err(|e| format!("Failed to spawn process '{}': {}", process.name, e))?;

		process.child = Some(child);
		println!(
			"Spawned cargo test process: {} (test: {})",
			process.name, process.test_function_name
		);

		Ok(())
	}

	/// Wait for success condition without spawning processes
	pub async fn wait_for_success<F>(&mut self, condition: F) -> Result<(), String>
	where
		F: Fn(&HashMap<String, String>) -> bool,
	{
		// Wait for success condition
		self.wait_until_condition(condition).await?;

		// Cleanup
		self.kill_all().await;

		Ok(())
	}

	/// Spawn all subprocesses using cargo test
	async fn spawn_all_processes(&mut self) -> Result<(), String> {
		for process in &mut self.processes {
			let mut command = Command::new("cargo");
			command
				.args(&[
					"test",
					&process.test_function_name,
					"--test",
					&self.test_file_name,
					"--",
					"--nocapture",
					"--ignored", // Run ignored tests
				])
				.env("TEST_ROLE", &process.name)
				.env("TEST_DATA_DIR", process.data_dir.path().to_str().unwrap());

			let child = command
				.stdout(Stdio::inherit())
				.stderr(Stdio::inherit())
				.spawn()
				.map_err(|e| format!("Failed to spawn process '{}': {}", process.name, e))?;

			process.child = Some(child);
			println!(
				"Spawned cargo test process: {} (test: {})",
				process.name, process.test_function_name
			);
		}

		Ok(())
	}

	/// Wait until the success condition is met
	async fn wait_until_condition<F>(&mut self, condition: F) -> Result<(), String>
	where
		F: Fn(&HashMap<String, String>) -> bool,
	{
		let mut check_interval = interval(Duration::from_millis(100));
		let start_time = Instant::now();

		loop {
			tokio::select! {
				_ = check_interval.tick() => {
					// Read output from all processes
					self.read_all_output().await;

					// Build output map for condition check
					let outputs: HashMap<String, String> = self.processes.iter()
						.map(|p| (p.name.clone(), p.output.clone()))
						.collect();

					// Check condition
					if condition(&outputs) {
						println!("Success condition met after {:?}", start_time.elapsed());
						return Ok(());
					}

					// Check for timeout
					if start_time.elapsed() > self.global_timeout {
						return Err("Timeout waiting for success condition".to_string());
					}

					// Check for failed processes
					self.check_process_health()?;
				}
			}
		}
	}

	/// Read output from all running processes
	async fn read_all_output(&mut self) {
		// Output is handled via stdio inheritance - just track what we see in output
		for process in &mut self.processes {
			if let Some(child) = &mut process.child {
				// Check if process has exited to capture final output
				if let Ok(Some(_)) = child.try_wait() {
					// Process has exited, mark its output as complete
				}
			}
		}
	}

	/// Check if any processes have failed
	fn check_process_health(&mut self) -> Result<(), String> {
		for process in &mut self.processes {
			if let Some(child) = &mut process.child {
				if let Ok(Some(exit_status)) = child.try_wait() {
					if !exit_status.success() {
						return Err(format!(
							"Process '{}' exited with failure: {:?}",
							process.name,
							exit_status.code()
						));
					}
				}
			}
		}
		Ok(())
	}

	/// Kill all processes
	pub async fn kill_all(&mut self) {
		for process in &mut self.processes {
			if let Some(mut child) = process.child.take() {
				let _ = child.kill().await;
				let _ = child.wait().await;
			}
		}
		println!("Killed all cargo test processes");
	}

	/// Get output from a specific process
	pub fn get_output(&self, name: &str) -> Option<&str> {
		self.processes
			.iter()
			.find(|p| p.name == name)
			.map(|p| p.output.as_str())
	}

	/// Get all outputs as a map
	pub fn get_all_outputs(&self) -> HashMap<String, String> {
		self.processes
			.iter()
			.map(|p| (p.name.clone(), p.output.clone()))
			.collect()
	}
}

impl Drop for CargoTestRunner {
	fn drop(&mut self) {
		// Best effort cleanup
		for process in &mut self.processes {
			if let Some(mut child) = process.child.take() {
				let _ = child.start_kill();
			}
		}
	}
}

impl Default for CargoTestRunner {
	fn default() -> Self {
		Self::new()
	}
}
