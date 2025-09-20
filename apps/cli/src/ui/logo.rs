/// Spacedrive ASCII logo generated with oh-my-logo
/// Generated with: npx oh-my-logo "SPACEDRIVE" dawn --filled --no-color
pub const SPACEDRIVE_LOGO: &str = r#"
███████╗ ██████╗   █████╗   ██████╗ ███████╗ ██████╗  ██████╗  ██╗ ██╗   ██╗ ███████╗
██╔════╝ ██╔══██╗ ██╔══██╗ ██╔════╝ ██╔════╝ ██╔══██╗ ██╔══██╗ ██║ ██║   ██║ ██╔════╝
███████╗ ██████╔╝ ███████║ ██║      █████╗   ██║  ██║ ██████╔╝ ██║ ██║   ██║ █████╗
╚════██║ ██╔═══╝  ██╔══██║ ██║      ██╔══╝   ██║  ██║ ██╔══██╗ ██║ ╚██╗ ██╔╝ ██╔══╝
███████║ ██║      ██║  ██║ ╚██████╗ ███████╗ ██████╔╝ ██║  ██║ ██║  ╚████╔╝  ███████╗
╚══════╝ ╚═╝      ╚═╝  ╚═╝  ╚═════╝ ╚══════╝ ╚═════╝  ╚═╝  ╚═╝ ╚═╝   ╚═══╝   ╚══════╝
"#;

/// Print the Spacedrive logo with colors using ANSI escape codes
/// Colors using a light blue to purple gradient
pub fn print_logo_colored() {
	// Light blue to purple gradient colors
	let lines = SPACEDRIVE_LOGO.lines().collect::<Vec<_>>();

	for (i, line) in lines.iter().enumerate() {
		if line.trim().is_empty() {
			println!();
			continue;
		}

		// Create a gradient effect from light blue to purple
		let color_code = match i % 6 {
			0 => "\x1b[38;5;117m", // Light blue
			1 => "\x1b[38;5;111m", // Sky blue
			2 => "\x1b[38;5;105m", // Light purple-blue
			3 => "\x1b[38;5;99m",  // Medium purple
			4 => "\x1b[38;5;93m",  // Purple
			_ => "\x1b[38;5;129m", // Deep purple
		};

		println!("{}{}\x1b[0m", color_code, line);
	}

	println!("                           Cross-platform file management");
	println!();
}

/// Display a compact version of the logo
pub fn print_compact_logo() {
	println!("Spacedrive CLI v2");
}
