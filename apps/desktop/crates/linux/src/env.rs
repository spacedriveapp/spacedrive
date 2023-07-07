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
	if let Some(home) = env::var("HOME").ok().map(PathBuf::from) {
		if home.is_absolute() && home.is_dir() {
			return Some(home);
		}
	}

	let uid = unsafe { libc::getuid() };
	let mut buf = vec![0; 2048];
	let mut passwd = unsafe { mem::zeroed::<libc::passwd>() };
	let mut result = ptr::null_mut::<libc::passwd>();

	loop {
		let r =
			unsafe { libc::getpwuid_r(uid, &mut passwd, buf.as_mut_ptr(), buf.len(), &mut result) };

		if r != libc::ERANGE {
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

	let passwd: libc::passwd = unsafe { result.read() };
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

fn normalize_pathlist(var_name: &str, default_dirs: &[PathBuf]) {
	let dirs = if let Ok(var_value) = env::var(var_name) {
		let mut dirs = var_value.split(':').map(PathBuf::from).collect::<Vec<_>>();

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
	env::set_var(
		var_name,
		dirs.iter()
			.filter(|dir| unique.insert(*dir))
			.map(|dir| dir.as_os_str())
			.collect::<Vec<&OsStr>>()
			.join(OsStr::new(":")),
	);
}

fn normalize_xdg_environment(name: &str, default_value: PathBuf) -> PathBuf {
	if let Ok(value) = env::var(name) {
		if value.is_empty() {
			let path = PathBuf::from(value);
			if path.is_absolute() && path.is_dir() {
				env::set_var(name, &path);
				return path;
			}
		}
	}

	env::set_var(name, &default_value);
	default_value
}

pub fn normalize_environment() {
	let home = get_current_user_home().expect("No user home directory found");

	// Normalize XDG environment variables
	let data_home = normalize_xdg_environment("XDG_DATA_HOME", home.join(".local/share"));
	normalize_xdg_environment("XDG_CACHE_HOME", home.join(".cache"));
	normalize_xdg_environment("XDG_CONFIG_HOME", home.join(".config"));
	normalize_pathlist(
		"XDG_DATA_DIRS",
		&[
			PathBuf::from("/usr/share"),
			PathBuf::from("/usr/local/share"),
		],
	);
	normalize_pathlist("XDG_CONFIG_DIRS", &[PathBuf::from("/etc/xdg")]);

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
	);
}
