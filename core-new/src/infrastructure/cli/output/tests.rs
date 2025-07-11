//! Tests for the CLI output system

#[cfg(test)]
mod tests {
    use crate::infrastructure::cli::output::*;
    use crate::infrastructure::cli::output::messages::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn test_human_format_basic_messages() {
        let (mut output, buffer) = CliOutput::test();
        
        output.success("Operation successful").unwrap();
        output.error(Message::Error("Something went wrong".to_string())).unwrap();
        output.warning("This is a warning").unwrap();
        output.info("Some information").unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Operation successful"));
        assert!(result.contains("Something went wrong"));
        assert!(result.contains("This is a warning"));
        assert!(result.contains("Some information"));
    }

    #[test]
    fn test_json_format() {
        use std::sync::{Arc, Mutex};
        use super::super::TestWriter;
        
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = TestWriter { buffer: buffer.clone() };
        let mut output = CliOutput {
            context: OutputContext::test(Box::new(writer)),
        };
        output.context.format = OutputFormat::Json;
        output.context.formatter = Box::new(crate::infrastructure::cli::output::formatters::JsonFormatter);
        
        output.print(Message::LibraryCreated {
            name: "Test Library".to_string(),
            id: Uuid::new_v4(),
            path: PathBuf::from("/test/path"),
        }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&result.trim()).unwrap();
        
        assert_eq!(json["type"], "library_created");
        assert_eq!(json["success"], true);
        assert_eq!(json["data"]["name"], "Test Library");
    }

    #[test]
    fn test_quiet_format() {
        use std::sync::{Arc, Mutex};
        use super::super::TestWriter;
        
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = TestWriter { buffer: buffer.clone() };
        let mut output = CliOutput {
            context: OutputContext::test(Box::new(writer)),
        };
        output.context.format = OutputFormat::Quiet;
        
        // Normal messages should not appear in quiet mode
        output.info("This should not appear").unwrap();
        output.success("This also should not appear").unwrap();
        
        // Errors should always appear
        output.error(Message::Error("This error should appear".to_string())).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(!result.contains("This should not appear"));
        assert!(!result.contains("This also should not appear"));
        assert!(result.contains("This error should appear"));
    }

    #[test]
    fn test_verbosity_levels() {
        use std::sync::{Arc, Mutex};
        use super::super::TestWriter;
        
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = TestWriter { buffer: buffer.clone() };
        let mut output = CliOutput::with_options(
            OutputFormat::Human,
            VerbosityLevel::Normal,
            ColorMode::Never
        );
        output.context = OutputContext::test(Box::new(writer));
        
        // Normal messages should appear
        output.info("Normal info").unwrap();
        
        // Debug messages should not appear at normal verbosity
        output.print(Message::Debug("Debug info".to_string())).unwrap();
        
        // Progress messages (verbose level) should not appear
        output.print(Message::IndexingProgress {
            current: 10,
            total: 100,
            location: "test".to_string(),
        }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Normal info"));
        assert!(!result.contains("Debug info"));
        assert!(!result.contains("Indexing"));
    }

    #[test]
    fn test_library_messages() {
        let (mut output, buffer) = CliOutput::test();
        
        let lib_id = Uuid::new_v4();
        output.print(Message::LibraryCreated {
            name: "My Library".to_string(),
            id: lib_id,
            path: PathBuf::from("/home/user/library"),
        }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Library 'My Library' created successfully"));
        assert!(result.contains(&lib_id.to_string()));
        assert!(result.contains("/home/user/library"));
    }

    #[test]
    fn test_device_list() {
        let (mut output, buffer) = CliOutput::test();
        
        let devices = vec![
            DeviceInfo {
                id: "device1".to_string(),
                name: "My Computer".to_string(),
                status: DeviceStatus::Online,
                peer_id: None,
            },
            DeviceInfo {
                id: "device2".to_string(),
                name: "My Phone".to_string(),
                status: DeviceStatus::Paired,
                peer_id: Some("peer123".to_string()),
            },
        ];
        
        output.print(Message::DevicesList { devices }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Discovered devices"));
        assert!(result.contains("My Computer"));
        assert!(result.contains("My Phone"));
        assert!(result.contains("Online"));
        assert!(result.contains("Paired"));
    }

    #[test]
    fn test_section_builder_basic() {
        let (mut output, buffer) = CliOutput::test();
        
        output.section()
            .title("Test Section")
            .status("Version", "1.0.0")
            .status("Status", "Running")
            .empty_line()
            .text("Some additional text")
            .render()
            .unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Test Section"));
        assert!(result.contains("Version: 1.0.0"));
        assert!(result.contains("Status: Running"));
        assert!(result.contains("Some additional text"));
    }

    #[test]
    fn test_section_builder_with_table() {
        let (mut output, buffer) = CliOutput::test();
        
        let mut table = comfy_table::Table::new();
        table.set_header(vec!["ID", "Name", "Status"]);
        table.add_row(vec!["1", "Item 1", "Active"]);
        table.add_row(vec!["2", "Item 2", "Inactive"]);
        
        output.section()
            .title("Items")
            .table(table)
            .render()
            .unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Items"));
        assert!(result.contains("ID"));
        assert!(result.contains("Name"));
        assert!(result.contains("Status"));
        assert!(result.contains("Item 1"));
        assert!(result.contains("Active"));
    }

    #[test]
    fn test_help_section() {
        let (mut output, buffer) = CliOutput::test();
        
        output.section()
            .title("Available Commands")
            .help()
                .item("create - Create a new item")
                .item("list - List all items")
                .item("delete - Delete an item")
            .render()
            .unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Available Commands"));
        assert!(result.contains("Tips:"));
        assert!(result.contains("• create - Create a new item"));
        assert!(result.contains("• list - List all items"));
        assert!(result.contains("• delete - Delete an item"));
    }

    #[test]
    fn test_progress_messages() {
        use std::sync::{Arc, Mutex};
        use super::super::TestWriter;
        
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = TestWriter { buffer: buffer.clone() };
        let mut output = CliOutput::with_options(
            OutputFormat::Human,
            VerbosityLevel::Verbose,
            ColorMode::Never
        );
        output.context = OutputContext::test(Box::new(writer));
        output.context.verbosity = VerbosityLevel::Verbose;
        
        output.print(Message::IndexingProgress {
            current: 150,
            total: 1000,
            location: "/home/user/documents".to_string(),
        }).unwrap();
        
        output.print(Message::CopyProgress {
            current: 5,
            total: 10,
            current_file: Some("file.txt".to_string()),
        }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Indexing /home/user/documents: 150/1000 files"));
        assert!(result.contains("Copying file.txt: 5/10 files"));
    }

    #[test]
    fn test_pairing_messages() {
        let (mut output, buffer) = CliOutput::test();
        
        output.print(Message::PairingCodeGenerated {
            code: "ABC123".to_string(),
        }).unwrap();
        
        output.print(Message::PairingSuccess {
            device_name: "John's Phone".to_string(),
            device_id: "device123".to_string(),
        }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Pairing code generated"));
        assert!(result.contains("ABC123"));
        assert!(result.contains("Successfully paired with John's Phone"));
    }

    #[test]
    fn test_job_messages() {
        let (mut output, buffer) = CliOutput::test();
        
        let job_id = Uuid::new_v4();
        
        output.print(Message::JobStarted {
            id: job_id,
            name: "File Copy".to_string(),
        }).unwrap();
        
        output.print(Message::JobCompleted {
            id: job_id,
            name: "File Copy".to_string(),
            duration: 42,
        }).unwrap();
        
        output.print(Message::JobFailed {
            id: job_id,
            name: "File Validation".to_string(),
            error: "Checksum mismatch".to_string(),
        }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Job started: File Copy"));
        assert!(result.contains("Job completed: File Copy (42s)"));
        assert!(result.contains("Job failed: File Validation"));
        assert!(result.contains("Checksum mismatch"));
    }

    #[test]
    fn test_empty_line_deduplication() {
        let (mut output, buffer) = CliOutput::test();
        
        output.section()
            .title("Test")
            .empty_line()
            .empty_line()  // Should be deduplicated
            .empty_line()  // Should be deduplicated
            .text("Content")
            .render()
            .unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        
        // Count empty lines between "Test" and "Content"
        let empty_count = lines.iter()
            .skip_while(|&&line| !line.contains("Test"))
            .take_while(|&&line| !line.contains("Content"))
            .filter(|&&line| line.trim().is_empty())
            .count();
        
        assert_eq!(empty_count, 1, "Should have exactly one empty line");
    }

    #[test]
    fn test_color_mode_detection() {
        // Test auto mode (will be false in test environment)
        let ctx = OutputContext::with_options(
            OutputFormat::Human,
            VerbosityLevel::Normal,
            ColorMode::Auto
        );
        // In test environment, should detect no color support
        assert!(!ctx.use_color());
        
        // Test always mode
        let ctx = OutputContext::with_options(
            OutputFormat::Human,
            VerbosityLevel::Normal,
            ColorMode::Always
        );
        assert!(ctx.use_color());
        
        // Test never mode
        let ctx = OutputContext::with_options(
            OutputFormat::Human,
            VerbosityLevel::Normal,
            ColorMode::Never
        );
        assert!(!ctx.use_color());
    }

    #[test]
    fn test_daemon_status_message() {
        let (mut output, buffer) = CliOutput::test();
        
        output.print(Message::DaemonStatus {
            version: "2.0.0".to_string(),
            uptime: 3600,
            instance: "default".to_string(),
            networking_enabled: true,
            libraries: vec![
                LibraryInfo {
                    id: Uuid::new_v4(),
                    name: "Main Library".to_string(),
                    path: PathBuf::from("/libraries/main"),
                }
            ],
        }).unwrap();
        
        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        
        assert!(result.contains("Spacedrive Daemon Status"));
        assert!(result.contains("Version: 2.0.0"));
        assert!(result.contains("Instance: default"));
        assert!(result.contains("Uptime: 3600 seconds"));
        assert!(result.contains("Networking: Enabled"));
        assert!(result.contains("Libraries: 1"));
    }
}