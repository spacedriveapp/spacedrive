use crate::{
	api::utils::library,
	location::LocationPubId,
	p2p::{operations, ConnectionMethod, DiscoveryMethod, Header, P2PEvent, PeerMetadata},
};

use sd_p2p::{PeerConnectionCandidate, RemoteIdentity};

use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::PoisonError};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};
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
				file_paths: Vec<PathBuf>,
			}

			R.with2(library())
				.mutation(|(node, library), args: SpacedropCloudArgs| async move {
					debug!("spacedropCloud args: {:?}", args);
					// For each file, return a dictionary with the file path, size, name and mime type
					let files = args
						.file_paths
						.into_iter()
						.map(|path| {
							let file = std::fs::File::open(&path).unwrap();
							let metadata = file.metadata().unwrap();
							// let extension = path.extension().unwrap().to_str().unwrap();
							let name = path.file_name().unwrap().to_str().unwrap().to_string();

							(name, metadata.len(), path)
						})
						.collect::<Vec<_>>();

					let json = serde_json::json!({
						"name": files[0].0,
						"size": files[0].1,
					});

					debug!("spacedropCloud json: {:?}", json);

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

					let res_obj =
						serde_json::from_str::<serde_json::Value>(&res).map_err(|err| {
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

					let file_stream = std::fs::File::open(&files[0].2).map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error opening file: {:?}", err),
						)
					})?;

					let file_stream = tokio::fs::File::from_std(file_stream);
					let _ = reqwest::Client::new()
						.put(upload_url)
						.header("content-length", files[0].1)
						.body(reqwest::Body::wrap_stream(
							tokio_util::io::ReaderStream::new(file_stream),
						)).send().await.map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error sending request: {:?}", err),
						)
					})?;

					debug!("spacedropCloud finalize url: {}", "https://app.spacedrive.com/api/v1/spacedrop/".to_owned() + id);
					let req = reqwest::Client::new()
						.put("https://app.spacedrive.com/api/v1/spacedrop/".to_owned() + id);

					let req_with_auth = node.add_auth_header(req).await;

					let res = req_with_auth.send().await.map_err(|err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error sending request: {:?}", err),
						)
					})?;

					if res.status() != 200 {
						return Err(rspc::Error::new(
							ErrorCode::InternalServerError,
							format!("error finalizing spacedrop: {:?}", res.status()),
						));

					}

					debug!("spacedropCloud finalize response: {:?}", res);

					info!("spacedropCloud implement");

					Ok(vec!["https://app.spacedrive.com/api/v1/spacedrop/".to_owned() + id])
				})
		})
}
