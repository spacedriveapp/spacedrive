mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::search::{
    output::FileSearchOutput, query::FileSearchQuery,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum SearchCmd {
    /// Search for files
    Files(FileSearchArgs),
}

pub async fn run(ctx: &Context, cmd: SearchCmd) -> Result<()> {
    match cmd {
        SearchCmd::Files(args) => {
            let input: sd_core::ops::search::input::FileSearchInput = args.into();
            let out: FileSearchOutput = execute_query!(ctx, FileSearchQuery { input });
            print_output!(ctx, &out, |o: &FileSearchOutput| {
                if o.results.is_empty() {
                    println!("No files found");
                    return;
                }
                
                println!("Found {} files ({} total)", o.results.len(), o.total_found);
                println!("Search ID: {}", o.search_id);
                println!("Execution time: {}ms", o.execution_time_ms);
                println!();
                
                for (i, result) in o.results.iter().enumerate() {
                    println!("{}. {} (score: {:.2})", 
                        i + 1, 
                        result.entry.name, 
                        result.score
                    );
                    
                    if let Some(extension) = &result.entry.extension {
                        println!("   Extension: {}", extension);
                    }
                    
                    println!("   Size: {} bytes", result.entry.size_in_bytes);
                    println!("   Modified: {}", result.entry.modified_at.format("%Y-%m-%d %H:%M:%S"));
                    println!("   Location: {}", result.entry.location_id);
                    
                    if !result.highlights.is_empty() {
                        println!("   Highlights:");
                        for highlight in &result.highlights {
                            println!("     {}: {}", highlight.field, highlight.text);
                        }
                    }
                    
                    if let Some(content) = &result.matched_content {
                        println!("   Matched content: {}", content);
                    }
                    
                    println!();
                }
                
                // Show facets if available
                if !o.facets.file_types.is_empty() {
                    println!("File types:");
                    for (file_type, count) in &o.facets.file_types {
                        println!("  {}: {}", file_type, count);
                    }
                    println!();
                }
                
                if !o.suggestions.is_empty() {
                    println!("Suggestions:");
                    for suggestion in &o.suggestions {
                        println!("  {}", suggestion);
                    }
                }
            });
        }
    }
    Ok(())
}