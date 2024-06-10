use std::{
	collections::HashSet,
	env,
	ffi::{CStr, OsStr},
	mem,
	os::unix::ffi::OsStrExt,
	path::PathBuf,
	ptr,
};

pub fn get_current_user_home() -> Option<PathBuf> {
	use libc::{getpwuid_r, getuid, passwd, ERANGE};

	if let Some(home) = env::var_os("HOME") {
		let home = PathBuf::from(home);
		if home.is_absolute() && home.is_dir() {
			return Some(home);
		}
	}

	let uid = unsafe { getuid() };
	let mut buf = vec![0; 2048];
	let mut passwd = unsafe { mem::zeroed::<passwd>() };
	let mut result = ptr::null_mut::<passwd>();

	loop {
		let r = unsafe { getpwuid_r(uid, &mut passwd, buf.as_mut_ptr(), buf.len(), &mut result) };

		if r != ERANGE {
			break;
		}

		let newsize = buf.len().checked_mul(2)?;
		buf.resize(newsize, 0);
	}

	if result.is_null() {
		// There is no such user, or an error has occurred.
		// errno gets set if there’s an error.
		return None;
	}

	if result != &mut passwd {
		// The result of getpwuid_r should be its input passwd.
		return None;
	}

	let passwd: passwd = unsafe { result.read() };
	if passwd.pw_dir.is_null() {
		return None;
	}

	let home = PathBuf::from(OsStr::from_bytes(
		unsafe { CStr::from_ptr(passwd.pw_dir) }.to_bytes(),
	));
	if home.is_absolute() && home.is_dir() {
		env::set_var("HOME", &home);
		Some(home)
	} else {
		None
	}
}

fn normalize_pathlist(
	env_name: &str,
	default_dirs: &[PathBuf],
) -> Result<Vec<PathBuf>, env::JoinPathsError> {
	let dirs = if let Some(value) = env::var_os(env_name) {
		let mut dirs = env::split_paths(&value)
			.filter(|entry| !entry.as_os_str().is_empty())
			.collect::<Vec<_>>();

		let mut insert_index = dirs.len();
		for default_dir in default_dirs {
			match dirs.iter().rev().position(|dir| dir == default_dir) {
				Some(mut index) => {
					index = dirs.len() - index - 1;
					if index < insert_index {
						insert_index = index
					}
				}
				None => dirs.insert(insert_index, default_dir.to_path_buf()),
			}
		}

		dirs
	} else {
		default_dirs.into()
	};

	let mut unique = HashSet::new();
	let mut pathlist = dirs
		.iter()
		.rev() // Reverse order to remove duplicates from the end
		.filter(|dir| unique.insert(*dir))
		.cloned()
		.collect::<Vec<_>>();

	pathlist.reverse();

	env::set_var(env_name, env::join_paths(&pathlist)?);

	Ok(pathlist)
}

fn normalize_xdg_environment(name: &str, default_value: PathBuf) -> PathBuf {
	if let Some(value) = env::var_os(name) {
		if !value.is_empty() {
			let path = PathBuf::from(value);
			if path.is_absolute() && path.is_dir() {
				return path;
			}
		}
	}

	env::set_var(name, &default_value);
	default_value
}

pub fn normalize_environment() {
	let home = get_current_user_home().expect("No user home directory found");

	// Normalize user XDG dirs environment variables
	// https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
	let data_home = normalize_xdg_environment("XDG_DATA_HOME", home.join(".local/share"));
	normalize_xdg_environment("XDG_CACHE_HOME", home.join(".cache"));
	normalize_xdg_environment("XDG_CONFIG_HOME", home.join(".config"));

	// Normalize system XDG dirs environment variables
	// https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
	normalize_pathlist(
		"XDG_DATA_DIRS",
		&[
			PathBuf::from("/usr/share"),
			PathBuf::from("/usr/local/share"),
			PathBuf::from("/var/lib/flatpak/exports/share"),
			data_home.join("flatpak/exports/share"),
		],
	)
	.expect("XDG_DATA_DIRS must be successfully normalized");
	normalize_pathlist("XDG_CONFIG_DIRS", &[PathBuf::from("/etc/xdg")])
		.expect("XDG_CONFIG_DIRS must be successfully normalized");

	// Normalize GStreamer plugin path
	// https://gstreamer.freedesktop.org/documentation/gstreamer/gstregistry.html#gstregistry-page
	normalize_pathlist(
		"GST_PLUGIN_SYSTEM_PATH",
		&[
			PathBuf::from("/usr/lib/gstreamer"),
			data_home.join("gstreamer/plugins"),
		],
	)
	.expect("GST_PLUGIN_SYSTEM_PATH must be successfully normalized");
	normalize_pathlist(
		"GST_PLUGIN_SYSTEM_PATH_1_0",
		&[
			PathBuf::from("/usr/lib/gstreamer-1.0"),
			data_home.join("gstreamer-1.0/plugins"),
		],
	)
	.expect("GST_PLUGIN_SYSTEM_PATH_1_0 must be successfully normalized");

	// Normalize PATH
	normalize_pathlist(
		"PATH",
		&[
			PathBuf::from("/sbin"),
			PathBuf::from("/bin"),
			PathBuf::from("/usr/sbin"),
			PathBuf::from("/usr/bin"),
			PathBuf::from("/usr/local/sbin"),
			PathBuf::from("/usr/local/bin"),
			PathBuf::from("/var/lib/flatpak/exports/bin"),
			data_home.join("flatpak/exports/bin"),
		],
	)
	.expect("PATH must be successfully normalized");

	if has_nvidia() {
		// Workaround for: https://github.com/tauri-apps/tauri/issues/9304
		env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
	}
}

// Check if snap by looking if SNAP is set and not empty and that the SNAP directory exists
pub fn is_snap() -> bool {
	if let Some(snap) = std::env::var_os("SNAP") {
		if !snap.is_empty() && PathBuf::from(snap).is_dir() {
			return true;
		}
	}

	false
}

// Check if flatpak by looking if FLATPAK_ID is set and not empty and that the .flatpak-info file exists
pub fn is_flatpak() -> bool {
	if let Some(flatpak_id) = std::env::var_os("FLATPAK_ID") {
		if !flatpak_id.is_empty() && PathBuf::from("/.flatpak-info").is_file() {
			return true;
		}
	}

	false
}

fn has_nvidia() -> bool {
	use wgpu::{
		Backends, DeviceType, Dx12Compiler, Gles3MinorVersion, Instance, InstanceDescriptor,
		InstanceFlags,
	};

	let instance = Instance::new(InstanceDescriptor {
		flags: InstanceFlags::empty(),
		backends: Backends::VULKAN | Backends::GL,
		gles_minor_version: Gles3MinorVersion::Automatic,
		dx12_shader_compiler: Dx12Compiler::default(),
	});
	for adapter in instance.enumerate_adapters(Backends::all()) {
		let info = adapter.get_info();
		match info.device_type {
			DeviceType::DiscreteGpu | DeviceType::IntegratedGpu | DeviceType::VirtualGpu => {
				// Nvidia PCI id
				if info.vendor == 0x10de {
					return true;
				}
			}
			_ => {}
		}
	}

	false
}
