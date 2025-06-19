//! CLI utility functions

use crate::{library::Library, location::ManagedLocation};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;

/// Format library information for display
pub async fn format_library_info(library: &Arc<Library>) -> String {
    let mut output = String::new();
    
    output.push_str(&format!("{}\n", style("Library Information").bold().underline()));
    output.push_str(&format!("  {} {}\n", style("Name:").bold(), style(library.name().await).cyan()));
    output.push_str(&format!("  {} {}\n", style("ID:").bold(), style(library.id()).yellow()));
    output.push_str(&format!("  {} {}\n", style("Path:").bold(), style(library.path().display()).dim()));
    
    let config = library.config().await;
    output.push_str(&format!("  {} {}\n", style("Created:").bold(), style(config.created_at.format("%Y-%m-%d %H:%M:%S")).dim()));
    output.push_str(&format!("  {} {}\n", style("Updated:").bold(), style(config.updated_at.format("%Y-%m-%d %H:%M:%S")).dim()));
    
    if let Some(desc) = &config.description {
        output.push_str(&format!("  {} {}\n", style("Description:").bold(), desc));
    }
    
    // Statistics
    output.push_str(&format!("\n{}\n", style("Statistics").bold().underline()));
    output.push_str(&format!("  {} {}\n", style("Total files:").bold(), style(config.statistics.total_file_count).cyan()));
    output.push_str(&format!("  {} {}\n", style("Total size:").bold(), format_bytes(config.statistics.total_byte_size)));
    output.push_str(&format!("  {} {}\n", style("Locations:").bold(), style(config.statistics.location_count).cyan()));
    output.push_str(&format!("  {} {}\n", style("Indexed files:").bold(), style(config.statistics.indexed_file_count).cyan()));
    
    output
}

/// Format location information for display
pub fn format_location_info(location: &ManagedLocation) -> String {
    let mut output = String::new();
    
    output.push_str(&format!("{}\n", style("Location Information").bold().underline()));
    output.push_str(&format!("  {} {}\n", style("Name:").bold(), style(&location.name).cyan()));
    output.push_str(&format!("  {} {}\n", style("ID:").bold(), style(location.id).yellow()));
    output.push_str(&format!("  {} {}\n", style("Path:").bold(), style(location.path.display()).dim()));
    output.push_str(&format!("  {} {:?}\n", style("Index Mode:").bold(), location.index_mode));
    output.push_str(&format!("  {} {}\n", style("Watch Enabled:").bold(), 
        if location.watch_enabled { style("Yes").green() } else { style("No").red() }
    ));
    output.push_str(&format!("  {} {}\n", style("Indexing Enabled:").bold(), 
        if location.indexing_enabled { style("Yes").green() } else { style("No").red() }
    ));
    
    output
}

/// Print job information
pub fn print_job_info(
    job_id: &str,
    job_type: &str,
    status: &str,
    progress: Option<f64>,
    message: Option<&str>,
) {
    println!("{}", style("Job Information").bold().underline());
    println!("  {} {}", style("ID:").bold(), style(job_id).yellow());
    println!("  {} {}", style("Type:").bold(), style(job_type).cyan());
    println!("  {} {}", style("Status:").bold(), format_status(status));
    
    if let Some(prog) = progress {
        println!("  {} {:.1}%", style("Progress:").bold(), prog * 100.0);
    }
    
    if let Some(msg) = message {
        println!("  {} {}", style("Message:").bold(), msg);
    }
}

/// Create and display a progress bar
pub fn print_progress_bar(current: u64, total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_position(current);
    pb.set_message(message.to_string());
    pb
}

/// Format status with color
fn format_status(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "running" | "active" | "indexing" => format!("{}", style(status).yellow()),
        "completed" | "success" | "done" => format!("{}", style(status).green()),
        "failed" | "error" => format!("{}", style(status).red()),
        "cancelled" | "stopped" => format!("{}", style(status).magenta()),
        "pending" | "queued" => format!("{}", style(status).blue()),
        _ => status.to_string(),
    }
}

/// Format bytes into human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Format duration into human-readable string
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

/// Print a table with headers and rows
pub fn print_table(headers: Vec<&str>, rows: Vec<Vec<String>>) {
    use console::Term;
    use std::cmp::max;
    
    let term = Term::stdout();
    let term_width = term.size().1 as usize;
    
    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = max(widths[i], cell.len());
            }
        }
    }
    
    // Ensure we don't exceed terminal width
    let total_width: usize = widths.iter().sum::<usize>() + (widths.len() - 1) * 3;
    if total_width > term_width {
        let scale = term_width as f64 / total_width as f64;
        for width in &mut widths {
            *width = (*width as f64 * scale) as usize;
        }
    }
    
    // Print headers
    let header_row: Vec<String> = headers.iter()
        .enumerate()
        .map(|(i, h)| format!("{:width$}", h, width = widths[i]))
        .collect();
    println!("{}", style(header_row.join(" | ")).bold());
    
    // Print separator
    let separator: Vec<String> = widths.iter()
        .map(|&w| "-".repeat(w))
        .collect();
    println!("{}", style(separator.join("-+-")).dim());
    
    // Print rows
    for row in rows {
        let formatted_row: Vec<String> = row.iter()
            .enumerate()
            .map(|(i, cell)| {
                if i < widths.len() {
                    let width = widths[i];
                    if cell.len() > width {
                        format!("{:width$.width$}…", cell, width = width - 1)
                    } else {
                        format!("{:width$}", cell, width = width)
                    }
                } else {
                    cell.clone()
                }
            })
            .collect();
        println!("{}", formatted_row.join(" | "));
    }
}

/// Create a simple spinner for long-running operations
pub fn create_spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(59), "59s");
        assert_eq!(format_duration(60), "1m 0s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3600), "1h 0m");
        assert_eq!(format_duration(3661), "1h 1m");
    }
}