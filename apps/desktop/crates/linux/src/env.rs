use std::{
	collections::HashSet,
	env,
	ffi::{CStr, OsStr, OsString},
	mem,
	os::unix::ffi::OsStrExt,
	path::{Path, PathBuf},
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
		let mut dirs = env::split_paths(&value).collect::<Vec<_>>();

		let mut insert_index: usize = dirs.len();
		for default_dir in default_dirs {
			match dirs.iter().position(|dir| dir == default_dir) {
				Some(index) => insert_index = index,
				None => dirs.insert(insert_index, default_dir.to_path_buf()),
			}
		}

		dirs
	} else {
		default_dirs.into()
	};

	let mut unique = HashSet::new();
	let pathlist = dirs
		.iter()
		.filter(|dir| unique.insert(*dir))
		.cloned()
		.collect::<Vec<_>>();

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
}

pub(crate) fn remove_prefix_from_pathlist(
	env_name: &str,
	prefix: &impl AsRef<Path>,
) -> Option<OsString> {
	env::var_os(env_name).map(|value| env::join_paths(env::split_paths(&value).filter(|dir| !dir.starts_with(prefix))).expect("Should not fail because we are only filtering a pathlist retrieved from the environmnet"))
}
