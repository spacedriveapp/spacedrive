use std::{
	fs::{File, OpenOptions},
	io::{self, Read, Seek, SeekFrom},
	path::{Path, PathBuf},
};

/// Cap on one service log before it is rotated. A debug-level daemon can
/// produce hundreds of megabytes a day, and the data directory is not the
/// place to discover that.
const MAX_BYTES: u64 = 16 * 1_048_576;
/// How much of the tail to read when answering for the last N lines. Large
/// enough that a full request is served from one slice, bounded so the answer
/// never costs the size of the file.
const TAIL_WINDOW_BYTES: u64 = 1_048_576;

/// One log file per owned service under the supervisor's log directory.
#[derive(Debug, Clone)]
pub struct ServiceLogs {
	dir: PathBuf,
}

impl ServiceLogs {
	pub fn new(dir: impl Into<PathBuf>) -> Self {
		Self { dir: dir.into() }
	}

	pub fn init(&self) -> io::Result<()> {
		std::fs::create_dir_all(&self.dir)
	}

	pub fn path(&self, name: &str) -> PathBuf {
		self.dir.join(format!("{name}.log"))
	}

	/// Owned children hold the file for their lifetime; append so a restart
	/// doesn't erase the previous run's tail.
	pub fn open_append(&self, name: &str) -> io::Result<File> {
		OpenOptions::new()
			.create(true)
			.append(true)
			.open(self.path(name))
	}

	/// Copy the log aside and truncate it in place once it passes the cap.
	/// Renaming would strand the writer: an owned child holds its file for
	/// its lifetime, so it would keep filling an unlinked inode and the space
	/// would never come back. Truncation works because the file is opened in
	/// append mode — every write seeks to end-of-file, so output resumes at
	/// zero. Lines written during the copy are the accepted cost of not
	/// restarting the child.
	pub fn rotate(&self, name: &str) -> io::Result<bool> {
		let path = self.path(name);
		let size = match std::fs::metadata(&path) {
			Ok(meta) => meta.len(),
			Err(_) => return Ok(false),
		};
		if size <= MAX_BYTES {
			return Ok(false);
		}
		std::fs::copy(&path, rotated_path(&path))?;
		OpenOptions::new().write(true).open(&path)?.set_len(0)?;
		Ok(true)
	}

	/// The last `lines` lines, read from the end of the file rather than the
	/// whole of it — a service log runs to the rotation cap, and answering
	/// for its last few lines must not cost that much memory.
	pub fn tail(&self, name: &str, lines: usize) -> io::Result<Vec<String>> {
		let path = self.path(name);
		let mut file = match File::open(&path) {
			Ok(file) => file,
			Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
			Err(err) => return Err(err),
		};
		let size = file.metadata()?.len();
		let from = size.saturating_sub(TAIL_WINDOW_BYTES);
		file.seek(SeekFrom::Start(from))?;
		let mut window = String::new();
		file.read_to_string(&mut window)?;

		let mut all: Vec<&str> = window.split('\n').collect();
		if all.last() == Some(&"") {
			all.pop();
		}
		// The window almost certainly opened mid-line; that fragment is not
		// a line.
		if from > 0 && !all.is_empty() {
			all.remove(0);
		}
		let start = all.len().saturating_sub(lines);
		Ok(all[start..].iter().map(|line| line.to_string()).collect())
	}
}

fn rotated_path(path: &Path) -> PathBuf {
	let mut rotated = path.as_os_str().to_owned();
	rotated.push(".1");
	PathBuf::from(rotated)
}
