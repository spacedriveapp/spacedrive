//! Interactive cloud storage setup

use crate::util::prelude::*;
use crate::{
	context::Context,
	util::confirm::{confirm_or_abort, password, select, text},
};
use anyhow::Result;
use sd_core::ops::volumes::{
	add_cloud::{CloudStorageConfig, VolumeAddCloudInput},
	list::VolumeListQueryInput,
	remove_cloud::VolumeRemoveCloudInput,
};
use sd_core::volume::backend::CloudServiceType;

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

	let service_category_idx = select(
		"Select cloud storage category",
		&[
			"S3-compatible (Amazon, R2, B2, MinIO, etc.)".to_string(),
			"Google Drive".to_string(),
			"Microsoft OneDrive".to_string(),
			"Dropbox".to_string(),
			"Azure Blob Storage".to_string(),
			"Google Cloud Storage".to_string(),
		],
	)?;

	match service_category_idx {
		0 => add_s3_interactive(ctx).await,
		1 => add_google_drive_interactive(ctx).await,
		2 => add_onedrive_interactive(ctx).await,
		3 => add_dropbox_interactive(ctx).await,
		4 => add_azure_blob_interactive(ctx).await,
		5 => add_gcs_interactive(ctx).await,
		_ => unreachable!(),
	}
}

async fn add_s3_interactive(ctx: &Context) -> Result<()> {
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
		1 => (CloudServiceType::S3, true, Some("auto".to_string())),
		2 => (CloudServiceType::BackblazeB2, false, None),
		3 => (CloudServiceType::Wasabi, false, None),
		4 => (CloudServiceType::DigitalOceanSpaces, false, None),
		5 => (CloudServiceType::S3, true, Some("us-east-1".to_string())),
		6 => (CloudServiceType::Other, true, Some("us-east-1".to_string())),
		_ => unreachable!(),
	};

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

	println!("\nCredentials will be stored securely in your system keyring\n");

	let access_key = password("Access Key ID", false)?.unwrap();
	let secret_key = password("Secret Access Key", false)?.unwrap();

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

	execute_add_cloud(ctx, input).await
}

async fn add_google_drive_interactive(ctx: &Context) -> Result<()> {
	let name = text("Volume name (e.g., 'My Google Drive')", false)?.unwrap();
	let root = text("Root folder ID (leave empty for entire drive)", true)?;

	println!("\nOAuth Setup:");
	println!("  You'll need OAuth credentials from Google Cloud Console");
	println!("  Visit: https://console.cloud.google.com/apis/credentials\n");

	let client_id = text("OAuth Client ID", false)?.unwrap();
	let client_secret = password("OAuth Client Secret", false)?.unwrap();

	println!("\nAfter authorizing, you'll receive tokens:");
	let access_token = password("Access Token", false)?.unwrap();
	let refresh_token = password("Refresh Token", false)?.unwrap();

	println!("\nSummary:");
	println!("  Provider: Google Drive");
	println!("  Name:     {}", name);
	if let Some(ref r) = root {
		println!("  Root:     {}", r);
	}
	println!();

	confirm_or_abort("Add this cloud volume?", false)?;

	let input = VolumeAddCloudInput {
		service: CloudServiceType::GoogleDrive,
		display_name: name.clone(),
		config: CloudStorageConfig::GoogleDrive {
			root,
			access_token,
			refresh_token,
			client_id,
			client_secret,
		},
	};

	execute_add_cloud(ctx, input).await
}

async fn add_onedrive_interactive(ctx: &Context) -> Result<()> {
	let name = text("Volume name (e.g., 'My OneDrive')", false)?.unwrap();
	let root = text("Root folder path (leave empty for entire drive)", true)?;

	println!("\nOAuth Setup:");
	println!("  You'll need OAuth credentials from Azure Portal");
	println!("  Visit: https://portal.azure.com/#blade/Microsoft_AAD_RegisteredApps\n");

	let client_id = text("Application (client) ID", false)?.unwrap();
	let client_secret = password("Client Secret", false)?.unwrap();

	println!("\nAfter authorizing, you'll receive tokens:");
	let access_token = password("Access Token", false)?.unwrap();
	let refresh_token = password("Refresh Token", false)?.unwrap();

	println!("\nSummary:");
	println!("  Provider: Microsoft OneDrive");
	println!("  Name:     {}", name);
	if let Some(ref r) = root {
		println!("  Root:     {}", r);
	}
	println!();

	confirm_or_abort("Add this cloud volume?", false)?;

	let input = VolumeAddCloudInput {
		service: CloudServiceType::OneDrive,
		display_name: name.clone(),
		config: CloudStorageConfig::OneDrive {
			root,
			access_token,
			refresh_token,
			client_id,
			client_secret,
		},
	};

	execute_add_cloud(ctx, input).await
}

async fn add_dropbox_interactive(ctx: &Context) -> Result<()> {
	let name = text("Volume name (e.g., 'My Dropbox')", false)?.unwrap();
	let root = text("Root folder path (leave empty for entire Dropbox)", true)?;

	println!("\nOAuth Setup:");
	println!("  You'll need OAuth credentials from Dropbox App Console");
	println!("  Visit: https://www.dropbox.com/developers/apps\n");

	let client_id = text("App Key (Client ID)", false)?.unwrap();
	let client_secret = password("App Secret (Client Secret)", false)?.unwrap();

	println!("\nAfter authorizing, you'll receive tokens:");
	let access_token = password("Access Token", false)?.unwrap();
	let refresh_token = password("Refresh Token", false)?.unwrap();

	println!("\nSummary:");
	println!("  Provider: Dropbox");
	println!("  Name:     {}", name);
	if let Some(ref r) = root {
		println!("  Root:     {}", r);
	}
	println!();

	confirm_or_abort("Add this cloud volume?", false)?;

	let input = VolumeAddCloudInput {
		service: CloudServiceType::Dropbox,
		display_name: name.clone(),
		config: CloudStorageConfig::Dropbox {
			root,
			access_token,
			refresh_token,
			client_id,
			client_secret,
		},
	};

	execute_add_cloud(ctx, input).await
}

async fn add_azure_blob_interactive(ctx: &Context) -> Result<()> {
	let name = text("Volume name (e.g., 'My Azure Storage')", false)?.unwrap();
	let container = text("Container name", false)?.unwrap();
	let account_name = text("Storage account name", false)?.unwrap();
	let endpoint = text("Custom endpoint (leave empty for default)", true)?;

	println!("\nCredentials will be stored securely in your system keyring\n");
	let account_key = password("Storage account key", false)?.unwrap();

	println!("\nSummary:");
	println!("  Provider: Azure Blob Storage");
	println!("  Name:      {}", name);
	println!("  Container: {}", container);
	println!("  Account:   {}", account_name);
	if let Some(ref e) = endpoint {
		println!("  Endpoint:  {}", e);
	}
	println!();

	confirm_or_abort("Add this cloud volume?", false)?;

	let input = VolumeAddCloudInput {
		service: CloudServiceType::AzureBlob,
		display_name: name.clone(),
		config: CloudStorageConfig::AzureBlob {
			container,
			endpoint,
			account_name,
			account_key,
		},
	};

	execute_add_cloud(ctx, input).await
}

async fn add_gcs_interactive(ctx: &Context) -> Result<()> {
	let name = text("Volume name (e.g., 'My GCS Bucket')", false)?.unwrap();
	let bucket = text("Bucket name", false)?.unwrap();
	let root = text("Root path (leave empty for entire bucket)", true)?;
	let endpoint = text("Custom endpoint (leave empty for default)", true)?;

	println!("\nService Account Setup:");
	println!("  You'll need a service account JSON key from Google Cloud Console");
	println!("  Visit: https://console.cloud.google.com/iam-admin/serviceaccounts\n");

	let service_account_path = text("Path to service account JSON file", false)?.unwrap();
	let credential = std::fs::read_to_string(&service_account_path)
		.map_err(|e| anyhow::anyhow!("Failed to read service account file: {}", e))?;

	println!("\nSummary:");
	println!("  Provider: Google Cloud Storage");
	println!("  Name:     {}", name);
	println!("  Bucket:   {}", bucket);
	if let Some(ref r) = root {
		println!("  Root:     {}", r);
	}
	if let Some(ref e) = endpoint {
		println!("  Endpoint: {}", e);
	}
	println!();

	confirm_or_abort("Add this cloud volume?", false)?;

	let input = VolumeAddCloudInput {
		service: CloudServiceType::GoogleCloudStorage,
		display_name: name.clone(),
		config: CloudStorageConfig::GoogleCloudStorage {
			bucket,
			root,
			endpoint,
			credential,
		},
	};

	execute_add_cloud(ctx, input).await
}

async fn execute_add_cloud(
	ctx: &Context,
	input: VolumeAddCloudInput,
) -> Result<()> {
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
	println!("\nYou can now add a location with interactive mode:\n  sd location add");

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

	let _: () = execute_action!(ctx, input);

	println!("\nRemoved cloud volume '{}'", selected_volume.name);

	Ok(())
}
