use base64::decode;
use dotenv::dotenv;
use futures::StreamExt;
use quinn::{ApplicationClose, Endpoint, ServerConfig};
use sd_tunnel_utils::Message;
use std::{
	env,
	error::Error,
	io::Cursor,
	net::{Ipv4Addr, SocketAddr},
};

use tracing::{error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

// TODO: Deal with certificate renewal and allow deploying in Kubernetes (where certificate could be reviewed by anyone).
// TODO: Deal with certificate revocation.
// TODO: Do I need TLS????

#[tokio::main]
async fn main() {
	dotenv().ok();

	tracing_subscriber::registry()
		.with(fmt::layer())
		.with(
			EnvFilter::from_default_env().add_directive(
				"trace".parse().expect("Error invalid tracing directive!"),
			), // TODO
			   // .add_directive(
			   // 	"mattrax=trace"
			   // 		.parse()
			   // 		.expect("Error invalid tracing directive!"),
			   // )
			   // .add_directive(
			   // 	"quinn=info"
			   // 		.parse()
			   // 		.expect("Error invalid tracing directive!"),
			   // ),
		)
		.init();

	let certificate = match env::var("SD_ROOT_CERTIFICATE") {
		Ok(certificate) => rustls::Certificate(decode(certificate).unwrap()),
		Err(e) => {
			error!("Error: 'SD_ROOT_CERTIFICATE' env var is not set!");
			return;
		},
	};
	let priv_key = match env::var("SD_ROOT_CERTIFICATE_KEY") {
		Ok(key) => rustls::PrivateKey(decode(key).unwrap()),
		Err(e) => {
			error!("Error: 'SD_ROOT_CERTIFICATE_KEY' env var is not set!");
			return;
		},
	};
	let server_port = match env::var("SD_PORT") {
		Ok(port) => port.parse::<u16>().unwrap(),
		Err(_) => 443,
	};

	let server_config =
		ServerConfig::with_single_cert(vec![certificate], priv_key).unwrap();
	let (endpoint, mut incoming) = Endpoint::server(
		server_config,
		SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), server_port).into(),
	)
	.unwrap();
	info!("Listening on {}", endpoint.local_addr().unwrap());

	while let Some(conn) = incoming.next().await {
		info!("connection incoming");
		let fut = handle_connection(conn);
		tokio::spawn(async move {
			if let Err(e) = fut.await {
				error!("connection failed: {reason}", reason = e.to_string())
			}
		});
	}
}

async fn handle_connection(conn: quinn::Connecting) -> Result<(), Box<dyn Error>> {
	let quinn::NewConnection {
		connection,
		mut bi_streams,
		..
	} = conn.await?;

	// TODO: Do certificate authenticate with the client so we can verify it's identity.

	info!("established");

	// TODO: Apply limit to number of quic streams and number of connections from each client. -> rate limiting!

	while let Some(stream) = bi_streams.next().await {
		let stream = match stream {
			Err(quinn::ConnectionError::ApplicationClosed(ApplicationClose {
				error_code,
				reason,
			})) => {
				info!("connection closed {}, {:?}", error_code, reason);
				return Ok(());
			},
			Err(e) => {
				return Err(Box::new(e));
			},
			Ok(s) => s,
		};
		let fut = handle_request(stream);
		tokio::spawn(async move {
			if let Err(e) = fut.await {
				error!("failed: {reason}", reason = e.to_string());
			}
		});
	}

	Ok(())
}

async fn handle_request(
	(mut send, mut recv): (quinn::SendStream, quinn::RecvStream),
) -> Result<(), Box<dyn Error>> {
	info!("handling request");

	// TODO: Handle multiple messages in a single session
	// TODO: Ensure connections are closed after an inactivity timeout

	info!("A");
	let mut req = recv
		.read_chunk(64 * 1024 /* TODO: Constant */, true)
		.await?
		.unwrap();
	let mut bytes: &[u8] = &req.bytes;
	info!("B");
	let msg = match Message::read(&mut bytes)? {
		Message::ClientAnnouncement { peer_id, addresses } => {
			info!("ClientAnnouncement {} {:?}", peer_id, addresses);
			Message::ClientAnnouncementResponse
		},
		_ => unimplemented!(),
	};
	info!("C");
	send.write_all(&msg.encode()?).await?;
	info!("D");

	Ok(())
}
