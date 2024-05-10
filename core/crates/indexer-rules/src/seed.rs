use std::path::Path;

use futures_concurrency::future::Join;
use globset::{Glob, GlobMatcher};
use sd_prisma::prisma::{indexer_rule, PrismaClient};

use chrono::Utc;
use thiserror::Error;
use tokio::{
	fs::{self, File},
	io::{BufReader, Lines},
};
use uuid::Uuid;

use super::{IndexerRule, IndexerRuleError, RulePerKind};

#[derive(Error, Debug)]
pub enum SeederError {
	#[error("Failed to run indexer rules seeder: {0}")]
	IndexerRules(#[from] IndexerRuleError),
	#[error("An error occurred with the database while applying migrations: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("Failed to parse indexer rules based on external system")]
	InhirentedExternalRules,
}

#[derive(Debug)]
pub struct GitIgnoreRules {
	rules: Vec<RulePerKind>,
}

impl GitIgnoreRules {
	pub async fn parse_if_gitrepo(path: &Path) -> Result<Self, SeederError> {
		let is_git = Self::is_git_repo(path);
		let has_gitignore = fs::try_exists(path.join(".gitignore"));

		let (is_git, has_gitignore) = (is_git, has_gitignore).join().await;
		if !(is_git && matches!(has_gitignore, Ok(true))) {
			return Err(SeederError::InhirentedExternalRules);
		}

		// TODO(matheus-consoli): extend the functionality to also consider other git ignore sources
		// `[gitignore, ...].map(parse_ignore_rules).collect().join().await` or something
		// see `https://git-scm.com/docs/gitignore` for other ignore sources

		Self::parse_ignore_rules(path, &path.join(".gitignore")).await
	}

	/// Parses the git ignore rules from a given file path
	pub async fn parse_ignore_rules(
		base_dir: &Path,
		gitignore: &Path,
	) -> Result<Self, SeederError> {
		use tokio::io::AsyncBufReadExt;

		let file = File::open(gitignore)
			.await
			.map_err(|_| SeederError::InhirentedExternalRules)?;

		let buf = BufReader::new(file);
		let mut lines = buf.lines();

		let mut ignored_star_globs = Vec::new();
		let mut negated_rules = Vec::new();

		let mut rules = Vec::new();
		while let Ok(Some(mut line)) = Self::next_line(&mut lines).await {
			// A blank line; skip
			if line.is_empty() {
				continue;
			}

			// A line starting with "#" serves as a comment; skip
			if line.starts_with('#') {
				continue;
			}

			// an optional "!" negates the pattern
			// any matching file excluded by a previous pattern will become included again
			// it's not possible to re-include a file if a pattern directory of that file is excluded
			let rule = if line.starts_with('!') {
				// TODO(matheus-consoli): support negated patterns (`!path/to/file`)
				// as of the time of writing, the indexer doesn't handle well usages of acceptance and rejection
				// when they are rules mixed.
				// As an example:
				// ```gitignore
				// docs/*.md
				// !docs/readme.md
				// ```
				// we create two rules for it:
				// - rejecting all `path/to/docs/*.md` (including `path/to/docs/readme.md`)
				//   this rule approves every file inside the git repo, except for the `docs/*.md` files
				// - accepting `path/to/readme.md`
				//   this REJECTS every file except for `docs/readme.md` (which has already been by the other rule)

				let full = base_dir.join(line);
				let Ok(file) = full.into_os_string().into_string() else {
					continue;
				};
				negated_rules.extend(file.parse::<Glob>());
				continue;
			} else {
				if line.starts_with('/') {
					line.remove(0);
				}
				let full = base_dir.join(&line);

				// ignore the rule if it's poorly formatted or invalid
				let Ok(file) = full.into_os_string().into_string() else {
					continue;
				};

				if line.contains("*") {
					ignored_star_globs.extend(file.parse::<Glob>());
					continue;
				}

				let Ok(rule) = RulePerKind::new_reject_files_by_globs_str([file]) else {
					continue;
				};
				rule
			};

			rules.push(rule);
		}

		// skip star rules that matches a negated pattern
		// ```example
		// *
		// !src
		// ```
		if !negated_rules.is_empty() {
			let ignored_negated_matches: Vec<GlobMatcher> =
				negated_rules.iter().map(|i| i.compile_matcher()).collect();

			ignored_star_globs.retain(|star_glob| {
				let star = star_glob.compile_matcher();
				let star_glob = star_glob.glob();
				negated_rules
					.iter()
					.zip(ignored_negated_matches.iter())
					.any(|(a, b)| !(star.is_match(a.glob()) || b.is_match(star_glob)))
			});
		}
		rules.extend(
			ignored_star_globs
				.into_iter()
				.filter_map(|rule| RulePerKind::new_reject_files_by_globs_str([rule.glob()]).ok()),
		);

		Ok(Self { rules })
	}

	async fn is_git_repo(path: &Path) -> bool {
		let path = path.join(".git");
		tokio::task::spawn_blocking(move || path.is_dir())
			.await
			.unwrap_or_default()
	}

	/// Read a line from the stream source and joins multi-lines into a single string
	async fn next_line(stream: &mut Lines<BufReader<File>>) -> Result<Option<String>, SeederError> {
		use std::ops::Not;
		let mut line = String::new();

		loop {
			let next = stream
				.next_line()
				.await
				.map_err(|_| SeederError::InhirentedExternalRules)?;

			if let Some(next) = next {
				line.push_str(next.trim());
				if line.ends_with('\\') {
					line.remove(line.len() - 1);
					// same as an in-place `line.trim_end()`, but without reallocation
					line.truncate(line.trim_end().len());
					// read and merge the next line
					continue;
				}
				break Ok(Some(line));
			}
			break Ok(line.is_empty().not().then_some(line));
		}
	}
}

impl From<GitIgnoreRules> for IndexerRule {
	fn from(git: GitIgnoreRules) -> Self {
		Self {
			id: None,
			name: ".gitignore'd".to_owned(),
			default: true,
			date_created: Utc::now(),
			date_modified: Utc::now(),
			rules: git.rules,
		}
	}
}

#[derive(Debug)]
pub struct SystemIndexerRule {
	name: &'static str,
	rules: Vec<RulePerKind>,
	default: bool,
}

impl From<SystemIndexerRule> for IndexerRule {
	fn from(rule: SystemIndexerRule) -> Self {
		Self {
			id: None,
			name: rule.name.to_string(),
			default: rule.default,
			rules: rule.rules,
			date_created: Utc::now(),
			date_modified: Utc::now(),
		}
	}
}

/// Seeds system indexer rules into a new or existing library,
pub async fn new_or_existing_library(db: &PrismaClient) -> Result<(), SeederError> {
	use indexer_rule::{date_created, date_modified, default, name, rules_per_kind};

	// DO NOT REORDER THIS ARRAY!
	for (i, rule) in [no_os_protected(), no_hidden(), no_git(), only_images()]
		.into_iter()
		.enumerate()
	{
		let pub_id = sd_utils::uuid_to_bytes(Uuid::from_u128(i as u128));
		let rules = rmp_serde::to_vec_named(&rule.rules).map_err(IndexerRuleError::from)?;

		let data = vec![
			name::set(Some(rule.name.to_string())),
			rules_per_kind::set(Some(rules.clone())),
			default::set(Some(rule.default)),
			date_created::set(Some(Utc::now().into())),
			date_modified::set(Some(Utc::now().into())),
		];

		db.indexer_rule()
			.upsert(
				indexer_rule::pub_id::equals(pub_id.clone()),
				indexer_rule::create(pub_id.clone(), data.clone()),
				data,
			)
			.exec()
			.await?;
	}

	Ok(())
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn no_os_protected() -> SystemIndexerRule {
	SystemIndexerRule {
        // TODO: On windows, beside the listed files, any file with the FILE_ATTRIBUTE_SYSTEM should be considered a system file
        // https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants#FILE_ATTRIBUTE_SYSTEM
        name: "No OS protected",
        default: true,
        rules: vec![
            RulePerKind::new_reject_files_by_globs_str(
                [
                    vec![
                        "**/.spacedrive",
                    ],
                    // Globset, even on Windows, requires the use of / as a separator
                    // https://github.com/github/gitignore/blob/main/Global/Windows.gitignore
                    // https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
                    #[cfg(target_os = "windows")]
                    vec![
                        // Windows thumbnail cache files
                        "**/{Thumbs.db,Thumbs.db:encryptable,ehthumbs.db,ehthumbs_vista.db}",
                        // Dump file
                        "**/*.stackdump",
                        // Folder config file
                        "**/[Dd]esktop.ini",
                        // Recycle Bin used on file shares
                        "**/$RECYCLE.BIN",
                        // Chkdsk recovery directory
                        "**/FOUND.[0-9][0-9][0-9]",
                        // Reserved names
                        "**/{CON,PRN,AUX,NUL,COM0,COM1,COM2,COM3,COM4,COM5,COM6,COM7,COM8,COM9,LPT0,LPT1,LPT2,LPT3,LPT4,LPT5,LPT6,LPT7,LPT8,LPT9}",
                        "**/{CON,PRN,AUX,NUL,COM0,COM1,COM2,COM3,COM4,COM5,COM6,COM7,COM8,COM9,LPT0,LPT1,LPT2,LPT3,LPT4,LPT5,LPT6,LPT7,LPT8,LPT9}.*",
                        // User special files
                        "C:/Users/*/NTUSER.DAT*",
                        "C:/Users/*/ntuser.dat*",
                        "C:/Users/*/{ntuser.ini,ntuser.dat,NTUSER.DAT}",
                        // User special folders (most of these the user don't even have permission to access)
                        "C:/Users/*/{Cookies,AppData,NetHood,Recent,PrintHood,SendTo,Templates,Start Menu,Application Data,Local Settings,My Documents}",
                        // System special folders
                        "C:/{$Recycle.Bin,$WinREAgent,Documents and Settings,Program Files,Program Files (x86),ProgramData,Recovery,PerfLogs,Windows,Windows.old}",
                        // NTFS internal dir, can exists on any drive
                        "[A-Z]:/System Volume Information",
                        // System special files
                        "C:/{config,pagefile,hiberfil}.sys",
                        // Windows can create a swapfile on any drive
                        "[A-Z]:/swapfile.sys",
                        "C:/DumpStack.log.tmp",
                    ],
                    // https://github.com/github/gitignore/blob/main/Global/macOS.gitignore
                    // https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html#//apple_ref/doc/uid/TP40010672-CH2-SW14
                    #[cfg(any(target_os = "ios", target_os = "macos"))]
                    vec![
                        "**/.{DS_Store,AppleDouble,LSOverride}",
                        // Icon must end with two \r
                        "**/Icon\r\r",
                        // Thumbnails
                        "**/._*",
                    ],
                    #[cfg(target_os = "macos")]
                    vec![
                        "/{System,Network,Library,Applications,.PreviousSystemInformation,.com.apple.templatemigration.boot-install}",
						"/System/Volumes/Data/{System,Network,Library,Applications,.PreviousSystemInformation,.com.apple.templatemigration.boot-install}",
                        "/Users/*/{Library,Applications}",
                        "/System/Volumes/Data/Users/*/{Library,Applications}",
                        "**/*.photoslibrary/{database,external,private,resources,scope}",
                        // Files that might appear in the root of a volume
                        "**/.{DocumentRevisions-V100,fseventsd,Spotlight-V100,TemporaryItems,Trashes,VolumeIcon.icns,com.apple.timemachine.donotpresent}",
                        // Directories potentially created on remote AFP share
                        "**/.{AppleDB,AppleDesktop,apdisk}",
                        "**/{Network Trash Folder,Temporary Items}",
                    ],
                    // https://github.com/github/gitignore/blob/main/Global/Linux.gitignore
                    #[cfg(target_os = "linux")]
                    vec![
                        "**/*~",
                        // temporary files which can be created if a process still has a handle open of a deleted file
                        "**/.fuse_hidden*",
                        // KDE directory preferences
                        "**/.directory",
                        // Linux trash folder which might appear on any partition or disk
                        "**/.Trash-*",
                        // .nfs files are created when an open file is removed but is still being accessed
                        "**/.nfs*",
                    ],
                    #[cfg(target_os = "android")]
                    vec![
                        "**/.nomedia",
                        "**/.thumbnails",
                    ],
                    // https://en.wikipedia.org/wiki/Unix_filesystem#Conventional_directory_layout
                    // https://en.wikipedia.org/wiki/Filesystem_Hierarchy_Standard
                    #[cfg(target_family = "unix")]
                    vec![
                        // Directories containing unix memory/device mapped files/dirs
                        "/{dev,sys,proc}",
                        // Directories containing special files for current running programs
                        "/{run,var,boot}",
                        // ext2-4 recovery directory
                        "**/lost+found",
                    ],
                ]
                .into_iter()
                .flatten()
            ).expect("this is hardcoded and should always work"),
        ],
    }
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn no_hidden() -> SystemIndexerRule {
	SystemIndexerRule {
		name: "No Hidden",
		default: false,
		rules: vec![RulePerKind::new_reject_files_by_globs_str(["**/.*"])
			.expect("this is hardcoded and should always work")],
	}
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
fn no_git() -> SystemIndexerRule {
	SystemIndexerRule {
		name: "No Git",
		default: false,
		rules: vec![RulePerKind::new_reject_files_by_globs_str([
			"**/{.git,.gitignore,.gitattributes,.gitkeep,.gitconfig,.gitmodules}",
		])
		.expect("this is hardcoded and should always work")],
	}
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
fn only_images() -> SystemIndexerRule {
	SystemIndexerRule {
		name: "Only Images",
		default: false,
		rules: vec![RulePerKind::new_accept_files_by_globs_str([
			"*.{avif,bmp,gif,ico,jpeg,jpg,png,svg,tif,tiff,webp}",
		])
		.expect("this is hardcoded and should always work")],
	}
}
