use sdcore::{ClientCommand, ClientQuery, Core, CoreController, CoreEvent, CoreResponse};
use std::{env, path::Path};

use actix::{
	Actor, AsyncContext, ContextFutureSpawner, Handler, Message, StreamHandler,
	WrapFuture,
};
use actix_web::{
	get, http::StatusCode, web, App, Error, HttpRequest, HttpResponse, HttpServer,
	Responder,
};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};

use tokio::sync::mpsc;

const DATA_DIR_ENV_VAR: &'static str = "DATA_DIR";

/// Define HTTP actor
struct Socket {
	_event_receiver: web::Data<mpsc::Receiver<CoreEvent>>,
	core: web::Data<CoreController>,
}

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
				let msg: SocketMessage = serde_json::from_str(&text).unwrap();

				let core = self.core.clone();

				let recipient = ctx.address().recipient();

				let fut = async move {
					match msg.payload {
						SocketMessagePayload::Query(query) => {
							match core.query(query).await {
								Ok(response) => recipient.do_send(SocketResponse {
									id: msg.id.clone(),
									payload: SocketResponsePayload::Query(response),
								}),
								Err(err) => {
									println!("query error: {:?}", err);
									// Err(err.to_string())
								},
							};
						},
						SocketMessagePayload::Command(command) => {
							match core.command(command).await {
								Ok(response) => recipient.do_send(SocketResponse {
									id: msg.id.clone(),
									payload: SocketResponsePayload::Query(response),
								}),
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum SocketResponsePayload {
	Query(CoreResponse),
}

#[derive(Message, Serialize)]
#[rtype(result = "()")]
struct SocketResponse {
	id: String,
	payload: SocketResponsePayload,
}

impl Handler<SocketResponse> for Socket {
	type Result = ();

	fn handle(&mut self, msg: SocketResponse, ctx: &mut Self::Context) {
		let string = serde_json::to_string(&msg).unwrap();
		ctx.text(string);
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

#[get("/ws")]
async fn ws_handler(
	req: HttpRequest,
	stream: web::Payload,
	event_receiver: web::Data<mpsc::Receiver<CoreEvent>>,
	controller: web::Data<CoreController>,
) -> Result<HttpResponse, Error> {
	let resp = ws::start(
		Socket {
			_event_receiver: event_receiver,
			core: controller,
		},
		&req,
		stream,
	);
	resp
}

#[get("/file/{file:.*}")]
async fn file() -> impl Responder {
	// TODO
	format!("OK")
}

async fn not_found() -> impl Responder {
	HttpResponse::build(StatusCode::OK).body("We're past the event horizon...")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	let (event_receiver, controller) = setup().await;

	println!("Listening http://localhost:8080");
	HttpServer::new(move || {
		App::new()
			.app_data(event_receiver.clone())
			.app_data(controller.clone())
			.service(index)
			.service(healthcheck)
			.service(ws_handler)
			.service(file)
			.default_service(web::route().to(not_found))
	})
	.bind(("0.0.0.0", 8080))?
	.run()
	.await
}

async fn setup() -> (
	web::Data<mpsc::Receiver<CoreEvent>>,
	web::Data<CoreController>,
) {
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

	let (mut core, event_receiver) = Core::new(data_dir_path).await;

	core.initializer().await;

	let controller = core.get_controller();

	tokio::spawn(async move {
		core.start().await;
	});

	(web::Data::new(event_receiver), web::Data::new(controller))
}
