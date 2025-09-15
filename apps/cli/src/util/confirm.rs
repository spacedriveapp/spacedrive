use anyhow::Result;

/// Prompt the user for confirmation before executing a dangerous action.
///
/// Behavior:
/// - Returns Ok(()) if the user confirms with "y" or "yes" (case-insensitive)
/// - Respects an explicit `assume_yes` flag to skip prompting
/// - Also respects `SD_CLI_YES=1` environment variable to skip prompting
/// - Otherwise returns an error ("Aborted by user") to allow early exit
pub fn confirm_or_abort(prompt: &str, assume_yes: bool) -> Result<()> {
    if assume_yes || std::env::var("SD_CLI_YES").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false) {
        return Ok(());
    }

    use std::io::{self, Write};
    let mut stderr = io::stderr();
    writeln!(stderr, "{} [y/N]: ", prompt)?;
    stderr.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let resp = input.trim().to_ascii_lowercase();
    if resp == "y" || resp == "yes" {
        Ok(())
    } else {
        anyhow::bail!("Aborted by user")
    }
}