use anyhow::Result;
use sd_core::infra::action::ConfirmationRequest;

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

/// Prompt the user for a multiple-choice selection.
/// Returns the 0-based index of the selected choice.
pub fn prompt_for_choice(request: ConfirmationRequest) -> Result<usize> {
    use std::io::{self, Write};

    println!("{}", request.message);
    for (i, choice) in request.choices.iter().enumerate() {
        println!("  [{}]: {}", i + 1, choice);
    }

    loop {
        print!("Please select an option (1-{}): ", request.choices.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().parse::<usize>() {
            Ok(num) if num > 0 && num <= request.choices.len() => {
                // Return the 0-based index
                return Ok(num - 1);
            }
            _ => {
                println!("Invalid input. Please enter a number between 1 and {}.", request.choices.len());
            }
        }
    }
}