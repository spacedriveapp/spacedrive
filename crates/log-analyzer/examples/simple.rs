//! Simple example of using the log analyzer library.

use anyhow::Result;
use log_analyzer::LogAnalyzer;

fn main() -> Result<()> {
	// Example log content
	let log_content = r#"
2025-11-16T07:19:57.232531Z DEBUG ThreadId(02) sd_core::service::sync::peer: Recorded ACK from peer peer=1817e146 hlc=HLC(1763277539319,1,:1817e146)
2025-11-16T07:19:57.232532Z DEBUG ThreadId(02) sd_core::service::sync::peer: Recorded ACK from peer peer=1817e146 hlc=HLC(1763277539319,2,:1817e146)
2025-11-16T07:19:57.232533Z DEBUG ThreadId(02) sd_core::service::sync::peer: Recorded ACK from peer peer=1817e146 hlc=HLC(1763277539320,1,:1817e146)
2025-11-16T07:19:57.232534Z INFO sd_core::service::sync: Sync completed successfully
2025-11-16T07:19:57.232535Z DEBUG ThreadId(03) sd_core::service::sync::protocol_handler: Handling shared change content_identity
2025-11-16T07:19:57.232536Z DEBUG ThreadId(03) sd_core::service::sync::protocol_handler: Handling shared change content_identity
"#;

	// Parse and analyze
	let analyzer = LogAnalyzer::from_string(log_content)?;

	println!("Analysis Results:");
	println!("  Total logs: {}", analyzer.log_count());
	println!("  Templates: {}", analyzer.template_count());
	println!("  Groups: {}", analyzer.group_count());
	println!(
		"  Compression: {:.1}%",
		analyzer.compression_ratio() * 100.0
	);

	println!("\nTemplates:");
	for template in analyzer.templates() {
		println!(
			"  #{}: {} ({}×)",
			template.id, template.example, template.total_count
		);
	}

	println!("\nGroups:");
	for group in analyzer.groups() {
		println!(
			"  Template #{}: {} instances, {}ms duration",
			group.template_id, group.count, group.duration_ms
		);
	}

	// Generate markdown report
	let report = analyzer.generate_markdown_report()?;
	println!("\n--- Markdown Report ---\n{}", report);

	Ok(())
}






