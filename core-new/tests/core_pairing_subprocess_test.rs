//! Direct Core method pairing test using subprocesses
//! This test bypasses the CLI layer and calls Core methods directly to isolate networking issues

use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::{timeout, sleep, interval};
use tokio::io::{AsyncBufReadExt, BufReader};
use tempfile::TempDir;

#[tokio::test]
async fn test_core_pairing_subprocess() {
    println!("🧪 Testing Core pairing methods directly with subprocesses");

    // Create temporary directories for Alice and Bob
    let alice_dir = TempDir::new().expect("Failed to create Alice temp dir");
    let bob_dir = TempDir::new().expect("Failed to create Bob temp dir");
    
    println!("📁 Alice data dir: {:?}", alice_dir.path());
    println!("📁 Bob data dir: {:?}", bob_dir.path());

    // Spawn Alice subprocess
    let alice_data_dir = alice_dir.path().to_str().unwrap().to_string();
    let mut alice_child = spawn_alice_core(alice_data_dir).await
        .expect("Failed to spawn Alice process");

    // Wait a bit for Alice to start
    sleep(Duration::from_secs(2)).await;

    // Spawn Bob subprocess  
    let bob_data_dir = bob_dir.path().to_str().unwrap().to_string();
    let mut bob_child = spawn_bob_core(bob_data_dir).await
        .expect("Failed to spawn Bob process");

    // Monitor both processes for success messages
    let timeout_duration = Duration::from_secs(90);
    let monitoring_result = timeout(timeout_duration, monitor_pairing_success(&mut alice_child, &mut bob_child)).await;

    match monitoring_result {
        Ok((alice_success, bob_success, alice_output, bob_output)) => {
            println!("🔍 Verifying device states:");
            println!("  Alice sees Bob: {}", alice_success);
            println!("  Bob sees Alice: {}", bob_success);
            
            if alice_success && bob_success {
                println!("🎉 Core pairing test successful with mutual device recognition!");
            } else {
                println!("❌ Pairing test failed:");
                if !alice_success { println!("  - Alice did not successfully connect to Bob"); }
                if !bob_success { println!("  - Bob did not successfully connect to Alice"); }
                println!("\nAlice output:\n{}", alice_output);
                println!("\nBob output:\n{}", bob_output);
                panic!("Pairing test failed - devices did not properly recognize each other");
            }
        }
        Err(_) => {
            println!("❌ Test timed out after {} seconds", timeout_duration.as_secs());
            let _ = alice_child.kill().await;
            let _ = bob_child.kill().await;
            panic!("Subprocess test timed out");
        }
    }

    // Clean up processes
    let _ = alice_child.kill().await;
    let _ = bob_child.kill().await;
}

async fn spawn_alice_core(data_dir: String) -> Result<Child, String> {
    Command::new("cargo")
        .args(&[
            "run", "--bin", "core_test_alice", "--",
            "--data-dir", &data_dir
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn Alice: {}", e))
}

async fn spawn_bob_core(data_dir: String) -> Result<Child, String> {
    Command::new("cargo")
        .args(&[
            "run", "--bin", "core_test_bob", "--", 
            "--data-dir", &data_dir
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn Bob: {}", e))
}

async fn monitor_pairing_success(alice_child: &mut Child, bob_child: &mut Child) -> (bool, bool, String, String) {
    let mut alice_output = String::new();
    let mut bob_output = String::new();
    let mut alice_success = false;
    let mut bob_success = false;
    
    // Get stdout readers
    let alice_stdout = alice_child.stdout.take().expect("Failed to get Alice stdout");
    let bob_stdout = bob_child.stdout.take().expect("Failed to get Bob stdout");
    
    let mut alice_reader = BufReader::new(alice_stdout).lines();
    let mut bob_reader = BufReader::new(bob_stdout).lines();
    
    let mut check_interval = interval(Duration::from_millis(100));
    
    loop {
        tokio::select! {
            // Read from Alice
            line = alice_reader.next_line() => {
                if let Ok(Some(line)) = line {
                    println!("Alice: {}", line);
                    alice_output.push_str(&line);
                    alice_output.push('\n');
                    
                    if line.contains("PAIRING_SUCCESS: Alice connected to Bob successfully") {
                        alice_success = true;
                        println!("✅ Alice pairing success detected!");
                    }
                }
            }
            
            // Read from Bob  
            line = bob_reader.next_line() => {
                if let Ok(Some(line)) = line {
                    println!("Bob: {}", line);
                    bob_output.push_str(&line);
                    bob_output.push('\n');
                    
                    if line.contains("PAIRING_SUCCESS: Bob connected to Alice successfully") {
                        bob_success = true;
                        println!("✅ Bob pairing success detected!");
                    }
                }
            }
            
            // Check if both succeeded
            _ = check_interval.tick() => {
                if alice_success && bob_success {
                    println!("🎉 Both processes succeeded, terminating...");
                    let _ = alice_child.kill().await;
                    let _ = bob_child.kill().await;
                    break;
                }
            }
        }
    }
    
    (alice_success, bob_success, alice_output, bob_output)
}