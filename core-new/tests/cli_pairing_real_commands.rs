//! Real CLI command integration test for device pairing
//! This test spawns actual CLI processes using instances and tests the real user workflow

use std::process::Command;
use std::time::Duration;
use tokio::time::timeout;
use tempfile::TempDir;

struct CliInstance {
    name: String,
    data_dir: String,
    cli_binary: String,
}

impl CliInstance {
    fn new(name: &str, data_dir: &str) -> Self {
        Self {
            name: name.to_string(),
            data_dir: data_dir.to_string(),
            cli_binary: "./target/debug/spacedrive".to_string(),
        }
    }

    fn run_command(&self, args: &[&str]) -> Result<std::process::Output, std::io::Error> {
        let mut full_args = vec![
            "--data-dir", &self.data_dir,
            "--instance", &self.name,
        ];
        full_args.extend_from_slice(args);
        
        Command::new(&self.cli_binary)
            .args(&full_args)
            .output()
    }

    async fn run_command_with_timeout(&self, args: &[&str], timeout_secs: u64) -> Result<std::process::Output, String> {
        let timeout_duration = Duration::from_secs(timeout_secs);
        
        match timeout(timeout_duration, async {
            self.run_command(args)
        }).await {
            Ok(result) => result.map_err(|e| format!("Command failed: {}", e)),
            Err(_) => Err(format!("Command timed out after {} seconds", timeout_secs))
        }
    }

    fn start_daemon(&self) -> Result<(), String> {
        let output = self.run_command(&["start", "--enable-networking"])
            .map_err(|e| format!("Failed to start daemon: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Daemon start failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        println!("{} daemon started: {}", self.name, String::from_utf8_lossy(&output.stdout));
        Ok(())
    }

    fn stop_daemon(&self) {
        let _ = self.run_command(&["instance", "stop", &self.name]);
    }

    fn check_status(&self) -> Result<String, String> {
        let output = self.run_command(&["status"])
            .map_err(|e| format!("Failed to check status: {}", e))?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn init_networking(&self, password: &str) -> Result<(), String> {
        let output = self.run_command(&["network", "init", "--password", password])
            .map_err(|e| format!("Failed to init networking: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Networking init failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        println!("{} networking initialized: {}", self.name, String::from_utf8_lossy(&output.stdout));
        Ok(())
    }

    async fn generate_pairing_code(&self) -> Result<(String, tokio::process::Child), String> {
        use tokio::process::Command as TokioCommand;
        use tokio::io::{AsyncBufReadExt, BufReader};
        
        let mut full_args = vec![
            "--data-dir", &self.data_dir,
            "--instance", &self.name,
            "network", "pair", "generate", "--auto-accept"
        ];
        
        let mut cmd = TokioCommand::new(&self.cli_binary)
            .args(&full_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn pairing command: {}", e))?;
        
        let stdout = cmd.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        
        // Read lines until we get the pairing code
        let mut pairing_code = None;
        let timeout_duration = tokio::time::Duration::from_secs(15);
        
        match tokio::time::timeout(timeout_duration, async {
            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                println!("{} output: {}", self.name, line.trim());
                
                // Look for the pairing code line
                if let Some(code) = extract_pairing_code(&line) {
                    pairing_code = Some(code);
                    break;
                }
                line.clear();
            }
        }).await {
            Ok(_) => {},
            Err(_) => return Err("Timeout waiting for pairing code".to_string()),
        }
        
        match pairing_code {
            Some(code) => Ok((code, cmd)),
            None => Err("Failed to extract pairing code from output".to_string()),
        }
    }

    async fn join_pairing(&self, code: &str) -> Result<(), String> {
        let output = self.run_command_with_timeout(&["network", "pair", "join", "--code", code], 30).await?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        println!("{} join output:\n{}", self.name, stdout);
        if !stderr.is_empty() {
            println!("{} join stderr:\n{}", self.name, stderr);
        }
        
        if !output.status.success() {
            return Err(format!("Pairing join failed: {}", stderr));
        }
        
        // Check for success indicators in output
        if stdout.contains("✓") || stdout.contains("success") || stdout.contains("paired") {
            Ok(())
        } else {
            Err("No success indicator found in output".to_string())
        }
    }
}

#[tokio::test]
async fn test_cli_pairing_real_commands() {
    println!("🧪 Testing real CLI pairing commands with instances");

    // Create temporary directories for Alice and Bob
    let alice_dir = TempDir::new().expect("Failed to create Alice temp dir");
    let bob_dir = TempDir::new().expect("Failed to create Bob temp dir");
    
    let alice = CliInstance::new("alice", alice_dir.path().to_str().unwrap());
    let bob = CliInstance::new("bob", bob_dir.path().to_str().unwrap());
    
    println!("📁 Alice data dir: {:?}", alice_dir.path());
    println!("📁 Bob data dir: {:?}", bob_dir.path());

    // Build the CLI binary first
    println!("🔨 Building CLI binary...");
    let build_result = Command::new("cargo")
        .args(&["build", "--bin", "spacedrive"])
        .output()
        .expect("Failed to build CLI binary");
    
    if !build_result.status.success() {
        panic!("Failed to build CLI binary: {}", String::from_utf8_lossy(&build_result.stderr));
    }

    // Cleanup function to stop daemons
    let cleanup = || {
        alice.stop_daemon();
        bob.stop_daemon();
    };

    // Start both daemons
    println!("🟦 Starting Alice daemon...");
    if let Err(e) = alice.start_daemon() {
        cleanup();
        panic!("Failed to start Alice daemon: {}", e);
    }

    println!("🟨 Starting Bob daemon...");
    if let Err(e) = bob.start_daemon() {
        cleanup();
        panic!("Failed to start Bob daemon: {}", e);
    }

    // Give daemons time to start
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Check daemon status
    println!("🔍 Checking daemon status...");
    match alice.check_status() {
        Ok(status) => println!("Alice status: {}", status.lines().take(3).collect::<Vec<_>>().join(" ")),
        Err(e) => println!("Alice status error: {}", e),
    }
    
    match bob.check_status() {
        Ok(status) => println!("Bob status: {}", status.lines().take(3).collect::<Vec<_>>().join(" ")),
        Err(e) => println!("Bob status error: {}", e),
    }

    // Initialize networking for both
    println!("🔧 Initializing networking...");
    if let Err(e) = alice.init_networking("alice-password") {
        cleanup();
        panic!("Failed to initialize Alice networking: {}", e);
    }

    if let Err(e) = bob.init_networking("bob-password") {
        cleanup();
        panic!("Failed to initialize Bob networking: {}", e);
    }

    // Give networking time to initialize
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Alice generates pairing code (starts background process)
    println!("🔑 Alice generating pairing code...");
    let (pairing_code, mut alice_process) = match alice.generate_pairing_code().await {
        Ok((code, process)) => (code, process),
        Err(e) => {
            cleanup();
            panic!("Alice pairing code generation failed: {}", e);
        }
    };

    println!("🔗 Extracted pairing code: {}...", 
             pairing_code.split_whitespace().take(3).collect::<Vec<_>>().join(" "));

    // Bob joins using the pairing code
    println!("🤝 Bob joining with pairing code...");
    match bob.join_pairing(&pairing_code).await {
        Ok(_) => {
            println!("✅ CLI pairing test successful!");
        }
        Err(e) => {
            // Kill Alice's pairing process before cleanup
            let _ = alice_process.kill().await;
            cleanup();
            panic!("Bob pairing failed: {}", e);
        }
    }

    // Wait a moment for pairing to complete on Alice's side
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Kill Alice's pairing process
    let _ = alice_process.kill().await;

    // Cleanup
    cleanup();
    println!("🧹 Cleaned up daemon instances");
}

/// Extract the pairing code from CLI output
fn extract_pairing_code(output: &str) -> Option<String> {
    // Look for the pairing code in the CLI output
    // The CLI outputs it in a specific format after "Your Pairing Code:" 
    for line in output.lines() {
        let trimmed = line.trim();
        // Look for a line with 12 words (the pairing code format)
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.len() == 12 {
            // Verify these look like BIP39 words (basic check - all lowercase alphabetic)
            if words.iter().all(|w| w.chars().all(|c| c.is_ascii_lowercase())) {
                return Some(words.join(" "));
            }
        }
    }
    None
}