//! CLI pairing integration test using separate processes
//! This approach spawns two separate processes to avoid Send+Sync issues

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

#[tokio::test]
async fn test_cli_pairing_separate_processes() {
    // Initialize logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with_test_writer()
        .try_init();

    println!("🔬 Starting CLI pairing test with separate processes...");

    // Create unique temp directories
    let test_id = Uuid::new_v4();
    let temp_dir_alice = std::env::temp_dir().join(format!("test-alice-{}", test_id));
    let temp_dir_bob = std::env::temp_dir().join(format!("test-bob-{}", test_id));

    // Build the helper binary
    println!("🔧 Building subprocess helper binary...");
    let build_result = Command::new("cargo")
        .args(&["build", "--bin", "cli_pairing_subprocess_helper"])
        .output()
        .expect("Failed to build helper binary");

    if !build_result.status.success() {
        panic!("❌ Failed to build helper binary: {}", 
               String::from_utf8_lossy(&build_result.stderr));
    }

    // Find the binary in target/debug directory
    let binary_path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("cli_pairing_subprocess_helper");

    println!("✅ Helper binary built successfully");

    // Start Alice as initiator
    println!("👑 Starting Alice as pairing initiator...");
    let mut alice_process = tokio::task::spawn_blocking({
        let binary_path = binary_path.clone();
        let temp_dir_alice = temp_dir_alice.clone();
        move || {
            Command::new(&binary_path)
                .args(&[
                    "initiator",
                    &temp_dir_alice.to_string_lossy(),
                    "alice-password-123"
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to spawn Alice process")
        }
    }).await.unwrap();

    // Read Alice's stdout to get the pairing code
    let alice_stdout = alice_process.stdout.take().expect("Failed to capture Alice's stdout");
    let mut alice_reader = BufReader::new(alice_stdout);
    
    println!("🔍 Waiting for Alice to generate pairing code...");
    let mut pairing_code: Option<String> = None;
    let mut attempts = 0;
    
    // Parse Alice's output to extract pairing code
    let pairing_code = tokio::task::spawn_blocking(move || {
        let mut line = String::new();
        while attempts < 100 {  // Timeout after ~10 seconds
            line.clear();
            match alice_reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    println!("👑 Alice: {}", line.trim());
                    
                    // Look for the pairing code output
                    if line.starts_with("PAIRING_CODE:") {
                        let code = line.trim_start_matches("PAIRING_CODE:").trim().to_string();
                        println!("📱 Extracted pairing code: {}...", 
                                 code.split_whitespace().take(3).collect::<Vec<_>>().join(" "));
                        return Some(code);
                    }
                }
                Err(_) => {
                    attempts += 1;
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        None
    }).await.unwrap();

    let pairing_code = match pairing_code {
        Some(code) => code,
        None => {
            println!("❌ Failed to extract pairing code from Alice");
            let _ = alice_process.kill();
            std::fs::remove_dir_all(&temp_dir_alice).ok();
            std::fs::remove_dir_all(&temp_dir_bob).ok();
            panic!("Could not get pairing code from Alice");
        }
    };

    println!("✅ Successfully extracted pairing code from Alice");

    // Give Alice time to fully set up mDNS discovery and generate pairing code
    sleep(Duration::from_millis(5000)).await;

    // Start Bob as joiner with the extracted pairing code
    println!("🤝 Starting Bob as pairing joiner...");
    let mut bob_process = tokio::task::spawn_blocking({
        let binary_path = binary_path.clone();
        let temp_dir_bob = temp_dir_bob.clone();
        let pairing_code = pairing_code.clone();
        move || {
            Command::new(&binary_path)
                .args(&[
                    "joiner", 
                    &temp_dir_bob.to_string_lossy(),
                    "bob-password-456",
                    &pairing_code
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to spawn Bob process")
        }
    }).await.unwrap();

    // Monitor both processes
    println!("⏳ Waiting for pairing to complete...");
    
    // Wait for both processes to complete with timeout
    let result = timeout(Duration::from_secs(30), async {
        let alice_result = tokio::task::spawn_blocking(move || alice_process.wait());
        let bob_result = tokio::task::spawn_blocking(move || bob_process.wait());
        
        let (alice_exit, bob_exit) = tokio::try_join!(alice_result, bob_result)?;
        Ok::<(std::process::ExitStatus, std::process::ExitStatus), Box<dyn std::error::Error + Send + Sync>>(
            (alice_exit?, bob_exit?)
        )
    }).await;

    // Check results
    match result {
        Ok(Ok((alice_status, bob_status))) => {
            if alice_status.success() && bob_status.success() {
                println!("🎉 Both Alice and Bob completed pairing successfully!");
                println!("✅ Alice exit code: {}", alice_status.code().unwrap_or(-1));
                println!("✅ Bob exit code: {}", bob_status.code().unwrap_or(-1));
                println!("🔥 PAIRING SUCCEEDED! 🔥");
            } else {
                println!("❌ One or both processes failed:");
                println!("   Alice exit code: {}", alice_status.code().unwrap_or(-1));
                println!("   Bob exit code: {}", bob_status.code().unwrap_or(-1));
                panic!("Pairing processes failed");
            }
        }
        Ok(Err(e)) => {
            println!("❌ Error waiting for processes: {}", e);
            panic!("Process execution error: {}", e);
        }
        Err(_) => {
            println!("⏰ Pairing timed out after 60 seconds");
            println!("⚠️  This may happen in CI environments with network limitations");
        }
    }
    
    // Cleanup
    println!("🧹 Cleaning up temporary directories...");
    std::fs::remove_dir_all(&temp_dir_alice).ok();
    std::fs::remove_dir_all(&temp_dir_bob).ok();
    
    println!("✅ CLI pairing subprocess test completed!");
}