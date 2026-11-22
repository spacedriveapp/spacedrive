//! Bring-your-own-key image captioning through an OpenAI-compatible API.
//!
//! One client covers every service that speaks the chat completions format
//! with image inputs, including local servers. It is used only when a library
//! has turned `ai.cloud_caption` on and stored a key; the key is encrypted with
//! the library key in the library database through `CloudCredentialManager`,
//! like cloud volume credentials, and is never logged or returned.

use std::io::Cursor;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use image::{imageops::FilterType, ImageFormat};

use super::registry::Captioner;
use super::{image as img, AiError};
use crate::crypto::cloud_credentials::{CloudCredential, CloudCredentialManager, CredentialData};
use crate::library::Library;
use crate::volume::CloudServiceType;

/// The credential slot the caption key is stored under.
const KEY_SLOT: &str = "ai:image_caption";
/// Images are downscaled before upload: captions need no more, and it keeps
/// what leaves the device small.
const UPLOAD_SIDE: u32 = 1024;
const PROMPT: &str =
	"Describe this image in one or two plain sentences. Name the setting, the main subjects and what they are doing.";

pub struct OpenAiCompatible {
	client: reqwest::Client,
	base_url: String,
	model: String,
	key: String,
}

fn credentials(library: &Library) -> CloudCredentialManager {
	CloudCredentialManager::new(
		library.core_context().key_manager.clone(),
		library.db().clone(),
		library.id(),
	)
}

pub async fn store_key(library: &Library, key: String) -> Result<(), AiError> {
	let credential = CloudCredential::new_api_key(CloudServiceType::Other, key);
	credentials(library)
		.store_credential(library.id(), KEY_SLOT, &credential)
		.await
		.map_err(|e| AiError::Cloud(e.to_string()))
}

pub async fn clear_key(library: &Library) -> Result<(), AiError> {
	let manager = credentials(library);
	tokio::task::block_in_place(|| manager.delete_credential(library.id(), KEY_SLOT))
		.map_err(|e| AiError::Cloud(e.to_string()))
}

pub async fn has_key(library: &Library) -> bool {
	load_key(library).await.is_some()
}

async fn load_key(library: &Library) -> Option<String> {
	let credential = credentials(library)
		.get_credential(library.id(), KEY_SLOT)
		.await
		.ok()?;
	match credential.data {
		CredentialData::ApiKey(key) => Some(key),
		_ => None,
	}
}

/// The library's captioner, when it has opted in and is fully configured.
pub async fn captioner(library: &Library) -> Result<Option<OpenAiCompatible>, AiError> {
	let settings = library.config().await.settings.ai.cloud_caption;
	if !settings.enabled {
		return Ok(None);
	}
	let (Some(base_url), Some(model)) = (settings.base_url, settings.model) else {
		return Ok(None);
	};
	let Some(key) = load_key(library).await else {
		return Ok(None);
	};
	let client = reqwest::Client::builder()
		.timeout(Duration::from_secs(60))
		.build()
		.map_err(|e| AiError::Cloud(e.to_string()))?;
	Ok(Some(OpenAiCompatible {
		client,
		base_url: base_url.trim_end_matches('/').to_string(),
		model,
		key,
	}))
}

#[async_trait]
impl Captioner for OpenAiCompatible {
	async fn caption(&self, image: &[u8]) -> Result<String, AiError> {
		let bytes = image.to_vec();
		let jpeg = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, AiError> {
			let rgb = img::decode(&bytes, UPLOAD_SIDE)?;
			let rgb = if rgb.width().max(rgb.height()) > UPLOAD_SIDE {
				image::imageops::resize(&rgb, UPLOAD_SIDE, UPLOAD_SIDE, FilterType::Triangle)
			} else {
				rgb
			};
			let mut out = Cursor::new(Vec::new());
			rgb.write_to(&mut out, ImageFormat::Jpeg)
				.map_err(|e| AiError::Input(e.to_string()))?;
			Ok(out.into_inner())
		})
		.await
		.map_err(|e| AiError::Model(e.to_string()))??;

		let data_url = format!(
			"data:image/jpeg;base64,{}",
			base64::engine::general_purpose::STANDARD.encode(jpeg)
		);
		let body = serde_json::json!({
			"model": self.model,
			"max_tokens": 120,
			"messages": [{
				"role": "user",
				"content": [
					{ "type": "text", "text": PROMPT },
					{ "type": "image_url", "image_url": { "url": data_url } },
				],
			}],
		});
		let response = self
			.client
			.post(format!("{}/chat/completions", self.base_url))
			.bearer_auth(&self.key)
			.header(reqwest::header::CONTENT_TYPE, "application/json")
			.body(body.to_string())
			.send()
			.await
			.map_err(|e| AiError::Cloud(e.without_url().to_string()))?;
		let status = response.status();
		let bytes = response
			.bytes()
			.await
			.map_err(|e| AiError::Cloud(format!("{status}: {}", e.without_url())))?;
		let reply: serde_json::Value = serde_json::from_slice(&bytes)
			.map_err(|_| AiError::Cloud(format!("{status}: reply was not JSON")))?;
		if !status.is_success() {
			let message = reply["error"]["message"]
				.as_str()
				.unwrap_or("request refused");
			return Err(AiError::Cloud(format!("{status}: {message}")));
		}
		reply["choices"][0]["message"]["content"]
			.as_str()
			.map(|s| s.trim().to_string())
			.filter(|s| !s.is_empty())
			.ok_or_else(|| AiError::Cloud("reply carried no caption".into()))
	}
}
