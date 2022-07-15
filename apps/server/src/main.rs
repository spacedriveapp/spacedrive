use sdcore::{ClientCommand, ClientQuery, CoreEvent, CoreResponse, Node, NodeController};
use std::{
	collections::HashSet,
	env,
	path::Path,
	sync::{Arc, RwLock},
	time::{Duration, Instant},
};

use actix::{
	Actor, ActorContext, Addr, AsyncContext, Context, ContextFutureSpawner, Handler,
	Message, StreamHandler, WrapFuture,
};
use actix_web::{
	get,
	http::{
		header::{HeaderName, HeaderValue},
		StatusCode,
	},
	web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};

use tokio::sync::mpsc;

const DATA_DIR_ENV_VAR: &'static str = "DATA_DIR";

#[derive(Serialize)]
pub struct Event(CoreEvent);

impl Message for Event {
	type Result = ();
}

struct EventServer {
	clients: Arc<RwLock<HashSet<Addr<Socket>>>>,
}

impl Actor for EventServer {
	type Context = Context<Self>;
}

impl EventServer {
	pub fn listen(mut event_receiver: mpsc::Receiver<CoreEvent>) -> Addr<Self> {
		let server = Self {
			clients: Arc::new(RwLock::new(HashSet::new())),
		};
		let clients = server.clients.clone();
		tokio::spawn(async move {
			let mut last = Instant::now();
			while let Some(event) = event_receiver.recv().await {
				match event {
					CoreEvent::InvalidateQueryDebounced(_) => {
						let current = Instant::now();
						if current.duration_since(last) > Duration::from_millis(1000 / 60)
						{
							last = current;
							for client in clients.read().unwrap().iter() {
								client.do_send(Event(event.clone()));
							}
						}
					},
					event => {
						for client in clients.read().unwrap().iter() {
							client.do_send(Event(event.clone()));
						}
					},
				}
			}
		});
		server.start()
	}
}

enum EventServerOperation {
	Connect(Addr<Socket>),
	Disconnect(Addr<Socket>),
}

impl Message for EventServerOperation {
	type Result = ();
}

impl Handler<EventServerOperation> for EventServer {
	type Result = ();

	fn handle(
		&mut self,
		msg: EventServerOperation,
		_: &mut Context<Self>,
	) -> Self::Result {
		match msg {
			EventServerOperation::Connect(addr) => {
				self.clients.write().unwrap().insert(addr)
			},
			EventServerOperation::Disconnect(addr) => {
				self.clients.write().unwrap().remove(&addr)
			},
		};
	}
}

/// Define HTTP actor
struct Socket(web::Data<NodeController>, web::Data<Addr<EventServer>>);

impl Actor for Socket {
	type Context = ws::WebsocketContext<Self>;
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
enum SocketMessagePayload {
	Command(ClientCommand),
	Query(ClientQuery),
}

#[derive(Serialize, Deserialize, Message)]
#[rtype(result = "()")]
#[serde(rename_all = "camelCase")]
struct SocketMessage {
	id: String,
	payload: SocketMessagePayload,
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Socket {
	fn handle(
		&mut self,
		msg: Result<ws::Message, ws::ProtocolError>,
		ctx: &mut Self::Context,
	) {
		// TODO: Add heartbeat and reconnect logic in the future. We can refer to https://github.com/actix/examples/blob/master/websockets/chat/src/session.rs for the heartbeat stuff.

		match msg {
			Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
			Ok(ws::Message::Text(text)) => {
				let msg = serde_json::from_str::<SocketMessage>(&text);

				let msg = match msg {
					Ok(msg) => msg,
					Err(err) => {
						println!("Error parsing message: {}", err);
						return;
					},
				};

				let core = self.0.clone();
				self.1.do_send(EventServerOperation::Connect(ctx.address()));

				let recipient = ctx.address().recipient();
				let fut = async move {
					match msg.payload {
						SocketMessagePayload::Query(query) => {
							match core.query(query).await {
								Ok(response) => {
									recipient.do_send(SocketResponse::Response {
										id: msg.id.clone(),
										payload: response,
									})
								},
								Err(err) => {
									println!("query error: {:?}", err);
									// Err(err.to_string())
								},
							};
						},
						SocketMessagePayload::Command(command) => {
							match core.command(command).await {
								Ok(response) => {
									recipient.do_send(SocketResponse::Response {
										id: msg.id.clone(),
										payload: response,
									})
								},
								Err(err) => {
									println!("command error: {:?}", err);
									// Err(err.to_string())
								},
							};
						},
					}
				};

				fut.into_actor(self).spawn(ctx);

				()
			},
			_ => (),
		}
	}

	fn finished(&mut self, ctx: &mut Self::Context) {
		self.1
			.do_send(EventServerOperation::Disconnect(ctx.address()));
		ctx.stop();
	}
}

impl Handler<Event> for Socket {
	type Result = ();

	fn handle(&mut self, msg: Event, ctx: &mut Self::Context) {
		ctx.text(serde_json::to_string(&SocketResponse::Event(msg.0)).unwrap());
	}
}

#[derive(Message, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
#[rtype(result = "()")]
enum SocketResponse {
	Response { id: String, payload: CoreResponse },
	Event(CoreEvent),
}

impl Handler<SocketResponse> for Socket {
	type Result = ();

	fn handle(&mut self, msg: SocketResponse, ctx: &mut Self::Context) {
		ctx.text(serde_json::to_string(&msg).unwrap());
	}
}

#[get("/")]
async fn index() -> impl Responder {
	format!("Spacedrive Server!")
}

#[get("/health")]
async fn healthcheck() -> impl Responder {
	format!("OK")
}

#[get("/spacedrive/{path:.*}")]
async fn spacedrive(
	path: web::Path<String>,
	controller: web::Data<NodeController>,
) -> impl Responder {
	let (status_code, content_type, body) =
		controller.handle_custom_uri(path.split("/").collect());

	let mut resp = HttpResponse::new(StatusCode::from_u16(status_code).unwrap());
	resp.headers_mut().append(
		HeaderName::from_static("content-type"),
		HeaderValue::from_str(content_type).unwrap(),
	);
	resp.set_body(body)
}

#[get("/ws")]
async fn ws_handler(
	req: HttpRequest,
	stream: web::Payload,
	controller: web::Data<NodeController>,
	server: web::Data<Addr<EventServer>>,
) -> Result<HttpResponse, Error> {
	let resp = ws::start(Socket(controller, server), &req, stream);
	resp
}

async fn not_found() -> impl Responder {
	HttpResponse::build(StatusCode::OK).body("We're past the event horizon...")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	let data_dir_path = match env::var(DATA_DIR_ENV_VAR) {
		Ok(path) => Path::new(&path).to_path_buf(),
		Err(_e) => {
			#[cfg(not(debug_assertions))]
			{
				panic!("${} is not set ({})", DATA_DIR_ENV_VAR, _e)
			}

			std::env::current_dir()
				.expect(
					"Unable to get your currrent directory. Maybe try setting $DATA_DIR?",
				)
				.join("sdserver_data")
		},
	};
	let port = env::var("PORT")
		.map(|port| port.parse::<u16>().unwrap_or(8080))
		.unwrap_or(8080);

	let (controller, event_receiver, node, _guard) = Node::new(data_dir_path).await;
	tokio::spawn(node.start());

	let controller = web::Data::new(controller);
	let server = web::Data::new(EventServer::listen(event_receiver));
	println!("Listening http://localhost:{}", port);
	HttpServer::new(move || {
		App::new()
			.app_data(controller.clone())
			.app_data(server.clone())
			.service(index)
			.service(healthcheck)
			.service(ws_handler)
			.default_service(web::route().to(not_found))
	})
	.bind(("0.0.0.0", port))?
	.run()
	.await
}
