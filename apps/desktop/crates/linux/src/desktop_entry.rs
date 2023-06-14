use std::{
	collections::HashMap,
	convert::TryFrom,
	ffi::OsString,
	path::{Path, PathBuf},
	process::{Command, Stdio},
	str::FromStr,
};

use aho_corasick::AhoCorasick;
use mime::Mime;

use crate::{Error, Result, SystemApps};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopEntry {
	pub name: String,
	pub exec: String,
	pub file_name: OsString,
	pub terminal: bool,
	pub mimes: Vec<Mime>,
	pub categories: HashMap<String, ()>,
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum Mode {
	Launch,
	Open,
}

fn terminal() -> Result<String> {
	SystemApps::get_entries()
		.ok()
		.and_then(|mut entries| {
			entries.find(|(_handler, entry)| entry.categories.contains_key("TerminalEmulator"))
		})
		.map(|e| e.1.exec)
		.ok_or(Error::NoTerminal)
}

impl DesktopEntry {
	pub fn exec(&self, mode: Mode, arguments: &[&str]) -> Result<()> {
		let supports_multiple = self.exec.contains("%F") || self.exec.contains("%U");
		if arguments.is_empty() {
			self.exec_inner(&[])?
		} else if supports_multiple || mode == Mode::Launch {
			self.exec_inner(arguments)?;
		} else {
			for arg in arguments {
				self.exec_inner(&[*arg])?;
			}
		};

		Ok(())
	}

	fn exec_inner(&self, args: &[&str]) -> Result<()> {
		let mut cmd = {
			let (cmd, args) = self.get_cmd(args)?;
			let mut cmd = Command::new(cmd);
			cmd.args(args);
			cmd
		};

		if self.terminal && atty::is(atty::Stream::Stdout) {
			cmd.spawn()?.wait()?;
		} else {
			cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
		}

		Ok(())
	}

	pub fn get_cmd(&self, args: &[&str]) -> Result<(String, Vec<String>)> {
		let special = AhoCorasick::new(["%f", "%F", "%u", "%U"]).expect("Failed to build pattern");

		let mut exec = shlex::split(&self.exec).ok_or(Error::InvalidExec(self.exec.clone()))?;

		// The desktop entry doesn't contain arguments - we make best effort and append them at
		// the end
		if special.is_match(&self.exec) {
			exec = exec
				.into_iter()
				.flat_map(|s| match s.as_str() {
					"%f" | "%F" | "%u" | "%U" => {
						args.iter().map(|arg| str::to_string(arg)).collect()
					}
					s if special.is_match(s) => vec![{
						let mut replaced = String::with_capacity(s.len() + args.len() * 2);
						special.replace_all_with(s, &mut replaced, |_, _, dst| {
							dst.push_str(args.join(" ").as_str());
							false
						});
						replaced
					}],
					_ => vec![s],
				})
				.collect()
		} else {
			exec.extend(args.iter().map(|arg| str::to_string(arg)));
		}

		// If the entry expects a terminal (emulator), but this process is not running in one, we
		// launch a new one.
		if self.terminal && !atty::is(atty::Stream::Stdout) {
			exec = shlex::split(&terminal()?)
				.ok_or(Error::InvalidExec(self.exec.clone()))?
				.into_iter()
				.chain(["-e".to_owned()])
				.chain(exec)
				.collect();
		}

		Ok((exec.remove(0), exec))
	}
}

fn parse_file(path: &Path) -> Option<DesktopEntry> {
	let raw_entry = freedesktop_entry_parser::parse_entry(path).ok()?;
	let section = raw_entry.section("Desktop Entry");

	let mut entry = DesktopEntry {
		file_name: path.file_name()?.to_owned(),
		..Default::default()
	};

	for attr in section.attrs().filter(|a| a.has_value()) {
		match attr.name {
			"Name" if entry.name.is_empty() => {
				entry.name = attr.value?.into();
			}
			"Exec" => entry.exec = attr.value?.into(),
			"MimeType" => {
				entry.mimes = attr
					.value?
					.split(';')
					.filter_map(|m| Mime::from_str(m).ok())
					.collect::<Vec<_>>();
			}
			"Terminal" => entry.terminal = attr.value? == "true",
			"Categories" => {
				entry.categories = attr
					.value?
					.split(';')
					.filter(|s| !s.is_empty())
					.map(|cat| (cat.to_owned(), ()))
					.collect();
			}
			_ => {}
		}
	}

	if !entry.name.is_empty() && !entry.exec.is_empty() {
		Some(entry)
	} else {
		None
	}
}

impl TryFrom<&PathBuf> for DesktopEntry {
	type Error = Error;
	fn try_from(path: &PathBuf) -> Result<DesktopEntry> {
		parse_file(path).ok_or(Error::BadEntry(path.clone()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn complex_exec() {
		let entry = parse_file(Path::new("tests/cmus.desktop")).unwrap();
		assert_eq!(entry.mimes.len(), 2);
		assert_eq!(entry.mimes[0].essence_str(), "audio/mp3");
		assert_eq!(entry.mimes[1].essence_str(), "audio/ogg");
	}
}
