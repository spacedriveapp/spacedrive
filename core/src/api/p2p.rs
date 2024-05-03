use crate::p2p::{operations, ConnectionMethod, DiscoveryMethod, Header, P2PEvent, PeerMetadata};

use reqwest::Body;
use sd_p2p::{PeerConnectionCandidate, RemoteIdentity};

use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::PoisonError};
use tokio::{fs::File, io::AsyncWriteExt};
use tokio_util::codec::{BytesCodec, FramedRead};
use uuid::Uuid;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("events", {
			R.subscription(|node, _: ()| async move {
				let mut rx = node.p2p.events.subscribe();

				let mut queued = Vec::new();

				for (_, peer, metadata) in node.p2p.p2p.peers().iter().filter_map(|(i, p)| {
					PeerMetadata::from_hashmap(&p.metadata())
						.ok()
						.map(|m| (i, p, m))
				}) {
					queued.push(P2PEvent::PeerChange {
						identity: peer.identity(),
						connection: if peer.is_connected_with_hook(node.p2p.libraries_hook_id) {
							ConnectionMethod::Relay
						} else if peer.is_connected() {
							ConnectionMethod::Local
						} else {
							ConnectionMethod::Disconnected
						},
						discovery: match peer
							.connection_candidates()
							.contains(&PeerConnectionCandidate::Relay)
						{
							true => DiscoveryMethod::Relay,
							false => DiscoveryMethod::Local,
						},
						metadata,
					});
				}

				Ok(async_stream::stream! {
					for event in queued.drain(..queued.len()) {
						yield event;
					}

					while let Ok(event) = rx.recv().await {
						yield event;
					}
				})
			})
		})
		.procedure("state", {
			R.query(|node, _: ()| async move { Ok(node.p2p.state().await) })
		})
		.procedure("listeners", {
			#[derive(Serialize, Type)]
			#[serde(tag = "type")]
			pub enum ListenerState {
				Listening,
				Error { error: String },
				Disabled,
			}

			#[derive(Serialize, Type)]
			pub struct Listeners {
				ipv4: ListenerState,
				ipv6: ListenerState,
			}

			R.query(|node, _: ()| async move {
				let addrs = node
					.p2p
					.p2p
					.listeners()
					.iter()
					.flat_map(|l| l.addrs.clone())
					.collect::<Vec<_>>();

				let errors = node
					.p2p
					.listener_errors
					.lock()
					.unwrap_or_else(PoisonError::into_inner);

				Ok(Listeners {
					ipv4: match errors.ipv4 {
						Some(ref err) => ListenerState::Error { error: err.clone() },
						None => match addrs.iter().any(|f| f.is_ipv4()) {
							true => ListenerState::Listening,
							false => ListenerState::Disabled,
						},
					},
					ipv6: match errors.ipv6 {
						Some(ref err) => ListenerState::Error { error: err.clone() },
						None => match addrs.iter().any(|f| f.is_ipv6()) {
							true => ListenerState::Listening,
							false => ListenerState::Disabled,
						},
					},
				})
			})
		})
		.procedure("debugConnect", {
			R.mutation(|node, identity: RemoteIdentity| async move {
				let peer = { node.p2p.p2p.peers().get(&identity).cloned() };
				let mut stream = peer
					.ok_or(rspc::Error::new(
						ErrorCode::InternalServerError,
						"big man, not found".into(),
					))?
					.new_stream()
					.await
					.map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error in peer.new_stream: {:?}", err),
						)
					})?;

				stream
					.write_all(&Header::Ping.to_bytes())
					.await
					.map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error sending ping header: {:?}", err),
						)
					})?;

				Ok("connected")
			})
		})
		.procedure("spacedrop", {
			#[derive(Type, Deserialize)]
			pub struct SpacedropArgs {
				identity: RemoteIdentity,
				file_path: Vec<String>,
			}

			R.mutation(|node, args: SpacedropArgs| async move {
				operations::spacedrop(
					node.p2p.clone(),
					args.identity,
					args.file_path
						.into_iter()
						.map(PathBuf::from)
						.collect::<Vec<_>>(),
				)
				.await
				.map_err(|_err| {
					rspc::Error::new(ErrorCode::InternalServerError, "todo: error".into())
				})
			})
		})
		.procedure("acceptSpacedrop", {
			R.mutation(|node, (id, path): (Uuid, Option<String>)| async move {
				match path {
					Some(path) => node.p2p.accept_spacedrop(id, path).await,
					None => node.p2p.reject_spacedrop(id).await,
				};

				Ok(())
			})
		})
		.procedure("cancelSpacedrop", {
			R.mutation(|node, id: Uuid| async move {
				node.p2p.cancel_spacedrop(id).await;

				Ok(())
			})
		})
		.procedure("spacedropCloud", {
			#[derive(Type, Deserialize, Debug)]
			pub struct SpacedropCloudArgs {
				file_path: PathBuf,
				#[specta(optional)]
				expires: Option<chrono::DateTime<chrono::Utc>>,
				#[specta(optional)]
				password: Option<String>,
			}

			R.mutation(|node, args: SpacedropCloudArgs| async move {
				// For each file, return a dictionary with the file path, size, name and mime type
				let (name, len, path) = args
					.file_path
					.file_name()
					.and_then(|name| name.to_str())
					.map(|name| {
						let file_path = args.file_path.clone();
						(
							name.to_string(),
							file_path
								.metadata()
								.map(|metadata| metadata.len())
								.unwrap_or(0),
							file_path,
						)
					})
					.unwrap_or_else(|| ("".to_string(), 0, PathBuf::new()));

				let mut json = serde_json::json!({
					"name": name,
					"size": len,
				});

				if let Some(expires) = args.expires {
					json["expires"] = serde_json::json!(expires);
				}

				if let Some(password) = args.password {
					json["password"] = serde_json::json!(password);
				}

				let req = reqwest::Client::new()
					.post("https://app.spacedrive.com/api/v1/spacedrop")
					.json(&json);

				let req_with_auth = node.add_auth_header(req).await;

				let res_1 = req_with_auth.send().await.map_err(|err| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error sending request: {:?}", err),
					)
				})?;

				if res_1.status() != 200 {
					return Err(rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error creating spacedrop for cloud: {:?}", res_1.status()),
					));
				}

				let res = &res_1.text().await.map_err(|err| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error reading response: {:?}", err),
					)
				})?;

				let res_obj = serde_json::from_str::<serde_json::Value>(res).map_err(|err| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error parsing response: {:?}", err),
					)
				})?;
				let id = res_obj["id"].as_str().ok_or_else(|| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						"missing id in response".into(),
					)
				})?;

				let upload_url = res_obj["url"].as_str().ok_or_else(|| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						"missing url in response".into(),
					)
				})?;

				let file = File::open(path.clone()).await.map_err(|err| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error opening file: {:?}", err),
					)
				})?;

				let _ = reqwest::Client::new()
					.put(upload_url)
					.header("content-length", len)
					.body(file_to_stream(file))
					.send()
					.await
					.map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error sending request: {:?}", err),
						)
					})?;

				let req = reqwest::Client::new()
					.post("https://app.spacedrive.com/api/v1/spacedrop/".to_owned() + id);

				let req_with_auth = node.add_auth_header(req).await;

				let _ = req_with_auth.send().await.map_err(|err| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error sending request: {:?}", err),
					)
				})?;

				let download_req = reqwest::Client::new()
					.get("https://app.spacedrive.com/api/v1/spacedrop/".to_owned() + id);

				let download_req_with_auth = node.add_auth_header(download_req).await;

				let download_res = download_req_with_auth.send().await.map_err(|err| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error sending request: {:?}", err),
					)
				})?;

				let download_res = download_res.text().await.map_err(|err| {
					rspc::Error::new(
						ErrorCode::InternalServerError,
						format!("error reading response: {:?}", err),
					)
				})?;

				let res_obj =
					serde_json::from_str::<serde_json::Value>(&download_res).map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error parsing response: {:?}", err),
						)
					})?;

				let download_url = res_obj["url"]
					.as_str()
					.ok_or_else(|| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"missing url in response".into(),
						)
					})?
					.to_string();

				Ok(download_url)
			})
		})
}

fn file_to_stream(file: File) -> Body {
	let stream = FramedRead::new(file, BytesCodec::new());
	Body::wrap_stream(stream)
}
