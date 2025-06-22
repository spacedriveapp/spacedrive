//! Simple test to verify LibP2P mDNS discovery works between separate processes
//! This isolates the networking layer from the full pairing protocol

use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_mdns_discovery_between_processes() {
    println!("🧪 Testing basic mDNS discovery between two LibP2P processes");

    // Start Alice (listener) process
    println!("🟦 Starting Alice (mDNS listener)...");
    let mut alice = Command::new("cargo")
        .args(&["run", "--bin", "mdns_test_helper", "--", "listen"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start Alice process");

    // Give Alice time to start listening
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Start Bob (discoverer) process  
    println!("🟨 Starting Bob (mDNS discoverer)...");
    let bob_result = timeout(
        Duration::from_secs(10),
        async {
            Command::new("cargo")
                .args(&["run", "--bin", "mdns_test_helper", "--", "discover"])
                .output()
                .expect("Failed to start Bob process")
        }
    ).await;

    // Kill Alice process
    let _ = alice.kill();
    let alice_output = alice.wait_with_output().expect("Failed to read Alice output");

    match bob_result {
        Ok(bob_output) => {
            let alice_stdout = String::from_utf8_lossy(&alice_output.stdout);
            let bob_stdout = String::from_utf8_lossy(&bob_output.stdout);
            
            println!("📤 Alice output:\n{}", alice_stdout);
            println!("📥 Bob output:\n{}", bob_stdout);

            // Check if discovery succeeded
            if bob_stdout.contains("PEER_DISCOVERED") {
                println!("✅ mDNS discovery successful!");
            } else {
                println!("❌ mDNS discovery failed");
                println!("Alice stderr: {}", String::from_utf8_lossy(&alice_output.stderr));
                println!("Bob stderr: {}", String::from_utf8_lossy(&bob_output.stderr));
                panic!("mDNS discovery between processes failed");
            }
        }
        Err(_) => {
            let alice_stdout = String::from_utf8_lossy(&alice_output.stdout);
            println!("📤 Alice output:\n{}", alice_stdout);
            println!("⏰ Bob discovery timed out after 10 seconds");
            panic!("mDNS discovery timed out");
        }
    }
}