//! System detection and platform information

use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
	Linux,
	MacOS,
	Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
	X86_64,
	Aarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Libc {
	Glibc,
	Musl,
}

pub struct SystemInfo {
	pub os: Os,
	pub arch: Arch,
	pub libc: Option<Libc>,
}

impl SystemInfo {
	pub fn detect() -> Result<Self> {
		let os = if cfg!(target_os = "linux") {
			Os::Linux
		} else if cfg!(target_os = "macos") {
			Os::MacOS
		} else if cfg!(target_os = "windows") {
			Os::Windows
		} else {
			anyhow::bail!("Unsupported operating system");
		};

		let arch = if cfg!(target_arch = "x86_64") {
			Arch::X86_64
		} else if cfg!(target_arch = "aarch64") {
			Arch::Aarch64
		} else {
			anyhow::bail!("Unsupported architecture");
		};

		// Detect libc on Linux
		let libc = if os == Os::Linux {
			Some(detect_libc()?)
		} else {
			None
		};

		Ok(SystemInfo { os, arch, libc })
	}

	pub fn native_deps_filename(&self) -> String {
		match (self.os, self.arch, self.libc) {
			(Os::Linux, Arch::X86_64, Some(Libc::Musl)) => {
				"native-deps-x86_64-linux-musl.tar.xz".to_string()
			}
			(Os::Linux, Arch::X86_64, Some(Libc::Glibc)) => {
				"native-deps-x86_64-linux-gnu.tar.xz".to_string()
			}
			(Os::Linux, Arch::Aarch64, Some(Libc::Musl)) => {
				"native-deps-aarch64-linux-musl.tar.xz".to_string()
			}
			(Os::Linux, Arch::Aarch64, Some(Libc::Glibc)) => {
				"native-deps-aarch64-linux-gnu.tar.xz".to_string()
			}
			(Os::MacOS, Arch::X86_64, _) => "native-deps-x86_64-darwin-apple.tar.xz".to_string(),
			(Os::MacOS, Arch::Aarch64, _) => "native-deps-aarch64-darwin-apple.tar.xz".to_string(),
			(Os::Windows, Arch::X86_64, _) => "native-deps-x86_64-windows-gnu.tar.xz".to_string(),
			(Os::Windows, Arch::Aarch64, _) => "native-deps-aarch64-windows-gnu.tar.xz".to_string(),
			_ => panic!("Unsupported platform combination"),
		}
	}
}

fn detect_libc() -> Result<Libc> {
	// Try to detect if we're on musl or glibc
	let output = Command::new("ldd").arg("--version").output();

	if let Ok(output) = output {
		let stdout = String::from_utf8_lossy(&output.stdout);
		if stdout.contains("musl") {
			return Ok(Libc::Musl);
		}
	}

	// Default to glibc on Linux
	Ok(Libc::Glibc)
}

/// Get list of installed Rust targets
pub fn get_rust_targets() -> Result<Vec<String>> {
	let output = Command::new("rustup")
		.args(["target", "list", "--installed"])
		.output()?;

	if !output.status.success() {
		anyhow::bail!("Failed to get rustup targets");
	}

	let targets = String::from_utf8(output.stdout)?
		.lines()
		.map(|s| s.trim().to_string())
		.collect();

	Ok(targets)
}

/// Check if a specific linker is available
pub fn has_linker(name: &str) -> bool {
	which::which(name).is_ok()
}

/// Get the best available linker for the platform
pub fn get_best_linker() -> Option<String> {
	if cfg!(target_os = "linux") {
		if has_linker("clang") {
			if has_linker("mold") {
				return Some("mold".to_string());
			} else if has_linker("lld") {
				return Some("lld".to_string());
			}
		}
	} else if cfg!(target_os = "windows")
		&& has_linker("lld-link") {
			return Some("lld-link".to_string());
		}
	None
}
