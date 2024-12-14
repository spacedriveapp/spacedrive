use super::{Ctx, SanitizedNodeConfig, R};
use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_crypto::cookie::CookieCipher;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error};

#[derive(Clone)]
struct CipherCache {
	uuid: String,
	cipher: CookieCipher,
}

async fn get_cipher(
	node: &Ctx,
	cache: Arc<RwLock<Option<CipherCache>>>,
) -> Result<CookieCipher, rspc::Error> {
	let config = SanitizedNodeConfig::from(node.config.get().await);
	let uuid = config.id.to_string();

	{
		let cache_read = cache.read().await;
		if let Some(ref cache) = *cache_read {
			if cache.uuid == uuid {
				return Ok(cache.cipher.clone());
			}
		}
	}

	let uuid_key = CookieCipher::generate_key_from_string(&uuid).map_err(|e| {
		error!("Failed to generate key: {:?}", e.to_string());
		rspc::Error::new(
			ErrorCode::InternalServerError,
			"Failed to generate key".to_string(),
		)
	})?;

	let cipher = CookieCipher::new(&uuid_key).map_err(|e| {
		error!("Failed to create cipher: {:?}", e.to_string());
		rspc::Error::new(
			ErrorCode::InternalServerError,
			"Failed to create cipher".to_string(),
		)
	})?;

	{
		let mut cache_write = cache.write().await;
		*cache_write = Some(CipherCache {
			uuid,
			cipher: cipher.clone(),
		});
	}

	Ok(cipher)
}

async fn read_file(path: &Path) -> Result<Vec<u8>, rspc::Error> {
	tokio::fs::read(path).await.map_err(|e| {
		error!("Failed to read file: {:?}", e.to_string());
		rspc::Error::new(
			ErrorCode::InternalServerError,
			format!("Failed to read file {:?}", path),
		)
	})
}

async fn write_file(path: &Path, data: &[u8]) -> Result<(), rspc::Error> {
	let mut file = tokio::fs::OpenOptions::new()
		.write(true)
		.create(true)
		.truncate(true)
		.open(path)
		.await
		.map_err(|e| {
			error!("Failed to open file: {:?}", e.to_string());
			rspc::Error::new(
				ErrorCode::InternalServerError,
				format!("Failed to open file {:?}", path),
			)
		})?;
	file.write_all(data).await.map_err(|e| {
		error!("Failed to write to file: {:?}", e.to_string());
		rspc::Error::new(
			ErrorCode::InternalServerError,
			format!("Failed to write to file {:?}", path),
		)
	})
}

fn sanitize_path(base_dir: &Path, path: &Path) -> Result<PathBuf, rspc::Error> {
	let abs_base = base_dir.canonicalize().map_err(|e| {
		error!("Failed to canonicalize base directory: {:?}", e.to_string());
		rspc::Error::new(
			ErrorCode::InternalServerError,
			"Failed to canonicalize base directory".to_string(),
		)
	})?;
	let abs_path = abs_base.join(path).canonicalize().map_err(|e| {
		error!("Failed to canonicalize path: {:?}", e.to_string());
		rspc::Error::new(
			ErrorCode::InternalServerError,
			"Failed to canonicalize path".to_string(),
		)
	})?;
	if abs_path.starts_with(&abs_base) {
		Ok(abs_path)
	} else {
		error!("Path injection attempt detected: {:?}", abs_path);
		Err(rspc::Error::new(
			ErrorCode::InternalServerError,
			"Invalid path".to_string(),
		))
	}
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	let cipher_cache = Arc::new(RwLock::new(None));

	R.router()
		.procedure("get", {
			let cipher_cache = cipher_cache.clone();
			R.query(move |node, _: ()| {
				let cipher_cache = cipher_cache.clone();
				async move {
					let base_dir = node.config.data_directory();
					let path = sanitize_path(&base_dir, Path::new(".sdks"))?;
					let data = read_file(&path).await?;
					let cipher = get_cipher(&node, cipher_cache).await?;

					let data_str = String::from_utf8(data).map_err(|e| {
						error!("Failed to convert data to string: {:?}", e.to_string());
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to convert data to string".to_string(),
						)
					})?;
					let data = CookieCipher::base64_decode(&data_str).map_err(|e| {
						error!("Failed to decode data: {:?}", e.to_string());
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to decode data".to_string(),
						)
					})?;
					let de_data = cipher.decrypt(&data).map_err(|e| {
						error!("Failed to decrypt data: {:?}", e.to_string());
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to decrypt data".to_string(),
						)
					})?;
					let de_data = String::from_utf8(de_data).map_err(|e| {
						error!("Failed to convert data to string: {:?}", e.to_string());
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to convert data to string".to_string(),
						)
					})?;
					Ok(de_data)
				}
			})
		})
		.procedure("save", {
			let cipher_cache = cipher_cache.clone();
			R.mutation(move |node, args: String| {
				let cipher_cache = cipher_cache.clone();
				async move {
					let cipher = get_cipher(&node, cipher_cache).await?;
					let en_data = cipher.encrypt(args.as_bytes()).map_err(|e| {
						error!("Failed to encrypt data: {:?}", e.to_string());
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to encrypt data".to_string(),
						)
					})?;
					let en_data = CookieCipher::base64_encode(&en_data);
					let base_dir = node.config.data_directory();
					let path = sanitize_path(&base_dir, Path::new(".sdks"))?;
					write_file(&path, en_data.as_bytes()).await?;
					debug!("Saved data to {:?}", path);
					Ok(())
				}
			})
		})
}
