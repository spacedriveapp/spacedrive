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
	if assume_yes
		|| std::env::var("SD_CLI_YES")
			.map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
			.unwrap_or(false)
	{
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
				println!(
					"Invalid input. Please enter a number between 1 and {}.",
					request.choices.len()
				);
			}
		}
	}
}

/// Prompt the user for text input.
/// Returns the trimmed input string, or None if empty and optional.
pub fn prompt_for_text(prompt: &str, optional: bool) -> Result<Option<String>> {
	use std::io::{self, Write};

	loop {
		if optional {
			print!("{} (optional): ", prompt);
		} else {
			print!("{}: ", prompt);
		}
		io::stdout().flush()?;

		let mut input = String::new();
		io::stdin().read_line(&mut input)?;
		let trimmed = input.trim().to_string();

		if trimmed.is_empty() {
			if optional {
				return Ok(None);
			} else {
				println!("This field is required. Please enter a value.");
				continue;
			}
		}

		return Ok(Some(trimmed));
	}
}

/// Simplified prompt for a list of string choices.
/// Returns the 0-based index of the selected choice.
pub fn prompt_for_list(message: &str, choices: &[String]) -> Result<usize> {
	use std::io::{self, Write};

	println!("{}", message);
	for (i, choice) in choices.iter().enumerate() {
		println!("  [{}]: {}", i + 1, choice);
	}

	loop {
		print!("Select (1-{}): ", choices.len());
		io::stdout().flush()?;

		let mut input = String::new();
		io::stdin().read_line(&mut input)?;

		match input.trim().parse::<usize>() {
			Ok(num) if num > 0 && num <= choices.len() => {
				return Ok(num - 1);
			}
			_ => {
				println!("Invalid. Please enter 1-{}.", choices.len());
			}
		}
	}
}

/// Modern arrow-key based selection with inquire.
/// Supports arrow keys (↑↓) and number shortcuts (1, 2, 3...).
pub fn select(message: &str, choices: &[String]) -> Result<usize> {
	use inquire::Select;

	let selection = Select::new(message, choices.to_vec())
		.with_page_size(10)
		.with_help_message("Use ↑↓ to navigate, Enter to select, or type a number")
		.prompt()?;

	// Find the index of the selected item
	let idx = choices
		.iter()
		.position(|c| c == &selection)
		.ok_or_else(|| anyhow::anyhow!("Selection not found"))?;

	Ok(idx)
}

/// Modern text input with inquire.
pub fn text(message: &str, optional: bool) -> Result<Option<String>> {
	use inquire::Text;

	let prompt_text = if optional {
		format!("{} (optional)", message)
	} else {
		message.to_string()
	};

	let mut prompt = Text::new(&prompt_text);

	if !optional {
		prompt = prompt.with_validator(|input: &str| {
			if input.trim().is_empty() {
				Ok(inquire::validator::Validation::Invalid(
					"This field is required".into(),
				))
			} else {
				Ok(inquire::validator::Validation::Valid)
			}
		});
	}

	let result = prompt.prompt()?;

	if result.trim().is_empty() && optional {
		Ok(None)
	} else {
		Ok(Some(result))
	}
}
