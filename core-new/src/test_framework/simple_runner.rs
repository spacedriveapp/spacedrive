//! Simple subprocess test runner - flexible abstraction for multi-core tests

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{interval, timeout};

/// A single subprocess in a multi-process test
pub struct TestProcess {
    pub name: String,
    pub data_dir: TempDir,
    pub child: Option<Child>,
    pub output: String,
}

/// Simple multi-process test runner
pub struct SimpleTestRunner {
    processes: Vec<TestProcess>,
    global_timeout: Duration,
}

impl SimpleTestRunner {
    /// Create a new test runner
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            global_timeout: Duration::from_secs(60),
        }
    }
    
    /// Set global timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.global_timeout = timeout;
        self
    }
    
    /// Add a process to spawn (but don't spawn it yet)
    pub fn add_process(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        let data_dir = TempDir::new().expect("Failed to create temp dir");
        
        let process = TestProcess {
            name,
            data_dir,
            child: None,
            output: String::new(),
        };
        
        self.processes.push(process);
        self
    }
    
    /// Get data directory for a process by name
    pub fn get_data_dir(&self, name: &str) -> Option<&std::path::Path> {
        self.processes.iter()
            .find(|p| p.name == name)
            .map(|p| p.data_dir.path())
    }
    
    /// Spawn a process with custom command
    pub async fn spawn_process<F>(&mut self, name: &str, command_builder: F) -> Result<(), String>
    where
        F: FnOnce(&std::path::Path) -> Command,
    {
        let process = self.processes.iter_mut()
            .find(|p| p.name == name)
            .ok_or_else(|| format!("Process '{}' not found", name))?;
        
        let data_dir = process.data_dir.path();
        let mut command = command_builder(data_dir);
        
        let child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", name, e))?;
        
        process.child = Some(child);
        println!("🚀 Spawned process: {}", name);
        Ok(())
    }
    
    /// Wait until a condition is met or timeout
    pub async fn wait_until<F>(&mut self, condition: F) -> Result<(), String>
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
                        println!("✅ Condition met after {:?}", start_time.elapsed());
                        return Ok(());
                    }
                    
                    // Check for timeout
                    if start_time.elapsed() > self.global_timeout {
                        return Err("Timeout waiting for condition".to_string());
                    }
                    
                    // Check for failed processes
                    self.check_process_health()?;
                }
            }
        }
    }
    
    /// Read output from all running processes
    async fn read_all_output(&mut self) {
        for process in &mut self.processes {
            if let Some(child) = &mut process.child {
                // Read stdout
                if let Some(stdout) = child.stdout.take() {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        println!("{}: {}", process.name, line);
                        process.output.push_str(&line);
                        process.output.push('\n');
                    }
                    // Note: We lose the stdout handle here, which is fine for simple cases
                    // For continuous monitoring, we'd need a different approach
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
        println!("🧹 Killed all processes");
    }
    
    /// Get output from a specific process
    pub fn get_output(&self, name: &str) -> Option<&str> {
        self.processes.iter()
            .find(|p| p.name == name)
            .map(|p| p.output.as_str())
    }
    
    /// Get all outputs as a map
    pub fn get_all_outputs(&self) -> HashMap<String, String> {
        self.processes.iter()
            .map(|p| (p.name.clone(), p.output.clone()))
            .collect()
    }
}

impl Drop for SimpleTestRunner {
    fn drop(&mut self) {
        // Best effort cleanup
        for process in &mut self.processes {
            if let Some(mut child) = process.child.take() {
                let _ = child.start_kill();
            }
        }
    }
}

impl Default for SimpleTestRunner {
    fn default() -> Self {
        Self::new()
    }
}