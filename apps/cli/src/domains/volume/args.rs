use clap::Args;
use sd_core::{
	ops::volumes::{
		add_cloud::{CloudStorageConfig, VolumeAddCloudInput},
		remove_cloud::VolumeRemoveCloudInput,
	},
	volume::{backend::CloudServiceType, VolumeFingerprint},
};

#[derive(Args, Debug)]
pub struct VolumeAddCloudArgs {
	/// Display name for the cloud volume
	pub name: String,

	/// Cloud service type
	#[arg(long, value_enum)]
	pub service: CloudServiceArg,

	/// S3 bucket name (for S3 service)
	#[arg(long, required_if_eq("service", "s3"))]
	pub bucket: Option<String>,

	/// S3 region (for S3 service)
	#[arg(long, required_if_eq("service", "s3"))]
	pub region: Option<String>,

	/// S3 access key ID (for S3 service)
	#[arg(long, required_if_eq("service", "s3"))]
	pub access_key_id: Option<String>,

	/// S3 secret access key (for S3 service)
	#[arg(long, required_if_eq("service", "s3"))]
	pub secret_access_key: Option<String>,

	/// Custom S3 endpoint (optional, for S3-compatible services like MinIO, R2, etc.)
	#[arg(long)]
	pub endpoint: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum CloudServiceArg {
	S3,
	GoogleDrive,
	Dropbox,
	OneDrive,
	GoogleCloudStorage,
	AzureBlob,
	BackblazeB2,
	Wasabi,
	DigitalOceanSpaces,
}

impl From<CloudServiceArg> for CloudServiceType {
	fn from(arg: CloudServiceArg) -> Self {
		match arg {
			CloudServiceArg::S3 => CloudServiceType::S3,
			CloudServiceArg::GoogleDrive => CloudServiceType::GoogleDrive,
			CloudServiceArg::Dropbox => CloudServiceType::Dropbox,
			CloudServiceArg::OneDrive => CloudServiceType::OneDrive,
			CloudServiceArg::GoogleCloudStorage => CloudServiceType::GoogleCloudStorage,
			CloudServiceArg::AzureBlob => CloudServiceType::AzureBlob,
			CloudServiceArg::BackblazeB2 => CloudServiceType::BackblazeB2,
			CloudServiceArg::Wasabi => CloudServiceType::Wasabi,
			CloudServiceArg::DigitalOceanSpaces => CloudServiceType::DigitalOceanSpaces,
		}
	}
}

impl VolumeAddCloudArgs {
	pub fn validate_and_build(self) -> Result<VolumeAddCloudInput, String> {
		let service = CloudServiceType::from(self.service.clone());

		let config = match self.service {
			CloudServiceArg::S3 => {
				let bucket = self.bucket.ok_or("--bucket is required for S3")?;
				let region = self.region.ok_or("--region is required for S3")?;
				let access_key_id = self
					.access_key_id
					.ok_or("--access-key-id is required for S3")?;
				let secret_access_key = self
					.secret_access_key
					.ok_or("--secret-access-key is required for S3")?;

				CloudStorageConfig::S3 {
					bucket,
					region,
					access_key_id,
					secret_access_key,
					endpoint: self.endpoint,
				}
			}
			_ => {
				return Err(format!(
					"Service {:?} is not yet supported. Only S3 is currently available.",
					self.service
				))
			}
		};

		Ok(VolumeAddCloudInput {
			service,
			display_name: self.name,
			config,
		})
	}
}

#[derive(Args, Debug)]
pub struct VolumeRemoveCloudArgs {
	/// Volume fingerprint (from volume list)
	pub fingerprint: String,

	/// Skip confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl TryFrom<VolumeRemoveCloudArgs> for VolumeRemoveCloudInput {
	type Error = String;

	fn try_from(args: VolumeRemoveCloudArgs) -> Result<Self, String> {
		let fingerprint = VolumeFingerprint::from_string(&args.fingerprint)
			.map_err(|e| format!("Invalid fingerprint: {}", e))?;

		Ok(Self { fingerprint })
	}
}
