//! Create a test memory file for development
//!
//! This example creates a real .memory file demonstrating the format.
//! Run with: cargo run --example create_memory

use sd_core::domain::memory::{DocumentType, FactType, MemoryFile, MemoryScope};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize logging
	tracing_subscriber::fmt()
		.with_env_filter("info")
		.init();

	println!("\nCreating Spacedrive Memory File\n");

	// Output path
	let output_path = PathBuf::from(
		"/Users/jamespine/Projects/spacedrive/workbench/test-memories/memory-file-system.memory",
	);

	// Ensure directory exists
	if let Some(parent) = output_path.parent() {
		std::fs::create_dir_all(parent)?;
	}

	// Create memory file
	let mut memory = MemoryFile::create(
		"memory-file-system".to_string(),
		MemoryScope::Directory {
			path: "/Users/jamespine/Projects/spacedrive/core/src/domain/memory".to_string(),
		},
		&output_path,
	)
	.await?;

	println!("Created memory archive\n");

	// Add design documents
	println!("Adding documents...");

	let design_doc = memory
		.add_document(
			None,
			"MEMORY_FILE_FORMAT_DESIGN.md".to_string(),
			Some(
				"Complete specification for .memory file format with custom archive".to_string(),
			),
			DocumentType::Design,
		)
		.await?;

	let impl_doc = memory
		.add_document(
			None,
			"MEMORY_FILE_IMPLEMENTATION_STATUS.md".to_string(),
			Some("Implementation status with custom archive format".to_string()),
			DocumentType::Documentation,
		)
		.await?;

	let agent_doc = memory
		.add_document(
			None,
			"AGENT_MEMORY_ARCHITECTURE_V1.md".to_string(),
			Some("Three-type agent memory architecture".to_string()),
			DocumentType::Design,
		)
		.await?;

	println!("  {} documents added\n", memory.get_documents().len());

	// Add learned facts
	println!("Adding facts...");

	memory
		.add_fact(
			"Memory files use custom archive format with magic bytes SDMEMORY".to_string(),
			FactType::Principle,
			1.0,
			Some(design_doc),
		)
		.await?;

	memory
		.add_fact(
			"Archive is append-only with index at end for efficient updates".to_string(),
			FactType::Pattern,
			1.0,
			Some(impl_doc),
		)
		.await?;

	memory
		.add_fact(
			"Vector store embedded using MessagePack serialization".to_string(),
			FactType::Decision,
			0.9,
			Some(impl_doc),
		)
		.await?;

	memory
		.add_fact(
			"Agent memory types: temporal (events), associative (knowledge), working (current)".to_string(),
			FactType::Pattern,
			1.0,
			Some(agent_doc),
		)
		.await?;

	memory
		.add_fact(
			"Memory files solve context-gathering problem for AI agents".to_string(),
			FactType::Principle,
			1.0,
			Some(design_doc),
		)
		.await?;

	println!("  {} facts added\n", memory.get_facts().len());

	// Add embeddings
	println!("Adding embeddings...");

	// 4D mock vectors (real would be 384D from AI model)
	let design_vector = vec![0.9, 0.2, 0.7, 0.1];
	let impl_vector = vec![0.1, 0.9, 0.3, 0.5];
	let agent_vector = vec![0.3, 0.2, 0.95, 0.1];

	memory.add_embedding(design_doc, design_vector).await?;
	memory.add_embedding(impl_doc, impl_vector).await?;
	memory.add_embedding(agent_doc, agent_vector).await?;

	println!(
		"  {} embeddings added\n",
		memory.embedding_count().await?
	);

	// Test search
	println!("Testing semantic search...");
	let query = vec![0.7, 0.15, 0.85, 0.05]; // Query: design + architecture
	let results = memory.search_similar(query, 3).await?;

	println!("  Results:");
	for (i, doc_id) in results.iter().enumerate() {
		if let Some(doc) = memory.get_document(*doc_id) {
			println!("    {}. {}", i + 1, doc.title);
		}
	}
	println!();

	// Show final statistics
	let metadata = memory.metadata();
	let stats = &metadata.statistics;

	println!("Memory Statistics:");
	println!("  Name: {}", metadata.name);
	println!("  Scope: {}", metadata.scope.identifier());
	println!("  Documents: {}", stats.document_count);
	println!("  Facts: {}", stats.fact_count);
	println!("  Embeddings: {}", stats.embedding_count);
	println!("  Total size: {} bytes", stats.file_size_bytes);
	println!();

	println!("Memory file created successfully!");
	println!("Location: {}", output_path.display());
	println!();
	println!("Verify:");
	println!("  file {}", output_path.display());
	println!("  hexdump -C {} | head -20", output_path.display());
	println!();

	// Verify single file
	assert!(output_path.is_file());
	assert!(!output_path.is_dir());
	println!("Confirmed: Single file archive\n");

	Ok(())
}
