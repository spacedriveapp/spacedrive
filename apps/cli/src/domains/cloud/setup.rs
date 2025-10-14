//! Interactive cloud storage setup

use crate::{context::Context, util::confirm::{confirm_or_abort, password, select, text}};
use anyhow::Result;
use sd_core::ops::volumes::{
	add_cloud::{CloudStorageConfig, VolumeAddCloudInput},
	list::VolumeListQueryInput,
	remove_cloud::VolumeRemoveCloudInput,
};
use sd_core::volume::backend::CloudServiceType;
use crate::util::prelude::*;

pub async fn run_interactive(ctx: &Context) -> Result<()> {
	loop {
		println!("\n╔════════════════════════════════════════╗");
		println!("║     Spacedrive Cloud Storage Setup    ║");
		println!("╚════════════════════════════════════════╝\n");

		let action = select(
			"What would you like to do?",
			&[
				"Add cloud storage".to_string(),
				"List cloud volumes".to_string(),
				"Remove cloud volume".to_string(),
				"Exit".to_string(),
			],
		)?;

		match action {
			0 => add_cloud_interactive(ctx).await?,
			1 => list_volumes(ctx).await?,
			2 => remove_volume_interactive(ctx).await?,
			3 => {
				println!("\nGoodbye! ");
				break;
			}
			_ => unreachable!(),
		}
	}

	Ok(())
}

async fn add_cloud_interactive(ctx: &Context) -> Result<()> {
	println!("\n=== Add Cloud Storage ===\n");

	// 1. Select service type (only S3-compatible for now)
	let service_idx = select(
		"Select S3-compatible storage provider",
		&[
			"Amazon S3".to_string(),
			"Cloudflare R2".to_string(),
			"Backblaze B2".to_string(),
			"Wasabi".to_string(),
			"DigitalOcean Spaces".to_string(),
			"MinIO (self-hosted)".to_string(),
			"Other S3-compatible".to_string(),
		],
	)?;

	let (service_type, needs_endpoint, default_region) = match service_idx {
		0 => (CloudServiceType::S3, false, None),
		1 => (CloudServiceType::S3, true, Some("auto".to_string())), // R2 uses "auto"
		2 => (CloudServiceType::BackblazeB2, false, None),
		3 => (CloudServiceType::Wasabi, false, None),
		4 => (CloudServiceType::DigitalOceanSpaces, false, None),
		5 => (CloudServiceType::S3, true, Some("us-east-1".to_string())), // MinIO default
		6 => (CloudServiceType::Other, true, Some("us-east-1".to_string())),
		_ => unreachable!(),
	};

	// 2. Basic configuration
	let name = text("Volume name (e.g., 'My S3 Bucket')", false)?.unwrap();

	let bucket = text("Bucket name", false)?.unwrap();

	let region = if let Some(default) = default_region {
		text(&format!("Region (default: {})", default), true)?.unwrap_or(default)
	} else {
		text("Region (e.g., us-east-1)", false)?.unwrap()
	};

	let endpoint = if needs_endpoint {
		let endpoint_hint = match service_idx {
			1 => "Cloudflare R2 account endpoint",
			5 => "MinIO server endpoint",
			_ => "Custom endpoint URL",
		};
		Some(text(endpoint_hint, false)?.unwrap())
	} else {
		None
	};

	// 3. Credentials
	println!("\nCredentials will be stored securely in your system keyring\n");

	let access_key = password("Access Key ID", false)?.unwrap();
	let secret_key = password("Secret Access Key", false)?.unwrap();

	// 4. Confirm and save
	println!("\nSummary:");
	println!("  Provider: {:?}", service_type);
	println!("  Name:     {}", name);
	println!("  Bucket:   {}", bucket);
	println!("  Region:   {}", region);
	if let Some(ref e) = endpoint {
		println!("  Endpoint: {}", e);
	}
	println!();

	confirm_or_abort("Add this cloud volume?", false)?;

	// Build and execute action
	let input = VolumeAddCloudInput {
		service: service_type,
		display_name: name.clone(),
		config: CloudStorageConfig::S3 {
			bucket,
			region,
			access_key_id: access_key,
			secret_access_key: secret_key,
			endpoint,
		},
	};

	print!("Connecting to cloud storage... ");
	std::io::Write::flush(&mut std::io::stdout())?;

	let output: sd_core::ops::volumes::add_cloud::VolumeAddCloudOutput =
		execute_action!(ctx, input);

	println!("✓");
	println!(
		"\nSuccessfully added cloud volume '{}'!",
		output.volume_name
	);
	println!("   Fingerprint: {}", output.fingerprint);
	println!("   Service:     {:?}", output.service);
	println!(
		"\nYou can now add a location with interactive mode:\n  sd location add"
	);

	std::process::exit(0);
}

async fn list_volumes(ctx: &Context) -> Result<()> {
	let volumes: sd_core::ops::volumes::list::VolumeListOutput =
		execute_query!(ctx, VolumeListQueryInput {});

	if volumes.volumes.is_empty() {
		println!("\nNo cloud volumes found.");
		println!("   Add one with the 'Add cloud storage' option.");
		return Ok(());
	}

	println!("\nCloud Volumes:\n");
	for vol in &volumes.volumes {
		println!("  • {} ({})", vol.name, vol.fingerprint.short_id());
		println!("    UUID:   {}", vol.uuid);
		println!("    Type:   {}", vol.volume_type);
		println!("    FP:     {}", vol.fingerprint);
		println!();
	}

	Ok(())
}

async fn remove_volume_interactive(ctx: &Context) -> Result<()> {
	let volumes: sd_core::ops::volumes::list::VolumeListOutput =
		execute_query!(ctx, VolumeListQueryInput {});

	if volumes.volumes.is_empty() {
		println!("\nNo cloud volumes to remove.");
		return Ok(());
	}

	println!("\n=== Remove Cloud Volume ===\n");

	let volume_choices: Vec<String> = volumes
		.volumes
		.iter()
		.map(|v| format!("{} ({})", v.name, v.fingerprint.short_id()))
		.collect();

	let volume_idx = select("Select volume to remove", &volume_choices)?;
	let selected_volume = &volumes.volumes[volume_idx];

	println!(
		"\n️  This will remove cloud volume '{}' from your library.",
		selected_volume.name
	);
	println!("   Credentials will be deleted from the keyring.");
	println!("   Any locations using this volume will be affected.");

	confirm_or_abort("Are you sure?", false)?;

	let input = VolumeRemoveCloudInput {
		fingerprint: selected_volume.fingerprint.clone(),
	};

	execute_action!(ctx, input);

	println!("\nRemoved cloud volume '{}'", selected_volume.name);

	Ok(())
}
