use std::{fs, path::Path};

use base64::encode;
use rcgen::generate_simple_self_signed;

fn main() {
	println!("Issuing sdtunnel certificate...");

	let env_file = Path::new("./.env");
	if env_file.exists() {
		println!("File '{}' already exists. Exiting...", env_file.display());
		return;
	}

	// TODO: Replace 'generate_simple_self_signed' with full code so we have full control over generated certificate.
	let cert = generate_simple_self_signed(vec!["sdtunnel.spacedrive.com".into()]).unwrap();

	match fs::write(
		env_file,
		format!(
			r#"SD_ROOT_CERTIFICATE={}
SD_ROOT_CERTIFICATE_KEY={}
SD_REDIS_URL=redis://127.0.0.1/"#,
			encode(cert.serialize_der().unwrap()),
			encode(cert.serialize_private_key_der())
		),
	) {
		Ok(_) => {}
		Err(err) => println!("Error writing to '{}': {}", env_file.display(), err),
	}

	println!("Generate keypair!");
}
