use crate::transport::UnixSocketTransport;
use crate::types::*;
use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

pub struct SpacedriveClient {
    transport: UnixSocketTransport,
    library_id: Option<String>,
    http_base_url: String,
}

impl SpacedriveClient {
    pub fn new(socket_path: PathBuf, http_base_url: String) -> Self {
        Self {
            transport: UnixSocketTransport::new(socket_path),
            library_id: None,
            http_base_url,
        }
    }

    pub fn set_library(&mut self, library_id: String) {
        self.library_id = Some(library_id);
    }

    pub fn get_library_id(&self) -> Option<&str> {
        self.library_id.as_deref()
    }

    pub async fn get_http_url(&self) -> Result<String> {
        // Try to get from daemon state or Tauri IPC
        // For now, return error - will need to be implemented in daemon
        Err(anyhow::anyhow!("HTTP URL query not implemented in daemon yet"))
    }

    pub async fn execute<I, O>(&self, wire_method: &str, input: I) -> Result<O>
    where
        I: Serialize,
        O: serde::de::DeserializeOwned,
    {
        let is_query = wire_method.starts_with("query:");

        let request = if is_query {
            QueryRequest {
                method: wire_method.to_string(),
                library_id: self.library_id.clone(),
                payload: serde_json::to_value(input)?,
            }
        } else {
            QueryRequest {
                method: wire_method.to_string(),
                library_id: self.library_id.clone(),
                payload: serde_json::to_value(input)?,
            }
        };

        let request_json = if is_query {
            serde_json::json!({ "Query": request })
        } else {
            serde_json::json!({ "Action": request })
        };

        self.transport.send_request(request_json).await
    }

    pub async fn media_listing(
        &self,
        path: SdPath,
        limit: Option<usize>,
    ) -> Result<Vec<File>> {
        #[derive(Serialize)]
        struct MediaListingInput {
            path: SdPath,
            include_descendants: bool,
            media_types: Option<Vec<String>>,
            limit: Option<usize>,
            sort_by: String,
        }

        #[derive(serde::Deserialize)]
        struct MediaListingResponse {
            files: Vec<File>,
            has_more: bool,
            total_count: usize,
        }

        let input = MediaListingInput {
            path,
            include_descendants: true,
            media_types: None, // Defaults to Image + Video
            limit,
            sort_by: "datetaken".to_string(),
        };

        let response: MediaListingResponse = self.execute("query:files.media_listing", input).await.map_err(|e| {
            eprintln!("Failed to deserialize media_listing response: {}", e);
            e
        })?;
        Ok(response.files)
    }

    pub fn thumbnail_url(&self, content_uuid: &str, variant: &str, format: &str) -> String {
        format!(
            "{}/sidecar/{}/{}/thumb/{}.{}",
            self.http_base_url,
            self.library_id
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("None"),
            content_uuid,
            variant,
            format
        )
    }

    pub fn select_best_thumbnail<'a>(
        &self,
        sidecars: &'a [Sidecar],
        target_size: f32,
    ) -> Option<&'a Sidecar> {
        sidecars
            .iter()
            .filter(|s| s.kind == "thumb" && s.status == "ready")
            .min_by_key(|s| {
                let size = self.parse_variant_size(&s.variant).unwrap_or(0);
                let scale = self.parse_variant_scale(&s.variant).unwrap_or(1);

                // Prefer 1x scale unless rendering very large (> 400px)
                let preferred_size = if target_size <= 400.0 {
                    (target_size * 0.6) as i32
                } else {
                    target_size as i32
                };

                // Heavily penalize higher scales for performance
                let penalty = (scale as i32 - 1) * 100;

                (size as i32 - preferred_size).abs() + penalty
            })
    }

    fn parse_variant_size(&self, variant: &str) -> Option<u32> {
        // Parse "grid@1x" -> 256 (or similar mapping)
        // For now, return hardcoded sizes
        if variant.starts_with("icon") {
            Some(128)
        } else if variant.starts_with("grid") {
            Some(256)
        } else if variant.starts_with("detail") {
            Some(1024)
        } else {
            None
        }
    }

    fn parse_variant_scale(&self, variant: &str) -> Option<u32> {
        // Parse "grid@2x" -> 2
        variant
            .split('@')
            .nth(1)
            .and_then(|s| s.chars().next())
            .and_then(|c| c.to_digit(10))
    }
}
