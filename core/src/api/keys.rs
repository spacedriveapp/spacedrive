use super::utils::library;
use super::{Ctx, SanitizedNodeConfig, R};
use once_cell::sync::Lazy;
use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_crypto::cookie::CookieCipher;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error};

static CACHE: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

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

// fn sanitize_path(base_dir: &Path, path: &Path) -> Result<PathBuf, rspc::Error> {
// 	let abs_base = base_dir.canonicalize().map_err(|e| {
// 		error!("Failed to canonicalize base directory: {:?}", e.to_string());
// 		rspc::Error::new(
// 			ErrorCode::InternalServerError,
// 			"Failed to canonicalize base directory".to_string(),
// 		)
// 	})?;
// 	let abs_path = abs_base.join(path).canonicalize().map_err(|e| {
// 		error!("Failed to canonicalize path: {:?}", e.to_string());
// 		rspc::Error::new(
// 			ErrorCode::InternalServerError,
// 			"Failed to canonicalize path".to_string(),
// 		)
// 	})?;
// 	if abs_path.starts_with(&abs_base) {
// 		Ok(abs_path)
// 	} else {
// 		error!("Path injection attempt detected: {:?}", abs_path);
// 		Err(rspc::Error::new(
// 			ErrorCode::InternalServerError,
// 			"Invalid path".to_string(),
// 		))
// 	}
// }

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	let cipher_cache = Arc::new(RwLock::new(None));

	R.router()
		.procedure("get", {
			let cipher_cache = cipher_cache.clone();
			R.query(move |node, _: ()| {
				let cipher_cache = cipher_cache.clone();
				async move {
					let cache_guard = CACHE.read().await;
					if let Some(cached_data) = cache_guard.clone().as_ref() {
						debug!("Returning cached data");
						return Ok(cached_data.clone());
					}

					let base_dir = node.config.data_directory();
					// let path = sanitize_path(&base_dir, Path::new(".sdks"))?;
					let path = base_dir.join(".sdks");
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
					let base_dir = node.config.data_directory();
					// let path = sanitize_path(&base_dir, Path::new(".sdks"))?;
					let path = base_dir.join(".sdks");

					// Read and decrypt existing data if it exists
					let existing_decrypted = if let Ok(existing_data) = read_file(&path).await {
						let cipher = get_cipher(&node, cipher_cache.clone()).await?;
						let data_str = String::from_utf8(existing_data).map_err(|e| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"Failed to convert data to string".to_string(),
							)
						})?;
						let decoded = CookieCipher::base64_decode(&data_str).map_err(|e| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"Failed to decode data".to_string(),
							)
						})?;
						let decrypted = cipher.decrypt(&decoded).map_err(|e| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"Failed to decrypt data".to_string(),
							)
						})?;
						String::from_utf8(decrypted).ok()
					} else {
						None
					};

					// Compare unencrypted data
					if let Some(existing) = existing_decrypted {
						if existing == args {
							debug!("Data unchanged, skipping write operation");
							return Ok(());
						}
					}

					// Only encrypt and write if data changed
					let cipher = get_cipher(&node, cipher_cache).await?;
					let en_data = cipher.encrypt(args.as_bytes()).map_err(|e| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to encrypt data".to_string(),
						)
					})?;

					let en_data = CookieCipher::base64_encode(&en_data);

					write_file(&path, en_data.as_bytes()).await?;
					let mut cache_guard = CACHE.write().await;
					*cache_guard = Some(args.clone());
					debug!("Written to read cache");

					debug!("Saved data to {:?}", path);
					Ok(())
				}
			})
		})
		.procedure("saveEmailAddress", {
			R.with2(library())
				.mutation(move |(node, library), args: String| async move {
					let path = node
						.libraries
						.libraries_dir
						.join(format!("{}.sdlibrary", library.id));

					let mut config = serde_json::from_slice::<Map<String, Value>>(
						&tokio::fs::read(path.clone()).await.map_err(|e| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								format!("Failed to read library config: {:?}", e.to_string()),
							)
						})?,
					)
					.map_err(|e: serde_json::Error| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("Failed to parse library config: {:?}", e.to_string()),
						)
					})?;

					// Decrypt existing email if present
					let existing_email = if let Some(encrypted) = config.get("cloud_email_address")
					{
						if let Some(encrypted_str) = encrypted.as_str() {
							let uuid_key = CookieCipher::generate_key_from_string(
								library.id.to_string().as_str(),
							)
							.map_err(|e| {
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
							let decoded =
								CookieCipher::base64_decode(encrypted_str).map_err(|e| {
									error!("Failed to decode data: {:?}", e.to_string());
									rspc::Error::new(
										ErrorCode::InternalServerError,
										"Failed to decode data".to_string(),
									)
								})?;
							let decrypted = cipher.decrypt(&decoded).map_err(|e| {
								error!("Failed to decrypt data: {:?}", e.to_string());
								rspc::Error::new(
									ErrorCode::InternalServerError,
									"Failed to decrypt data".to_string(),
								)
							})?;
							String::from_utf8(decrypted).ok()
						} else {
							None
						}
					} else {
						None
					};

					// Compare unencrypted data
					if let Some(existing) = existing_email {
						if existing == args {
							debug!("Email unchanged, skipping write operation");
							return Ok(());
						}
					}

					// Only encrypt and write if email changed
					let uuid_key =
						CookieCipher::generate_key_from_string(library.id.to_string().as_str())
							.map_err(|e| {
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
					let en_data = cipher.encrypt(args.as_bytes()).map_err(|e| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to encrypt data".to_string(),
						)
					})?;
					let en_data = CookieCipher::base64_encode(&en_data);

					config.insert("cloud_email_address".to_string(), json!(en_data));

					let config_vec = serde_json::to_vec(&config).map_err(|e| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("Failed to serialize library config: {:?}", e.to_string()),
						)
					})?;

					tokio::fs::write(path, config_vec).await.map_err(|e| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("Failed to write library config: {:?}", e.to_string()),
						)
					})?;

					Ok(())
				})
		})
		.procedure("getEmailAddress", {
			R.with2(library())
				.query(move |(node, library), _: ()| async move {
					let path = node
						.libraries
						.libraries_dir
						.join(format!("{}.sdlibrary", library.id));

					let config = serde_json::from_slice::<Map<String, Value>>(
						&tokio::fs::read(path.clone()).await.map_err(|e| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								format!("Failed to read library config: {:?}", e.to_string()),
							)
						})?,
					)
					.map_err(|e: serde_json::Error| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("Failed to parse library config: {:?}", e.to_string()),
						)
					})?;

					let en_data = config.get("cloud_email_address").ok_or_else(|| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to get cloud_email_address".to_string(),
						)
					})?;

					let en_data = en_data.as_str().ok_or_else(|| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to get cloud_email_address".to_string(),
						)
					})?;

					let en_data = CookieCipher::base64_decode(en_data).map_err(|e| {
						error!("Failed to decode data: {:?}", e.to_string());
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"Failed to decode data".to_string(),
						)
					})?;

					let uuid_key =
						CookieCipher::generate_key_from_string(library.id.to_string().as_str())
							.map_err(|e| {
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

					let de_data = cipher.decrypt(&en_data).map_err(|e| {
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
				})
		})
}
