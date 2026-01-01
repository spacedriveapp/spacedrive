/// Calculate brightness for a point on a sphere with lighting
fn calculate_sphere_brightness(x: f32, y: f32, radius: f32) -> Option<f32> {
	let dx = x;
	let dy = y;
	let distance = (dx * dx + dy * dy).sqrt();

	// Slightly reduce effective radius to avoid stray single pixels at edges
	if distance > radius - 0.5 {
		return None;
	}

	// Calculate z-coordinate on sphere surface
	let z = (radius * radius - dx * dx - dy * dy).sqrt();

	// Normal vector (pointing outward from sphere)
	let nx = dx / radius;
	let ny = dy / radius;
	let nz = z / radius;

	// Light from top-left-front
	let lx: f32 = -0.4;
	let ly: f32 = -0.3;
	let lz: f32 = 0.8;
	let light_len = (lx * lx + ly * ly + lz * lz).sqrt();
	let lx = lx / light_len;
	let ly = ly / light_len;
	let lz = lz / light_len;

	// Diffuse lighting
	let diffuse = (nx * lx + ny * ly + nz * lz).max(0.0);

	// Specular highlight
	let view_z = 1.0;
	let reflect_z = 2.0 * diffuse * nz - lz;
	let specular = reflect_z.max(0.0).powf(20.0);

	// Combine ambient, diffuse, and specular
	let brightness = 0.2 + diffuse * 0.6 + specular * 0.8;

	Some(brightness.min(1.0))
}

/// Get RGB color for purple gradient based on brightness
fn get_purple_color(brightness: f32) -> (u8, u8, u8) {
	// Purple color palette - from dark to bright
	let r = (80.0 + brightness * 175.0) as u8;
	let g = (40.0 + brightness * 100.0) as u8;
	let b = (120.0 + brightness * 135.0) as u8;
	(r, g, b)
}

/// Print the Spacedrive logo as a purple orb using ANSI colors and Unicode half-blocks
pub fn print_logo_colored() {
	let width = 36;
	let height = 18;
	let radius = 9.0;
	let center_x = width as f32 / 2.0;
	let center_y = height as f32 / 2.0;

	println!();

	// Render using half-blocks for 2x vertical resolution
	for row in 0..height {
		print!("                    ");
		for col in 0..width {
			let x_pos = col as f32 - center_x;

			// Top half of the character cell
			let y_top = row as f32 * 2.0 - center_y;
			let brightness_top = calculate_sphere_brightness(x_pos, y_top, radius);

			// Bottom half of the character cell
			let y_bottom = row as f32 * 2.0 + 1.0 - center_y;
			let brightness_bottom = calculate_sphere_brightness(x_pos, y_bottom, radius);

			match (brightness_top, brightness_bottom) {
				(Some(b_top), Some(b_bottom)) => {
					// Both halves are part of the sphere
					let (r, g, b) = get_purple_color(b_top);
					print!("\x1b[38;2;{};{};{}m", r, g, b);
					let (r, g, b) = get_purple_color(b_bottom);
					print!("\x1b[48;2;{};{};{}m", r, g, b);
					print!("▀");
					print!("\x1b[0m");
				}
				(Some(b_top), None) => {
					// Only top half is sphere
					let (r, g, b) = get_purple_color(b_top);
					print!("\x1b[38;2;{};{};{}m▀\x1b[0m", r, g, b);
				}
				(None, Some(b_bottom)) => {
					// Only bottom half is sphere
					let (r, g, b) = get_purple_color(b_bottom);
					print!("\x1b[38;2;{};{};{}m▄\x1b[0m", r, g, b);
				}
				(None, None) => {
					// Neither half is sphere
					print!(" ");
				}
			}
		}
		println!();
	}

	println!();
	println!("                         SPACEDRIVE");
	println!();
}

/// Display a compact version of the logo
pub fn print_compact_logo() {
	println!("Spacedrive CLI v2");
}
