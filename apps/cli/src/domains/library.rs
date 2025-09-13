use anyhow::Result;
use clap::Subcommand;

use crate::context::{Context, OutputFormat};
use crate::util::output::print_json;
use sd_core::ops::libraries::create;
use sd_core::ops::libraries::session::set_current;

#[derive(Subcommand, Debug)]
pub enum LibraryCmd {
	/// Create a new library
	Create { name: String },
	/// Switch to a different library
	Switch { id: uuid::Uuid },
	/// List libraries
	List,
}

pub async fn run(ctx: &Context, cmd: LibraryCmd) -> Result<()> {
	match cmd {
		LibraryCmd::Create { name } => {
			let input = create::LibraryCreateInput::new(name);
			let bytes = ctx.core.action(&input).await?;
			eprintln!("DEBUG: Received {} bytes from daemon", bytes.len());
			// Try to deserialize and provide detailed error information
			match bincode::deserialize::<create::LibraryCreateOutput>(&bytes) {
				Ok(out) => {
					println!(
						"Created library {} with ID {} at {}",
						out.name, out.library_id, out.path
					);
				}
				Err(e) => {
					eprintln!("DEBUG: Bincode deserialization failed: {}", e);
					eprintln!("DEBUG: Raw bytes length: {}", bytes.len());

					// Try to decode as much as possible manually to debug
					if bytes.len() >= 16 {
						let uuid_bytes = &bytes[0..16];
						eprintln!("DEBUG: UUID bytes: {:?}", uuid_bytes);

						if bytes.len() > 16 {
							let name_len = bytes[16] as usize;
							eprintln!("DEBUG: Name length: {}", name_len);

							if bytes.len() > 17 + name_len {
								let name_bytes = &bytes[17..17 + name_len];
								if let Ok(name) = std::str::from_utf8(name_bytes) {
									eprintln!("DEBUG: Library name: {}", name);
								}
							}
						}
					}

					return Err(anyhow::anyhow!(
						"Failed to deserialize library creation response: {}",
						e
					));
				}
			}
		}
		LibraryCmd::Switch { id } => {
			let input = set_current::SetCurrentLibraryInput { library_id: id };
			let bytes = ctx.core.action(&input).await?;
			let out: set_current::SetCurrentLibraryOutput = bincode::deserialize(&bytes)?;
			if out.success {
				println!("Switched to library {}", id);
			} else {
				println!("Failed to switch to library {}", id);
			}
		}
		LibraryCmd::List => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			match ctx.format {
				OutputFormat::Human => {
					if libs.is_empty() {
						println!("No libraries found");
					}
					for l in libs {
						println!("- {} {}", l.id, l.path.display());
					}
				}
				OutputFormat::Json => print_json(&libs),
			}
		}
	}
	Ok(())
}
