use crate::api::{Ctx, R};

use sd_core_cloud_services::QuinnConnection;

use sd_cloud_schema::{
	auth::AccessToken,
	devices::{self, DeviceOS, HardwareModel, PubId},
	opaque_ke::{
		ClientLogin, ClientLoginFinishParameters, ClientLoginFinishResult, ClientLoginStartResult,
		ClientRegistration, ClientRegistrationFinishParameters, ClientRegistrationFinishResult,
		ClientRegistrationStartResult,
	},
	Client, NodeId, Request, Response, SpacedriveCipherSuite,
};
use sd_crypto::{cloud::secret_key::SecretKey, CryptoRng};

use blake3::Hash;
use futures::{FutureExt, SinkExt, StreamExt};
use futures_concurrency::future::TryJoin;
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use tracing::{debug, error};

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|node, pub_id: devices::PubId| async move {
				use devices::get::{Request, Response};

				let (client, access_token) = super::get_client_and_access_token(&node).await?;

				let Response(device) = super::handle_comm_error(
					client
						.devices()
						.get(Request {
							pub_id,
							access_token,
						})
						.await,
					"Failed to get device;",
				)??;

				debug!(?device, "Got device");

				Ok(device)
			})
		})
		.procedure("list", {
			R.query(|node, _: ()| async move {
				use devices::list::{Request, Response};

				let ((client, access_token), pub_id) = (
					super::get_client_and_access_token(&node),
					node.config.get().map(|config| Ok(config.id.into())),
				)
					.try_join()
					.await?;

				let Response(mut devices) = super::handle_comm_error(
					client.devices().list(Request { access_token }).await,
					"Failed to list devices;",
				)??;

				// Filter out the local device by matching pub_id
				devices.retain(|device| device.pub_id != pub_id);

				debug!(?devices, "Listed devices");

				Ok(devices)
			})
		})
		.procedure("get_current_device", {
			R.query(|node, _: ()| async move {
				use devices::get::{Request, Response};

				let ((client, access_token), pub_id) = (
					super::get_client_and_access_token(&node),
					node.config.get().map(|config| Ok(config.id.into())),
				)
					.try_join()
					.await?;

				let Response(device) = super::handle_comm_error(
					client
						.devices()
						.get(Request {
							pub_id,
							access_token,
						})
						.await,
					"Failed to get current device;",
				)??;
				Ok(device)
			})
		})
		.procedure("delete", {
			R.mutation(|node, pub_id: devices::PubId| async move {
				use devices::delete::Request;

				let (client, access_token) = super::get_client_and_access_token(&node).await?;

				super::handle_comm_error(
					client
						.devices()
						.delete(Request {
							pub_id,
							access_token,
						})
						.await,
					"Failed to delete device;",
				)??;

				debug!("Deleted device");

				Ok(())
			})
		})
		.procedure("update", {
			#[derive(Deserialize, specta::Type)]
			struct CloudUpdateDeviceArgs {
				pub_id: devices::PubId,
				name: String,
			}

			R.mutation(
				|node, CloudUpdateDeviceArgs { pub_id, name }: CloudUpdateDeviceArgs| async move {
					use devices::update::Request;

					let (client, access_token) = super::get_client_and_access_token(&node).await?;

					super::handle_comm_error(
						client
							.devices()
							.update(Request {
								access_token,
								pub_id,
								name,
							})
							.await,
						"Failed to update device;",
					)??;

					debug!("Updated device");

					Ok(())
				},
			)
		})
}

pub async fn hello(
	client: &Client<QuinnConnection<Response, Request>>,
	access_token: AccessToken,
	device_pub_id: PubId,
	hashed_pub_id: Hash,
	rng: &mut CryptoRng,
) -> Result<SecretKey, rspc::Error> {
	use devices::hello::{Request, RequestUpdate, Response, State};

	let ClientLoginStartResult { message, state } =
		ClientLogin::<SpacedriveCipherSuite>::start(rng, hashed_pub_id.as_bytes().as_slice())
			.map_err(|e| {
				error!(?e, "OPAQUE error initializing device hello request;");
				rspc::Error::new(
					rspc::ErrorCode::InternalServerError,
					"Failed to initialize device login".into(),
				)
			})?;

	let (mut hello_continuation, mut res_stream) = super::handle_comm_error(
		client
			.devices()
			.hello(Request {
				access_token,
				pub_id: device_pub_id,
				opaque_login_message: Box::new(message),
			})
			.await,
		"Failed to send device hello request;",
	)?;

	let Some(res) = res_stream.next().await else {
		let message = "Server did not send a device hello response;";
		error!("{message}");
		return Err(rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			message.to_string(),
		));
	};

	let credential_response = match super::handle_comm_error(
		res,
		"Communication error on device hello response;",
	)? {
		Ok(Response(State::LoginResponse(credential_response))) => credential_response,

		Ok(Response(State::End)) => {
			unreachable!("Device hello response MUST not be End here, this is a serious bug and should crash;");
		}

		Err(e) => {
			error!(?e, "Device hello response error;");
			return Err(e.into());
		}
	};

	let ClientLoginFinishResult {
		message,
		export_key,
		..
	} = state
		.finish(
			hashed_pub_id.as_bytes().as_slice(),
			*credential_response,
			ClientLoginFinishParameters::default(),
		)
		.map_err(|e| {
			error!(?e, "Device hello finish error;");
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Failed to finish device login".into(),
			)
		})?;

	hello_continuation
		.send(RequestUpdate {
			opaque_login_finish: Box::new(message),
		})
		.await
		.map_err(|e| {
			error!(?e, "Failed to send device hello request continuation;");
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Failed to finish device login procedure;".into(),
			)
		})?;

	let Some(res) = res_stream.next().await else {
		let message = "Server did not send a device hello END response;";
		error!("{message}");
		return Err(rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			message.to_string(),
		));
	};

	match super::handle_comm_error(res, "Communication error on device hello response;")? {
		Ok(Response(State::LoginResponse(_))) => {
			unreachable!("Device hello final response MUST be End here, this is a serious bug and should crash;");
		}

		Ok(Response(State::End)) => {
			// Protocol completed successfully
			Ok(SecretKey::from(export_key))
		}

		Err(e) => {
			error!(?e, "Device hello final response error;");
			Err(e.into())
		}
	}
}

pub struct DeviceRegisterData {
	pub pub_id: PubId,
	pub name: String,
	pub os: DeviceOS,
	pub hardware_model: HardwareModel,
	pub connection_id: NodeId,
}

pub async fn register(
	client: &Client<QuinnConnection<Response, Request>>,
	access_token: AccessToken,
	DeviceRegisterData {
		pub_id,
		name,
		os,
		hardware_model,
		connection_id,
	}: DeviceRegisterData,
	hashed_pub_id: Hash,
	rng: &mut CryptoRng,
) -> Result<SecretKey, rspc::Error> {
	use devices::register::{Request, RequestUpdate, Response, State};

	let ClientRegistrationStartResult { message, state } =
		ClientRegistration::<SpacedriveCipherSuite>::start(
			rng,
			hashed_pub_id.as_bytes().as_slice(),
		)
		.map_err(|e| {
			error!(?e, "OPAQUE error initializing device register request;");
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Failed to initialize device register".into(),
			)
		})?;

	let (mut register_continuation, mut res_stream) = super::handle_comm_error(
		client
			.devices()
			.register(Request {
				access_token,
				pub_id,
				name,
				os,
				hardware_model,
				connection_id,
				opaque_register_message: Box::new(message),
			})
			.await,
		"Failed to send device register request;",
	)?;

	let Some(res) = res_stream.next().await else {
		let message = "Server did not send a device register response;";
		error!("{message}");
		return Err(rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			message.to_string(),
		));
	};

	let registration_response = match super::handle_comm_error(
		res,
		"Communication error on device register response;",
	)? {
		Ok(Response(State::RegistrationResponse(res))) => res,

		Ok(Response(State::End)) => {
			unreachable!("Device hello response MUST not be End here, this is a serious bug and should crash;");
		}

		Err(e) => {
			error!(?e, "Device hello response error;");
			return Err(e.into());
		}
	};

	let ClientRegistrationFinishResult {
		message,
		export_key,
		..
	} = state
		.finish(
			rng,
			hashed_pub_id.as_bytes().as_slice(),
			*registration_response,
			ClientRegistrationFinishParameters::default(),
		)
		.map_err(|e| {
			error!(?e, "Device register finish error;");
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Failed to finish device register".into(),
			)
		})?;

	register_continuation
		.send(RequestUpdate {
			opaque_registration_finish: Box::new(message),
		})
		.await
		.map_err(|e| {
			error!(?e, "Failed to send device register request continuation;");
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Failed to finish device register procedure;".into(),
			)
		})?;

	let Some(res) = res_stream.next().await else {
		let message = "Server did not send a device register END response;";
		error!("{message}");
		return Err(rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			message.to_string(),
		));
	};

	match super::handle_comm_error(res, "Communication error on device register response;")? {
		Ok(Response(State::RegistrationResponse(_))) => {
			unreachable!("Device register final response MUST be End here, this is a serious bug and should crash;");
		}

		Ok(Response(State::End)) => {
			// Protocol completed successfully
			Ok(SecretKey::from(export_key))
		}

		Err(e) => {
			error!(?e, "Device register final response error;");
			Err(e.into())
		}
	}
}
