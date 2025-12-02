use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

pub struct TcpTransport {
    socket_addr: String,
}

impl TcpTransport {
    pub fn new(socket_addr: String) -> Self {
        Self { socket_addr }
    }

    pub async fn send_request<O>(&self, request: serde_json::Value) -> Result<O>
    where
        O: DeserializeOwned,
    {
        // Connect to daemon
        let stream = TcpStream::connect(&self.socket_addr).await?;
        let (reader, mut writer) = stream.into_split();

        // Send request as newline-delimited JSON
        let request_line = format!("{}\n", serde_json::to_string(&request)?);
        eprintln!("Sending request: {}", request_line.trim());
        writer.write_all(request_line.as_bytes()).await?;
        writer.flush().await?;

        // Read response (newline-delimited)
        let mut reader = BufReader::new(reader);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        if response_line.is_empty() {
            return Err(anyhow!("Empty response from daemon"));
        }

        eprintln!("Got response: {}", response_line.trim());

        // Parse response
        let response: serde_json::Value = serde_json::from_str(&response_line)?;

        // Handle different response formats
        if let Some(json) = response.get("json") {
            eprintln!("Response has 'json' field");
            return serde_json::from_value(json.clone()).map_err(|e| {
                eprintln!("Deserialization error: {}", e);
                eprintln!("Trying to deserialize: {}", serde_json::to_string_pretty(json).unwrap_or_default());
                anyhow!("Failed to deserialize 'json' field: {}", e)
            });
        }

        if let Some(json_ok) = response.get("JsonOk") {
            eprintln!("Response has 'JsonOk' field");
            return serde_json::from_value(json_ok.clone()).map_err(|e| {
                eprintln!("Deserialization error: {}", e);
                eprintln!("Trying to deserialize (first 500 chars): {}",
                    serde_json::to_string(json_ok).unwrap_or_default().chars().take(500).collect::<String>());
                anyhow!("Failed to deserialize 'JsonOk' field: {}", e)
            });
        }

        if let Some(error) = response.get("Error").or_else(|| response.get("error")) {
            return Err(anyhow!(
                "Daemon error: {}",
                error.as_str().unwrap_or("unknown error")
            ));
        }

        eprintln!("Response doesn't match any known format, full response: {:?}", response);

        // If it's a raw value (like Pong), try to deserialize directly
        serde_json::from_value(response).map_err(|e| anyhow!("Failed to parse response: {}", e))
    }
}
