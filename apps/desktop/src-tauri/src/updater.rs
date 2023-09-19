use core::fmt;

use tokio::sync::Mutex;

#[derive(Debug, specta::Type, serde::Serialize)]
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

#[derive(Default, Clone)]
pub enum State {
	#[default]
	Idle,
	Fetching,
	Fetched(Option<tauri::updater::UpdateResponse<tauri::Wry>>),
	Downloading,
}

impl fmt::Debug for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Idle => write!(f, "Idle"),
			Self::Fetching => write!(f, "Fetching"),
			Self::Fetched(_) => write!(f, "Fetched"),
			Self::Downloading => write!(f, "Downloading"),
		}
	}
}

#[derive(Default)]
pub struct StateObject {
	current: Mutex<State>,
}

#[derive(Debug, serde::Serialize, specta::Type)]
pub enum CheckForUpdate {
	InProgress,
	Fetched(Option<Update>),
}

#[tauri::command]
#[specta::specta]
pub async fn check_for_update(
	app: tauri::AppHandle,
	state: tauri::State<'_, StateObject>,
) -> Result<CheckForUpdate, ()> {
	let lock = state.current.try_lock();

	let mut current = match lock {
		Ok(current) => current,
		Err(_) => return Ok(CheckForUpdate::InProgress),
	};

	let current = match &mut *current {
		current @ State::Idle | current @ State::Fetched(None) => current,
		State::Fetched(Some(update)) => {
			return Ok(CheckForUpdate::Fetched(Some(Update::new(update))))
		}
		_ => return Ok(CheckForUpdate::InProgress),
	};

	*current = State::Fetching;

	let update = tauri::updater::builder(app)
		.header("X-Spacedrive-Version", "stable")
		.map_err(|_| ())?
		.check()
		.await
		.map_err(|_| ())?;

	let ret = CheckForUpdate::Fetched(update.is_update_available().then(|| Update::new(&update)));

	*current = State::Fetched(update.is_update_available().then(|| update));

	Ok(ret)
}

#[tauri::command]
#[specta::specta]
pub async fn install_update(state: tauri::State<'_, StateObject>) -> Result<(), ()> {
	let mut current = state.current.lock().await;
	let current = &mut *current;

	let update = match std::mem::replace(current, State::Downloading) {
		State::Fetched(Some(update)) => update,
		s => {
			*current = s;
			return Err(());
		}
	};

	update.download_and_install().await.map_err(|_| ())?;

	Ok(())
}
