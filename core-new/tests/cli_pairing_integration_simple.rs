//! Simple CLI pairing integration test focusing on core functionality
//! 
//! This test verifies that our enhanced subprocess approach works correctly
//! by testing individual components of the pairing workflow.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct ProcessResult {
    current_status: String,
    pairing_code: Option<String>,
    expires_in: Option<u64>,
    errors: Vec<String>,
    device_state_verified: bool,
}

impl ProcessResult {
    fn new() -> Self {
        Self {
            current_status: "unknown".to_string(),
            pairing_code: None,
            expires_in: None,
            errors: Vec::new(),
            device_state_verified: false,
        }
    }
    
    fn is_successful(&self) -> bool {
        self.current_status == "SUCCESS" && self.errors.is_empty()
    }
    
    fn has_valid_pairing_code(&self) -> bool {
        if let Some(ref code) = self.pairing_code {
            let words: Vec<&str> = code.split_whitespace().collect();
            words.len() == 12 // BIP39 mnemonic should have 12 words
        } else {
            false
        }
    }
}

#[tokio::test]
async fn test_simple_pairing_code_generation() {
    println!("🔬 Testing simple pairing code generation...");
    
    // Test that Alice can successfully generate a pairing code
    let test_id = Uuid::new_v4();
    let temp_dir = std::env::temp_dir().join(format!("simple-test-{}", test_id));

    // Build helper binary
    let build_result = Command::new("cargo")
        .args(&["build", "--bin", "cli_pairing_subprocess_helper"])
        .output()
        .expect("Failed to build helper binary");

    if !build_result.status.success() {
        panic!("❌ Failed to build helper binary: {}", 
               String::from_utf8_lossy(&build_result.stderr));
    }

    let binary_path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("cli_pairing_subprocess_helper");

    // Test pairing code generation
    let result = run_single_process(&binary_path, &temp_dir, "initiator", None).await;
    
    // Verify the results
    assert!(result.is_successful(), 
            "Process should complete successfully. Errors: {:?}", result.errors);
    
    assert!(result.has_valid_pairing_code(), 
            "Should generate valid 12-word pairing code. Got: {:?}", result.pairing_code);
    
    assert!(result.expires_in.is_some(), 
            "Should have expiration time");
    
    let expires_in = result.expires_in.unwrap();
    assert!(expires_in > 200 && expires_in <= 300, 
            "Expiration should be reasonable (got {})", expires_in);
    
    assert!(result.device_state_verified, 
            "Device state should be verified");
    
    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
    
    println!("✅ Simple pairing code generation test passed!");
    println!("   Generated code: {}...", 
             result.pairing_code.unwrap().split_whitespace().take(3).collect::<Vec<_>>().join(" "));
    println!("   Expires in: {} seconds", expires_in);
}

#[tokio::test] 
async fn test_pairing_error_handling() {
    println!("🧪 Testing pairing error handling...");
    
    let test_id = Uuid::new_v4();
    let temp_dir = std::env::temp_dir().join(format!("error-test-{}", test_id));

    let binary_path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("cli_pairing_subprocess_helper");

    // Test joiner without pairing code (should fail gracefully)
    let result = run_single_process(&binary_path, &temp_dir, "joiner", None).await;
    
    // Should fail with proper error
    assert!(!result.is_successful(), "Joiner without code should fail");
    assert!(!result.errors.is_empty(), "Should have error messages");
    assert!(result.errors.iter().any(|e| e.contains("MISSING_PAIRING_CODE")), 
            "Should have missing pairing code error");
    
    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
    
    println!("✅ Error handling test passed!");
}

#[tokio::test]
async fn test_pairing_code_format_validation() {
    println!("🔍 Testing pairing code format validation...");
    
    let test_id = Uuid::new_v4();
    let temp_dir = std::env::temp_dir().join(format!("format-test-{}", test_id));

    let binary_path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("cli_pairing_subprocess_helper");

    // Test with invalid pairing code
    let invalid_code = "invalid short code";
    let result = run_single_process(&binary_path, &temp_dir, "joiner", Some(invalid_code)).await;
    
    // Should fail due to invalid format
    assert!(!result.is_successful(), "Invalid pairing code should fail");
    
    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
    
    println!("✅ Pairing code format validation test passed!");
}

async fn run_single_process(
    binary_path: &std::path::PathBuf,
    data_dir: &std::path::PathBuf,
    role: &str,
    pairing_code: Option<&str>
) -> ProcessResult {
    println!("🔧 Running {} process...", role);
    
    let data_dir_str = data_dir.to_string_lossy().to_string();
    let role_str = role.to_string();
    let pairing_code_str = pairing_code.map(|s| s.to_string());
    
    let mut args = vec![
        role_str.clone(),
        data_dir_str,
        "test-password".to_string()
    ];
    
    if let Some(ref code) = pairing_code_str {
        args.push(code.clone());
    }
    
    let mut process = tokio::task::spawn_blocking({
        let binary_path = binary_path.clone();
        let args = args.clone();
        move || {
            Command::new(&binary_path)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to spawn process")
        }
    }).await.unwrap();

    let stdout = process.stdout.take().expect("Failed to capture stdout");
    let mut reader = BufReader::new(stdout);
    
    let mut result = ProcessResult::new();
    let mut line = String::new();
    
    // Parse process output with timeout
    let parse_result = tokio::time::timeout(Duration::from_secs(30), tokio::task::spawn_blocking(move || {
        let mut attempts = 0;
        while attempts < 300 {  // 30 second timeout
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    println!("🔧 {}: {}", role_str, trimmed);
                    
                    parse_process_output(&mut result, trimmed);
                    
                    // Check for completion
                    if trimmed == "STATUS:SUCCESS" || trimmed.starts_with("ERROR:") {
                        break;
                    }
                }
                Err(_) => {
                    attempts += 1;
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            attempts += 1;
        }
        result
    })).await;

    let final_result = match parse_result {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => {
            let mut result = ProcessResult::new();
            result.errors.push("Process task failed".to_string());
            result
        }
        Err(_) => {
            let mut result = ProcessResult::new();
            result.errors.push("Process timed out".to_string());
            result
        }
    };

    // Wait for process completion
    let exit_status = tokio::task::spawn_blocking(move || process.wait()).await.unwrap().unwrap();
    
    if !exit_status.success() {
        println!("⚠️  Process exited with code: {:?}", exit_status.code());
    }

    final_result
}

fn parse_process_output(result: &mut ProcessResult, line: &str) {
    if line.starts_with("STATUS:") {
        result.current_status = line.trim_start_matches("STATUS:").to_string();
        
        if line == "STATUS:DEVICE_STATE_VERIFIED" {
            result.device_state_verified = true;
        }
    } else if line.starts_with("PAIRING_CODE:") {
        result.pairing_code = Some(line.trim_start_matches("PAIRING_CODE:").to_string());
    } else if line.starts_with("EXPIRES_IN:") {
        if let Ok(expires) = line.trim_start_matches("EXPIRES_IN:").parse::<u64>() {
            result.expires_in = Some(expires);
        }
    } else if line.starts_with("ERROR:") {
        result.errors.push(line.to_string());
    }
}