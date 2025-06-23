//! Direct Core method pairing test using subprocesses
//! This test bypasses the CLI layer and calls Core methods directly to isolate networking issues

use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tempfile::TempDir;
use serde_json;

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
    let alice_handle = tokio::spawn(async move {
        run_alice_core(alice_data_dir).await
    });

    // Spawn Bob subprocess  
    let bob_data_dir = bob_dir.path().to_str().unwrap().to_string();
    let bob_handle = tokio::spawn(async move {
        run_bob_core(bob_data_dir).await
    });

    // Wait for both to complete with timeout
    let timeout_duration = Duration::from_secs(60);
    
    let alice_result = timeout(timeout_duration, alice_handle).await;
    let bob_result = timeout(timeout_duration, bob_handle).await;

    match (alice_result, bob_result) {
        (Ok(Ok(Ok(alice_output))), Ok(Ok(Ok(bob_output)))) => {
            println!("✅ Alice output: {}", alice_output);
            println!("✅ Bob output: {}", bob_output);
            
            // Parse outputs to verify pairing success
            if alice_output.contains("PAIRING_SUCCESS") && bob_output.contains("PAIRING_SUCCESS") {
                println!("🎉 Core pairing test successful!");
            } else {
                println!("❌ Pairing did not complete successfully");
                println!("Alice: {}", alice_output);
                println!("Bob: {}", bob_output);
                panic!("Pairing failed");
            }
        }
        (alice_result, bob_result) => {
            println!("❌ Test timed out or failed:");
            println!("Alice result: {:?}", alice_result);
            println!("Bob result: {:?}", bob_result);
            panic!("Subprocess test failed");
        }
    }
}

async fn run_alice_core(data_dir: String) -> Result<String, String> {
    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "core_test_alice", "--",
            "--data-dir", &data_dir
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to spawn Alice: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !output.status.success() {
        return Err(format!("Alice failed: {}\nStderr: {}", stdout, stderr));
    }
    
    Ok(format!("{}\n{}", stdout, stderr))
}

async fn run_bob_core(data_dir: String) -> Result<String, String> {
    // Wait a bit for Alice to start
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "core_test_bob", "--", 
            "--data-dir", &data_dir
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to spawn Bob: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !output.status.success() {
        return Err(format!("Bob failed: {}\nStderr: {}", stdout, stderr));
    }
    
    Ok(format!("{}\n{}", stdout, stderr))
}