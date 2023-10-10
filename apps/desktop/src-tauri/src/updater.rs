use tauri::{plugin::TauriPlugin, Manager, Runtime};
use tokio::sync::Mutex;
use tracing::{error, warn};

#[derive(Debug, Clone, specta::Type, serde::Serialize)]
pub struct Update {
	pub version: String,
	pub body: Option<String>,
}

impl Update {
	fn new(update: &tauri::updater::UpdateResponse<impl tauri::Runtime>) -> Self {
		Self {
			version: update.latest_version().to_string(),
			body: update.body().map(|b| b.to_string()),
		}
	}
}

#[derive(Default)]
pub struct State {
	install_lock: Mutex<()>,
}

async fn get_update(
	app: tauri::AppHandle,
) -> Result<tauri::updater::UpdateResponse<impl tauri::Runtime>, String> {
	tauri::updater::builder(app)
		.header("X-Spacedrive-Version", "stable")
		.map_err(|e| e.to_string())?
		.check()
		.await
		.map_err(|e| e.to_string())
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum UpdateEvent {
	Loading,
	Error(String),
	UpdateAvailable { update: Update },
	NoUpdateAvailable,
	Installing,
}

#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<Option<Update>, String> {
	app.emit_all("updater", UpdateEvent::Loading).ok();

	let update = match get_update(app.clone()).await {
		Ok(update) => update,
		Err(e) => {
			app.emit_all("updater", UpdateEvent::Error(e.clone())).ok();
			return Err(e);
		}
	};

	let update = update.is_update_available().then(|| Update::new(&update));

	app.emit_all(
		"updater",
		update
			.clone()
			.map(|update| UpdateEvent::UpdateAvailable { update })
			.unwrap_or(UpdateEvent::NoUpdateAvailable),
	)
	.ok();

	Ok(update)
}

#[tauri::command]
#[specta::specta]
pub async fn install_update(
	app: tauri::AppHandle,
	state: tauri::State<'_, State>,
) -> Result<(), String> {
	let lock = match state.install_lock.try_lock() {
		Ok(lock) => lock,
		Err(_) => return Err("Update already installing".into()),
	};

	app.emit_all("updater", UpdateEvent::Installing).ok();

	get_update(app.clone())
		.await?
		.download_and_install()
		.await
		.map_err(|e| e.to_string())?;

	drop(lock);

	Ok(())
}

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
	tauri::plugin::Builder::new("sd-updater")
		.on_page_load(|window, _| {
			#[cfg(target_os = "linux")]
			let updater_available = {
				let env = window.env();

				env.appimage.is_some()
			};
			#[cfg(not(target_os = "linux"))]
			let updater_available = true;

			if updater_available {
				window
					.eval(&format!("window.__SD_UPDATER__ = true"))
					.expect("Failed to inject updater JS");
			}
		})
		.js_init_script(format!(""))
		.build()
}
