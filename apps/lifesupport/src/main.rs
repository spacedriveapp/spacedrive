use base64::decode;
use dotenv::dotenv;
use futures::StreamExt;
use quinn::{ApplicationClose, Endpoint, ServerConfig};
use std::{
	env,
	error::Error,
	net::{Ipv4Addr, SocketAddr},
	path::Path,
};

use lifesupport::service_capnp::{
	client_announcement,
	discovery_system::{
		self, PublishAnnouncementParams, PublishAnnouncementResults,
		QueryAnnouncementParams, QueryAnnouncementResults,
	},
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
			), // .add_directive(
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
				// panic!("TODO")
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

	// TODO: Apply limit to number of quic streams and number of connections from each client. -> rate limitting!

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
		tokio::spawn(
			async move {
				if let Err(e) = fut.await {
					error!("failed: {reason}", reason = e.to_string());
				}
			}, // .instrument(info_span!("request")),
		);
	}

	Ok(())

	// let span = info_span!(
	// 	"connection",
	// 	remote = %connection.remote_address(),
	// 	protocol = %connection
	// 		.handshake_data()
	// 		.unwrap()
	// 		.downcast::<quinn::crypto::rustls::HandshakeData>().unwrap()
	// 		.protocol
	// 		.map_or_else(|| "<none>".into(), |x| String::from_utf8_lossy(&x).into_owned())
	// );
	// async {
	// 	info!("established");

	// 	// Each stream initiated by the client constitutes a new request.
	// 	while let Some(stream) = bi_streams.next().await {
	// 		let stream = match stream {
	// 			Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
	// 				info!("connection closed");
	// 				return Ok(());
	// 			},
	// 			Err(e) => {
	// 				return Err(e);
	// 			},
	// 			Ok(s) => s,
	// 		};
	// 		let fut = handle_request(root.clone(), stream);
	// 		tokio::spawn(
	// 			async move {
	// 				if let Err(e) = fut.await {
	// 					error!("failed: {reason}", reason = e.to_string());
	// 				}
	// 			}
	// 			.instrument(info_span!("request")),
	// 		);
	// 	}
	// 	Ok(())
	// }
	// .instrument(span)
	// .await?;
}

async fn handle_request(
	(mut send, recv): (quinn::SendStream, quinn::RecvStream),
) -> Result<(), Box<dyn Error>> {
	info!("bruh");

	let req = recv.read_to_end(64 * 1024).await?;

	info!("{:?}", req);

	Ok(())
	// let req = recv
	//     .read_to_end(64 * 1024)
	//     .await
	//     .map_err(|e| anyhow!("failed reading request: {}", e))?;
	// let mut escaped = String::new();
	// for &x in &req[..] {
	//     let part = ascii::escape_default(x).collect::<Vec<_>>();
	//     escaped.push_str(str::from_utf8(&part).unwrap());
	// }
	// info!(content = %escaped);
	// // Execute the request
	// let resp = process_get(&root, &req).unwrap_or_else(|e| {
	//     error!("failed: {}", e);
	//     format!("failed to process request: {}\n", e).into_bytes()
	// });
	// // Write the response
	// send.write_all(&resp)
	//     .await
	//     .map_err(|e| anyhow!("failed to send response: {}", e))?;
	// // Gracefully terminate the stream
	// send.finish()
	//     .await
	//     .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;
	// info!("complete");
	// Ok(())
}
