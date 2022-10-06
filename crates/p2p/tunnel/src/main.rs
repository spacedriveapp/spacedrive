use base64::decode;
use bb8_redis::{
	bb8::Pool,
	redis::{cmd, RedisError},
	RedisConnectionManager,
};
use dotenv::dotenv;
use futures::StreamExt;
use metrics::increment_counter;
use metrics_exporter_prometheus::PrometheusBuilder;
use quinn::{ApplicationClose, Endpoint, ServerConfig};
use rustls::Certificate;
use sd_tunnel_utils::{
	quic, ClientAnnouncementResponse, Message, MessageError, PeerId, MAX_MESSAGE_SIZE,
};
use std::{
	collections::HashMap,
	env,
	net::{Ipv4Addr, ToSocketAddrs},
	sync::Arc,
};
use thiserror::Error;

use tracing::{debug, error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() {
	dotenv().ok();

	tracing_subscriber::registry()
		.with(fmt::layer())
		.with(
			EnvFilter::from_default_env()
				.add_directive("info".parse().expect("Error invalid tracing directive!"))
				.add_directive(
					"tunnel=debug"
						.parse()
						.expect("Error invalid tracing directive!"),
				),
		)
		.init();

	let certificate = match env::var("SD_ROOT_CERTIFICATE") {
		Ok(certificate) => {
			rustls::Certificate(decode(certificate).expect("Error decoding 'SD_ROOT_CERTIFICATE'"))
		}
		Err(_) => {
			error!("Error: 'SD_ROOT_CERTIFICATE' env var is not set!");
			return;
		}
	};
	let priv_key = match env::var("SD_ROOT_CERTIFICATE_KEY") {
		Ok(key) => {
			rustls::PrivateKey(decode(key).expect("Error decoding 'SD_ROOT_CERTIFICATE_KEY'"))
		}
		Err(_) => {
			error!("Error: 'SD_ROOT_CERTIFICATE_KEY' env var is not set!");
			return;
		}
	};
	let redis_url = match env::var("SD_REDIS_URL") {
		Ok(redis_url) => redis_url,
		Err(_) => {
			error!("Error: 'SD_REDIS_URL' env var is not set!");
			return;
		}
	};
	let server_port = env::var("SD_PORT")
		.map(|port| port.parse::<u16>().unwrap_or(9000))
		.unwrap_or(9000);
	let bind_addr = env::var("SD_BIND_ADDR").unwrap_or(Ipv4Addr::UNSPECIFIED.to_string());

	let manager =
		RedisConnectionManager::new(redis_url).expect("Error creating Redis connection manager!");
	let redis_pool = Pool::builder()
		.build(manager)
		.await
		.expect("Error creating Redis pool!");

	let builder = PrometheusBuilder::new();
	builder
		.install()
		.expect("failed to install recorder/exporter");

	let addr = format!("{}:{}", bind_addr, server_port)
		.to_socket_addrs()
		.expect("Error looking up bind address")
		.into_iter()
		.next()
		.expect("Error no bind addresses were found");
	let server_config = ServerConfig::with_crypto(Arc::new(
		quic::server_config(vec![certificate], priv_key)
			.expect("Error initialising 'ServerConfig'!"),
	));
	let (endpoint, mut incoming) =
		Endpoint::server(server_config, addr).expect("Error creating endpoint!");
	info!(
		"Listening on {}",
		endpoint.local_addr().expect("Error passing local address!")
	);

	while let Some(conn) = incoming.next().await {
		let remote_addr = conn.remote_address();
		debug!("accepted connection from '{}'", remote_addr);
		increment_counter!("spacetunnel_connections_accepted");

		let fut = handle_connection(redis_pool.clone(), conn);
		tokio::spawn(async move {
			if let Err(e) = fut.await {
				error!(
					"'handle_connection' from remote '{}' threw error: {}",
					remote_addr,
					e.to_string()
				);
				increment_counter!("spacetunnel_connections_errored");
			} else {
				debug!("closed connection from '{}'", remote_addr);
			}
		});
	}
}

async fn handle_connection(
	redis_pool: Pool<RedisConnectionManager>,
	conn: quinn::Connecting,
) -> Result<(), ConnectionError> {
	let quinn::NewConnection {
		connection,
		mut bi_streams,
		..
	} = conn.await?;

	let peer_id = match connection
		.peer_identity()
		.unwrap()
		.downcast::<Vec<Certificate>>()
	{
		Ok(certs) if certs.len() == 1 => PeerId::from_cert(&certs[0]),
		Ok(_) => {
			error!("Error: peer has multiple client certificates!");
			increment_counter!("spacetunnel_connections_invalid");
			return Ok(());
		}
		Err(_) => {
			error!("Error: peer did not provide a client certificates!");
			increment_counter!("spacetunnel_connections_invalid");
			return Ok(());
		}
	};
	info!(
		"established connection with peer '{}' from addr '{}'",
		peer_id,
		connection.remote_address()
	);

	// TODO: Ensure connections are closed automatically after an inactivity timeout
	// TODO: Ensure streams are closed automatically after an inactivity timeout

	let peer_id = &peer_id;
	while let Some(stream) = bi_streams.next().await {
		let stream = match stream {
			Err(quinn::ConnectionError::ApplicationClosed(ApplicationClose {
				error_code,
				reason,
			})) => {
				debug!(
					"closed connection with peer '{}' with error_code '{}' and reason '{:?}' ",
					peer_id, error_code, reason
				);
				return Ok(());
			}
			Err(e) => return Err(e.into()),
			Ok(s) => s,
		};

		debug!("accepted stream from peer '{}'", peer_id);
		increment_counter!("spacetunnel_streams_accepted");

		let peer_id = peer_id.clone();
		let redis_pool = redis_pool.clone();
		tokio::spawn(async move {
			let (mut tx, mut rx) = stream;
			let fut = handle_stream(redis_pool, &peer_id, (&mut tx, &mut rx));
			if let Err(err) = fut.await {
				error!("'handle_stream' threw error: {}", err.to_string());
				if matches!(err, ConnectionError::RedisErr(_)) {
					increment_counter!("spacetunnel_redis_error", "error_src" => "handle_stream");
				} else {
					increment_counter!("spacetunnel_stream_errored");
				}
				match Message::Error(MessageError::InternalServerErr).encode() {
					Ok(msg) => {
						let _ = tx.write_all(&msg).await;
					}
					Err(e) => {
						error!("Error encoding error error message: {}", e.to_string());
						increment_counter!("spacetunnel_stream_errored");
					}
				}
			} else {
				debug!("closed stream from peer '{}'", peer_id);
			}
		});
	}

	Ok(())
}

async fn handle_stream(
	redis_pool: Pool<RedisConnectionManager>,
	authenticated_peer_id: &PeerId,
	(send, recv): (&mut quinn::SendStream, &mut quinn::RecvStream),
) -> Result<(), ConnectionError> {
	let mut redis = match redis_pool.get().await {
		Ok(conn) => conn,
		Err(err) => {
			error!("Error getting Redis connection: {}", err);
			increment_counter!("spacetunnel_redis_error", "error_src" => "get");
			return Ok(());
		}
	};

	while let Some(chunk) = recv.read_chunk(MAX_MESSAGE_SIZE, true).await? {
		let mut bytes: &[u8] = &chunk.bytes;
		let msg = match Message::read(&mut bytes)? {
			Message::ClientAnnouncement { peer_id, addresses } => {
				if authenticated_peer_id != peer_id {
					Message::Error(MessageError::InvalidAuthErr)
				} else {
					increment_counter!("spacetunnel_discovery_announcements");
					let redis_key = format!("peer:announcement:{}", peer_id.to_string());
					cmd("HSET")
						.arg(&redis_key)
						.arg("addresses")
						.arg(addresses.join(","))
						.query_async(&mut *redis)
						.await?;
					cmd("EXPIRE")
						.arg(&redis_key)
						.arg(60 * 60u32 /* 1 Hour in seconds */)
						.query_async(&mut *redis)
						.await?;

					Message::ClientAnnouncementOk
				}
			}
			Message::QueryClientAnnouncement(peer_ids) => {
				increment_counter!("spacetunnel_discovery_announcement_queries");

				// TODO: Rate limit number queries that can come from each each IP
				// TODO: Check if peer is authorised to query this announcement. Syncthing don't do an auth check so for now it's fine being unauthorised.

				if peer_ids.len() > 15 {
					error!(
						"Client requested too many client announcements '{}'",
						peer_ids.len()
					);
					increment_counter!("spacetunnel_discovery_announcement_queries_invalid");
					Message::Error(MessageError::InvalidReqErr)
				} else {
					let mut peers = Vec::with_capacity(peer_ids.len());
					for peer_id in peer_ids.iter() {
						let redis_key = format!("peer:announcement:{}", peer_id.to_string());

						let resp: HashMap<String, String> = cmd("HGETALL")
							.arg(&redis_key)
							.query_async(&mut *redis)
							.await?;

						peers.push(ClientAnnouncementResponse {
							peer_id: peer_id.clone(),
							addresses: resp
								.get("addresses")
								.unwrap_or(&"".to_string())
								.split(",")
								.map(|v| v.to_string())
								.collect(),
						})
					}
					Message::QueryClientAnnouncementResponse(peers)
				}
			}
			Message::ClientAnnouncementOk
			| Message::QueryClientAnnouncementResponse { .. }
			| Message::Error(_) => Message::Error(MessageError::InvalidReqErr),
		};
		send.write_all(&msg.encode()?).await?;
	}

	Ok(())
}

#[derive(Error, Debug)]
pub enum ConnectionError {
	#[error("connection error: {0}")]
	ConnectionErr(#[from] quinn::ConnectionError),
	#[error("redis error: {0}")]
	RedisErr(#[from] RedisError),
}
