use super::{Ctx, SanitizedNodeConfig, R};
use rspc::{alpha::AlphaRouter, ErrorCode};
use tokio::io::AsyncWriteExt;
use tracing::{debug, error};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|node, _: ()| async move {
				// Get .sdks file path
				let path = node.config.data_directory().join(".sdks");

				// Open the file
				let data = tokio::fs::read(&path).await.map_err(|e| {
					error!("Failed to read file: {:?}", e);
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("Failed to read file {:?}", path),
					)
				})?;

				// Get node UUID
				let config = SanitizedNodeConfig::from(node.config.get().await);
				let uuid = config.id;

				// Convert UUID to string
				let uuid = uuid.to_string();

				// Decrypt the data
				let de_data = sd_crypto::basic::decrypt_string(&data, &uuid).map_err(|e| {
					error!("Failed to decrypt data: {:?}", e);
					rspc::Error::new(
						ErrorCode::InternalServerError,
						"Failed to decrypt data".to_string(),
					)
				})?;

				Ok(de_data)
			})
		})
		.procedure("save", {
			R.mutation(|node, args: String| async move {
				// Get node UUID
				let config = SanitizedNodeConfig::from(node.config.get().await);
				let uuid = config.id;

				// Convert UUID to string
				let uuid = uuid.to_string();

				// Encrypt the args using AES from openssl
				let en_data = sd_crypto::basic::encrypt_string(&args, &uuid).map_err(|e| {
					error!("Failed to encrypt data: {:?}", e);
					rspc::Error::new(
						ErrorCode::InternalServerError,
						"Failed to encrypt data".to_string(),
					)
				})?;

				// Get .sdks file path
				let path = node.config.data_directory().join(".sdks");

				// Open the file
				let mut file = tokio::fs::OpenOptions::new()
					.write(true)
					.create(true)
					.open(&path)
					.await
					.map_err(|e| {
						error!("Failed to open file: {:?}", e);
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("Failed to open file {:?}", path),
						)
					})?;

				// Write the encrypted data
				file.write_all(&en_data).await.map_err(|e| {
					error!("Failed to write to file: {:?}", e);
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("Failed to write to file {:?}", path),
					)
				})?;

				// Log the success
				debug!("Saved data to {:?}", path);

				Ok(())
			})
		})
}
