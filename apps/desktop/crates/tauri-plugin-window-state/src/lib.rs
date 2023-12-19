// Copyright 2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use tauri::{
	plugin::{Builder as PluginBuilder, TauriPlugin},
	LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, RunEvent, Runtime, Window,
	WindowEvent,
};

use std::{
	collections::{HashMap, HashSet},
	fs::{create_dir_all, File},
	io::Write,
	sync::{Arc, Mutex},
};

mod cmd;

pub const STATE_FILENAME: &str = ".window-state";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Tauri(#[from] tauri::Error),
	#[error(transparent)]
	TauriApi(#[from] tauri::api::Error),
	#[error(transparent)]
	Bincode(#[from] Box<bincode::ErrorKind>),
}

pub type Result<T> = std::result::Result<T, Error>;

bitflags! {
	#[derive(Clone, Copy, Debug)]
	pub struct StateFlags: u32 {
		const SIZE        = 1 << 0;
		const POSITION    = 1 << 1;
		const MAXIMIZED   = 1 << 2;
		const VISIBLE     = 1 << 3;
		const DECORATIONS = 1 << 4;
		const FULLSCREEN  = 1 << 5;
	}
}

impl Default for StateFlags {
	fn default() -> Self {
		Self::all()
	}
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct WindowState {
	width: f64,
	height: f64,
	x: i32,
	y: i32,
	maximized: bool,
	visible: bool,
	decorated: bool,
	fullscreen: bool,
}

impl Default for WindowState {
	fn default() -> Self {
		Self {
			width: Default::default(),
			height: Default::default(),
			x: Default::default(),
			y: Default::default(),
			maximized: Default::default(),
			visible: true,
			decorated: true,
			fullscreen: Default::default(),
		}
	}
}

struct WindowStateCache(Arc<Mutex<HashMap<String, WindowState>>>);
pub trait AppHandleExt {
	/// Saves all open windows state to disk
	fn save_window_state(&self, flags: StateFlags) -> Result<()>;
}

impl<R: Runtime> AppHandleExt for tauri::AppHandle<R> {
	fn save_window_state(&self, flags: StateFlags) -> Result<()> {
		if let Some(app_dir) = self.path_resolver().app_config_dir() {
			let state_path = app_dir.join(STATE_FILENAME);
			let cache = self.state::<WindowStateCache>();
			let mut state = cache.0.lock().unwrap();
			for (label, s) in state.iter_mut() {
				if let Some(window) = self.get_window(label) {
					window.update_state(s, flags)?;
				}
			}

			create_dir_all(&app_dir)
				.map_err(Error::Io)
				.and_then(|_| File::create(state_path).map_err(Into::into))
				.and_then(|mut f| {
					f.write_all(&bincode::serialize(&*state).map_err(Error::Bincode)?)
						.map_err(Into::into)
				})
		} else {
			Ok(())
		}
	}
}

pub trait WindowExt {
	/// Restores this window state from disk
	fn restore_state(&self, flags: StateFlags) -> tauri::Result<()>;
}

impl<R: Runtime> WindowExt for Window<R> {
	fn restore_state(&self, flags: StateFlags) -> tauri::Result<()> {
		let cache = self.state::<WindowStateCache>();
		let mut c = cache.0.lock().unwrap();

		let mut should_show = true;

		if let Some(state) = c.get(self.label()) {
			// avoid restoring the default zeroed state
			if *state == WindowState::default() {
				return Ok(());
			}

			if flags.contains(StateFlags::DECORATIONS) {
				self.set_decorations(state.decorated)?;
			}

			if flags.contains(StateFlags::SIZE) {
				self.set_size(LogicalSize {
					width: state.width,
					height: state.height,
				})?;
			}

			if flags.contains(StateFlags::POSITION) {
				// restore position to saved value if saved monitor exists
				// otherwise, let the OS decide where to place the window
				for m in self.available_monitors()? {
					if m.contains((state.x, state.y).into()) {
						self.set_position(PhysicalPosition {
							x: state.x,
							y: state.y,
						})?;
					}
				}
			}

			if flags.contains(StateFlags::MAXIMIZED) && state.maximized {
				self.maximize()?;
			}

			if flags.contains(StateFlags::FULLSCREEN) {
				self.set_fullscreen(state.fullscreen)?;
			}

			should_show = state.visible;
		} else {
			let mut metadata = WindowState::default();

			if flags.contains(StateFlags::SIZE) {
				let scale_factor = self
					.current_monitor()?
					.map(|m| m.scale_factor())
					.unwrap_or(1.);
				let size = self.inner_size()?.to_logical(scale_factor);
				metadata.width = size.width;
				metadata.height = size.height;
			}

			if flags.contains(StateFlags::POSITION) {
				let pos = self.outer_position()?;
				metadata.x = pos.x;
				metadata.y = pos.y;
			}

			if flags.contains(StateFlags::MAXIMIZED) {
				metadata.maximized = self.is_maximized()?;
			}

			if flags.contains(StateFlags::VISIBLE) {
				metadata.visible = self.is_visible()?;
			}

			if flags.contains(StateFlags::DECORATIONS) {
				metadata.decorated = self.is_decorated()?;
			}

			if flags.contains(StateFlags::FULLSCREEN) {
				metadata.fullscreen = self.is_fullscreen()?;
			}

			c.insert(self.label().into(), metadata);
		}

		if flags.contains(StateFlags::VISIBLE) && should_show {
			self.show()?;
			self.set_focus()?;
		}

		Ok(())
	}
}

trait WindowExtInternal {
	fn update_state(&self, state: &mut WindowState, flags: StateFlags) -> tauri::Result<()>;
}

impl<R: Runtime> WindowExtInternal for Window<R> {
	fn update_state(&self, state: &mut WindowState, flags: StateFlags) -> tauri::Result<()> {
		let is_maximized = match flags.intersects(StateFlags::MAXIMIZED | StateFlags::SIZE) {
			true => self.is_maximized()?,
			false => false,
		};

		if flags.contains(StateFlags::MAXIMIZED) {
			state.maximized = is_maximized;
		}

		if flags.contains(StateFlags::FULLSCREEN) {
			state.fullscreen = self.is_fullscreen()?;
		}

		if flags.contains(StateFlags::DECORATIONS) {
			state.decorated = self.is_decorated()?;
		}

		if flags.contains(StateFlags::VISIBLE) {
			state.visible = self.is_visible()?;
		}

		if flags.contains(StateFlags::SIZE) {
			let scale_factor = self
				.current_monitor()?
				.map(|m| m.scale_factor())
				.unwrap_or(1.);
			let size = self.inner_size()?.to_logical(scale_factor);

			// It doesn't make sense to save a self with 0 height or width
			if size.width > 0. && size.height > 0. && !is_maximized {
				state.width = size.width;
				state.height = size.height;
			}
		}

		if flags.contains(StateFlags::POSITION) {
			let position = self.outer_position()?;
			if let Ok(Some(monitor)) = self.current_monitor() {
				// save only window positions that are inside the current monitor
				if monitor.contains(position) && !is_maximized {
					state.x = position.x;
					state.y = position.y;
				}
			}
		}

		Ok(())
	}
}

#[derive(Default)]
pub struct Builder {
	denylist: HashSet<String>,
	skip_initial_state: HashSet<String>,
	state_flags: StateFlags,
}

impl Builder {
	/// Sets the state flags to control what state gets restored and saved.
	pub fn with_state_flags(mut self, flags: StateFlags) -> Self {
		self.state_flags = flags;
		self
	}

	/// Sets a list of windows that shouldn't be tracked and managed by this plugin
	/// for example splash screen windows.
	pub fn with_denylist(mut self, denylist: &[&str]) -> Self {
		self.denylist = denylist.iter().map(|l| l.to_string()).collect();
		self
	}

	/// Adds the given window label to a list of windows to skip initial state restore.
	pub fn skip_initial_state(mut self, label: &str) -> Self {
		self.skip_initial_state.insert(label.into());
		self
	}

	pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
		let flags = self.state_flags;
		PluginBuilder::new("window-state")
			.invoke_handler(tauri::generate_handler![
				cmd::save_window_state,
				cmd::restore_state
			])
			.setup(|app| {
				let cache: Arc<Mutex<HashMap<String, WindowState>>> = if let Some(app_dir) =
					app.path_resolver().app_config_dir()
				{
					let state_path = app_dir.join(STATE_FILENAME);
					if state_path.exists() {
						Arc::new(Mutex::new(
							tauri::api::file::read_binary(state_path)
								.map_err(Error::TauriApi)
								.and_then(|state| bincode::deserialize(&state).map_err(Into::into))
								.unwrap_or_default(),
						))
					} else {
						Default::default()
					}
				} else {
					Default::default()
				};
				app.manage(WindowStateCache(cache));
				Ok(())
			})
			.on_webview_ready(move |window| {
				if self.denylist.contains(window.label()) {
					return;
				}

				if !self.skip_initial_state.contains(window.label()) {
					let _ = window.restore_state(self.state_flags);
				}

				let cache = window.state::<WindowStateCache>();
				let cache = cache.0.clone();
				let label = window.label().to_string();
				let window_clone = window.clone();
				let flags = self.state_flags;

				// insert a default state if this window should be tracked and
				// the disk cache doesn't have a state for it
				{
					cache
						.lock()
						.unwrap()
						.entry(label.clone())
						.or_insert_with(WindowState::default);
				}

				window.on_window_event(move |e| {
					if let WindowEvent::CloseRequested { .. } = e {
						let mut c = cache.lock().unwrap();
						if let Some(state) = c.get_mut(&label) {
							let _ = window_clone.update_state(state, flags);
						}
					}
				});
			})
			.on_event(move |app, event| {
				if let RunEvent::Exit = event {
					let _ = app.save_window_state(flags);
				}
			})
			.build()
	}
}

trait MonitorExt {
	fn contains(&self, position: PhysicalPosition<i32>) -> bool;
}

impl MonitorExt for Monitor {
	fn contains(&self, position: PhysicalPosition<i32>) -> bool {
		let PhysicalPosition { x, y } = *self.position();
		let PhysicalSize { width, height } = *self.size();

		x < position.x as _
			&& position.x < (x + width as i32)
			&& y < position.y as _
			&& position.y < (y + height as i32)
	}
}
