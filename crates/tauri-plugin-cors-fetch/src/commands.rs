// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// Source: tauri-plugin-http@2.0.0-beta.3

use std::{collections::HashMap, sync::Arc, time::Duration};

use http::{header, HeaderName, HeaderValue, Method};
use reqwest::{redirect::Policy, NoProxy, RequestBuilder};
use sd_crypto::cookie::CookieCipher;
use serde::{Deserialize, Serialize};
use tauri::command;
use tracing::{debug, error};

use crate::{Error, Result, NODE_DATA_DIR};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestConfig {
	request_id: u64,
	method: String,
	url: url::Url,
	headers: Vec<(String, String)>,
	data: Option<Vec<u8>>,
	connect_timeout: Option<u64>,
	max_redirections: Option<usize>,
	proxy: Option<Proxy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
	status: u16,
	status_text: String,
	headers: Vec<(String, String)>,
	url: String,
	body: Option<Vec<u8>>,
}

use once_cell::sync::Lazy;
use tokio::sync::oneshot;
type RequestPool = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<()>>>>;
static REQUEST_POOL: Lazy<RequestPool> =
	Lazy::new(|| Arc::new(std::sync::Mutex::new(HashMap::new())));

#[command]
pub fn cancel_cors_request(request_id: u64) {
	if let Some(tx) = REQUEST_POOL.lock().unwrap().remove(&request_id) {
		tx.send(()).ok();
	}
}

#[command]
pub async fn cors_request(request: RequestConfig) -> Result<FetchResponse> {
	let request_id = request.request_id;
	let (tx, rx) = oneshot::channel();
	REQUEST_POOL.lock().unwrap().insert(request_id, tx);
	let request_config = build_request(request)?;
	let response = get_response(request_config, rx).await;
	if !REQUEST_POOL.lock().unwrap().contains_key(&request_id) {
		return Err(Error::RequestCanceled);
	}
	REQUEST_POOL.lock().unwrap().remove(&request_id);
	response
}

pub fn build_request(request_config: RequestConfig) -> Result<RequestBuilder> {
	debug!("\n=== Starting Request Build Process ===");
	let RequestConfig {
		request_id: _,
		method,
		url,
		headers,
		data,
		connect_timeout,
		max_redirections,
		proxy,
	} = request_config;

	debug!("\nRequest Details:");
	debug!("  Method: {}", method);
	debug!("  URL: {}", url);

	let method = Method::from_bytes(method.as_bytes())?;
	debug!("\nParsed HTTP method: {}", method);

	let headers: HashMap<String, String> = HashMap::from_iter(headers);
	debug!("\nHeaders:");
	for (key, value) in &headers {
		debug!("  {} = {}", key, value);
	}

	let mut builder = reqwest::ClientBuilder::new();
	debug!("\nBuilding Client Configuration:");

	if let Some(timeout) = connect_timeout {
		debug!("  Connect Timeout: {}ms", timeout);
		builder = builder.connect_timeout(Duration::from_millis(timeout));
	}

	if let Some(max_redirections) = max_redirections {
		debug!("  Redirect Policy:");
		builder = builder.redirect(if max_redirections == 0 {
			debug!("    Redirects disabled");
			Policy::none()
		} else {
			debug!("    Max redirects: {}", max_redirections);
			Policy::limited(max_redirections)
		});
	}

	if let Some(proxy_config) = proxy {
		debug!("  Configuring proxy settings");
		builder = attach_proxy(proxy_config, builder)?;
	}

	debug!("\nFinalizing Request:");
	let client = builder.build()?;
	let mut request = client.request(method.clone(), url.clone());

	debug!("\nSetting Headers:");
	for (name, value) in &headers {
		debug!("  Adding: {} = {}", name, value);
		let name = HeaderName::from_bytes(name.as_bytes())?;
		let value = HeaderValue::from_bytes(value.as_bytes())?;
		request = request.header(name, value);
	}

	if data.is_none() && matches!(method, Method::POST | Method::PUT) {
		debug!(
			"  Adding empty content-length header for {} request",
			method
		);
		request = request.header(header::CONTENT_LENGTH, HeaderValue::from(0));
	}

	if headers.contains_key(header::RANGE.as_str()) {
		debug!("  Range header present - Setting Accept-Encoding: identity");
		request = request.header(
			header::ACCEPT_ENCODING,
			HeaderValue::from_static("identity"),
		);
	}

	if let Some(data) = data {
		debug!("\nRequest Body:");
		debug!("  Size: {} bytes", data.len());
		request = request.body(data);
	}

	debug!("\nRequest build completed successfully");
	debug!("=====================================\n");
	Ok(request)
}

pub async fn get_response(
	request: RequestBuilder,
	rx: oneshot::Receiver<()>,
) -> Result<FetchResponse> {
	debug!("\n=== Starting Response Fetch ===");

	let response_or_none = tokio::select! {
		_ = rx => {
			debug!("\nRequest cancelled by receiver");
			None
		},
		res = request.send() => {
			debug!("\nRequest sent, awaiting response");
			Some(res)
		},
	};

	if let Some(response) = response_or_none {
		debug!("\nProcessing Response:");
		match response {
			Ok(res) => {
				let status = res.status();
				debug!(
					"\nStatus: {} ({})",
					status.as_u16(),
					status.canonical_reason().unwrap_or_default()
				);

				let url = res.url().to_string();
				debug!("Final URL: {}", url);

				let mut headers = Vec::new();
				debug!("\nResponse Headers:");
				for (key, val) in res.headers().iter() {
					debug!("  {} = {:?}", key.as_str(), val);
					headers.push((
						key.as_str().into(),
						String::from_utf8(val.as_bytes().to_vec())?,
					));
				}

				// Create cookies from headers
				let mut cookie_store: HashMap<String, String> = HashMap::new();

				// Filter based on Supertokens' headers
				for (key, val) in res.headers().iter() {
					match key.as_str() {
						"front-token" => {
							if val.to_str().map(|v| v == "remove").unwrap_or(false) {
								debug!("Removing front-token header (value: remove)");
								continue;
							}
							debug!("Adding front-token header");
							cookie_store.insert(
								"front-token".to_string(),
								val.to_str().unwrap_or_default().to_string(),
							);
						}
						"st-access-token" => {
							if val.to_str().map(|v| v.is_empty()).unwrap_or(false) {
								debug!("Removing empty st-access-token header");
								continue;
							}
							debug!("Setting st-access-token cookie");
							cookie_store.insert(
								"st-access-token".to_string(),
								val.to_str().unwrap_or_default().to_string(),
							);
						}
						"st-refresh-token" => {
							if val.to_str().map(|v| v.is_empty()).unwrap_or(false) {
								debug!("Removing empty st-refresh-token header");
								continue;
							}
							debug!("Setting st-refresh-token cookie");
							cookie_store.insert(
								"st-refresh-token".to_string(),
								val.to_str().unwrap_or_default().to_string(),
							);
						}
						// "set-cookie" => {
						// 	if let Ok(cookie_str) = val.to_str() {
						// 		if let Some((name, value)) = cookie_str.split_once('=') {
						// 			if let Some(value) = value.split(';').next() {
						// 				cookie_store.insert(name.trim().to_string(), value.trim().to_string());
						// 			}
						// 		}
						// 	}
						// }
						_ => {}
					}

					debug!("  {} = {:?}", key.as_str(), val);
					headers.push((
						key.as_str().into(),
						String::from_utf8(val.as_bytes().to_vec())?,
					));
				}
				if !cookie_store.is_empty() {
					let data_dir = NODE_DATA_DIR.get().unwrap().clone();
					let data_dir = data_dir.join("spacedrive").join("dev");

					let node_config_path = data_dir.join("node_state.sdconfig");
					let node_config = std::fs::read_to_string(node_config_path).unwrap();

					let node_config: serde_json::Value =
						serde_json::from_str(&node_config).unwrap();
					let node_id = node_config["id"]["Uuid"].as_str().unwrap();
					debug!("Node ID: {:?}", node_id);

					// Create Cipher
					let key = CookieCipher::generate_key_from_string(node_id).unwrap();
					let cipher = CookieCipher::new(&key).unwrap();

					// Read .sdks file
					let sdks_path = data_dir.join(".sdks");
					let data = std::fs::read(sdks_path.clone()).unwrap();

					let data_str = String::from_utf8(data)
						.map_err(|e| {
							error!("Failed to convert data to string: {:?}", e.to_string());
						})
						.unwrap();
					let data = CookieCipher::base64_decode(&data_str)
						.map_err(|e| {
							error!("Failed to decode data: {:?}", e.to_string());
						})
						.unwrap();
					let de_data = cipher
						.decrypt(&data)
						.map_err(|e| {
							error!("Failed to decrypt data: {:?}", e.to_string());
						})
						.unwrap();
					let de_data = String::from_utf8(de_data)
						.map_err(|e| {
							error!("Failed to convert data to string: {:?}", e.to_string());
						})
						.unwrap();

					debug!("Decrypted Data: {:?}", de_data);

					debug!("\nCookies:");
					for (name, value) in &cookie_store {
						debug!("  {} = {}", name, value);
					}

					let mut de_data: Vec<String> = serde_json::from_str(&de_data).unwrap();
					for cookie in &mut de_data {
						for (name, value) in &cookie_store {
							if cookie.starts_with(name) {
								*cookie = format!("{}={};expires=Fri, 31 Dec 9999 23:59:59 GMT;path=/;samesite=lax", name, value);
							}
						}
					}

					debug!("Updated Cookies: {:?}", de_data);

					// Now, we will encrypt the de_data and save it to the .sdks file
					let de_data = serde_json::to_string(&de_data).unwrap();
					let en_data = cipher
						.encrypt(de_data.as_bytes())
						.map_err(|e| {
							error!("Failed to encrypt data: {:?}", e.to_string());
						})
						.unwrap();
					let en_data = CookieCipher::base64_encode(&en_data);

					std::fs::write(sdks_path, en_data).unwrap();
				}

				debug!("\nReading Response Body...");
				let body = res.bytes().await;
				match body {
					Ok(bytes) => {
						debug!("Body received: {} bytes", bytes.len());
						Ok(FetchResponse {
							status: status.as_u16(),
							status_text: status.canonical_reason().unwrap_or_default().to_string(),
							headers,
							url,
							body: Some(bytes.to_vec()),
						})
					}
					Err(e) => {
						error!("Failed to read body: {}", e);
						Err(Error::Network(e))
					}
				}
			}
			Err(err) => {
				error!("Network error: {}", err);
				Err(Error::Network(err))
			}
		}
	} else {
		debug!("Request was cancelled");
		Err(Error::RequestCanceled)
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
	all: Option<UrlOrConfig>,
	http: Option<UrlOrConfig>,
	https: Option<UrlOrConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum UrlOrConfig {
	Url(String),
	Config(ProxyConfig),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
	url: String,
	basic_auth: Option<BasicAuth>,
	no_proxy: Option<String>,
}

#[derive(Deserialize)]
pub struct BasicAuth {
	username: String,
	password: String,
}

#[inline]
fn proxy_creator(
	url_or_config: UrlOrConfig,
	proxy_fn: fn(String) -> reqwest::Result<reqwest::Proxy>,
) -> reqwest::Result<reqwest::Proxy> {
	match url_or_config {
		UrlOrConfig::Url(url) => Ok(proxy_fn(url)?),
		UrlOrConfig::Config(ProxyConfig {
			url,
			basic_auth,
			no_proxy,
		}) => {
			let mut proxy = proxy_fn(url)?;
			if let Some(basic_auth) = basic_auth {
				proxy = proxy.basic_auth(&basic_auth.username, &basic_auth.password);
			}
			if let Some(no_proxy) = no_proxy {
				proxy = proxy.no_proxy(NoProxy::from_string(&no_proxy));
			}
			Ok(proxy)
		}
	}
}

fn attach_proxy(
	proxy: Proxy,
	mut builder: reqwest::ClientBuilder,
) -> crate::Result<reqwest::ClientBuilder> {
	let Proxy { all, http, https } = proxy;

	if let Some(all) = all {
		let proxy = proxy_creator(all, reqwest::Proxy::all)?;
		builder = builder.proxy(proxy);
	}

	if let Some(http) = http {
		let proxy = proxy_creator(http, reqwest::Proxy::http)?;
		builder = builder.proxy(proxy);
	}

	if let Some(https) = https {
		let proxy = proxy_creator(https, reqwest::Proxy::https)?;
		builder = builder.proxy(proxy);
	}

	Ok(builder)
}
