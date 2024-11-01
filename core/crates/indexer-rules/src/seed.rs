use sd_prisma::prisma::{indexer_rule, PrismaClient};

use std::{
	path::{Path, PathBuf},
	sync::LazyLock,
};

use chrono::Utc;
use futures_concurrency::future::Join;
use gix_ignore::{glob::search::pattern::List, search::Ignore, Search};
use tokio::fs;
use uuid::Uuid;

use super::{Error, IndexerRule, RulePerKind};

#[derive(thiserror::Error, Debug)]
pub enum SeederError {
	#[error("Failed to run indexer rules seeder: {0}")]
	IndexerRules(#[from] Error),
	#[error("An error occurred with the database while applying migrations: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("Failed to parse indexer rules based on external system")]
	InheritedExternalRules,
}

#[derive(Debug)]
pub struct GitIgnoreRules {
	rules: RulePerKind,
}

impl GitIgnoreRules {
	pub async fn get_rules_if_in_git_repo(
		location_root: &Path,
		current: &Path,
	) -> Option<Result<Self, SeederError>> {
		let mut git_repo = None;

		let mut ignores = Vec::new();

		for ancestor in current
			.ancestors()
			.take_while(|&path| path.starts_with(location_root))
		{
			let git_ignore = ancestor.join(".gitignore");

			// consider any `.gitignore` files that are inside a git repo
			if matches!(fs::try_exists(&git_ignore).await, Ok(true)) {
				ignores.push(git_ignore);
			}

			if Self::is_git_repo(ancestor).await {
				git_repo.replace(ancestor);
				break;
			}
		}

		let git_repo = git_repo?;
		Some(Self::parse_git_repo(git_repo, ignores).await)
	}

	async fn parse_git_repo(
		git_repo: &Path,
		git_ignores: Vec<PathBuf>,
	) -> Result<Self, SeederError> {
		let mut search = Search::default();

		let git_ignores = git_ignores
			.into_iter()
			.map(Self::parse_git_ignore)
			.collect::<Vec<_>>()
			.join()
			.await;
		search
			.patterns
			.extend(git_ignores.into_iter().filter_map(Result::ok));

		let git_exclude_rules = Self::parse_git_exclude(git_repo.join(".git")).await;
		if let Ok(rules) = git_exclude_rules {
			search.patterns.extend(rules);
		}

		Ok(Self {
			rules: RulePerKind::IgnoredByGit(git_repo.to_owned(), search),
		})
	}

	async fn parse_git_ignore(gitignore: PathBuf) -> Result<List<Ignore>, SeederError> {
		tokio::task::spawn_blocking(move || {
			let mut buf = Vec::with_capacity(30);
			if let Ok(Some(patterns)) = List::from_file(gitignore, None, true, &mut buf) {
				Ok(patterns)
			} else {
				Err(SeederError::InheritedExternalRules)
			}
		})
		.await
		.map_err(|_| SeederError::InheritedExternalRules)?
	}

	async fn parse_git_exclude(dot_git: PathBuf) -> Result<Vec<List<Ignore>>, SeederError> {
		tokio::task::spawn_blocking(move || {
			let mut buf = Vec::new();
			Search::from_git_dir(dot_git.as_ref(), None, &mut buf)
				.map(|search| search.patterns)
				.map_err(|_| SeederError::InheritedExternalRules)
		})
		.await
		.map_err(|_| SeederError::InheritedExternalRules)?
	}

	async fn is_git_repo(path: &Path) -> bool {
		let path = path.join(".git");
		tokio::task::spawn_blocking(move || path.is_dir())
			.await
			.unwrap_or_default()
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
			rules: vec![git.rules],
		}
	}
}

#[derive(Debug)]
pub struct SystemIndexerRule {
	name: &'static str,
	rules: Vec<RulePerKind>,
	default: bool,
}

impl PartialEq<IndexerRule> for SystemIndexerRule {
	fn eq(&self, other: &IndexerRule) -> bool {
		self.name == other.name
	}
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

impl From<&SystemIndexerRule> for IndexerRule {
	fn from(rule: &SystemIndexerRule) -> Self {
		Self {
			id: None,
			name: rule.name.to_string(),
			default: rule.default,
			rules: rule.rules.clone(),
			date_created: Utc::now(),
			date_modified: Utc::now(),
		}
	}
}

/// Seeds system indexer rules into a new or existing library,
pub async fn new_or_existing_library(db: &PrismaClient) -> Result<(), SeederError> {
	use indexer_rule::{date_created, date_modified, default, name, rules_per_kind};

	// DO NOT REORDER THIS ARRAY!
	for (i, rule) in [
		&NO_SYSTEM_FILES,
		&NO_HIDDEN,
		&NO_GIT,
		&GITIGNORE,
		&ONLY_IMAGES,
	]
	.into_iter()
	.enumerate()
	{
		let pub_id = sd_utils::uuid_to_bytes(&Uuid::from_u128(i as u128));
		let rules = rmp_serde::to_vec_named(&rule.rules).map_err(Error::from)?;

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

pub static NO_SYSTEM_FILES: LazyLock<SystemIndexerRule> = LazyLock::new(|| {
	SystemIndexerRule {
	// TODO: On windows, beside the listed files, any file with the FILE_ATTRIBUTE_SYSTEM should be considered a system file
	// https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants#FILE_ATTRIBUTE_SYSTEM
	name: "No System files",
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
});

pub static NO_HIDDEN: LazyLock<SystemIndexerRule> = LazyLock::new(|| SystemIndexerRule {
	name: "No Hidden files",
	default: false,
	rules: vec![RulePerKind::new_reject_files_by_globs_str(["**/.*"])
		.expect("this is hardcoded and should always work")],
});

pub static NO_GIT: LazyLock<SystemIndexerRule> = LazyLock::new(|| SystemIndexerRule {
	name: "No Git files",
	default: true,
	rules: vec![RulePerKind::new_reject_files_by_globs_str([
		"**/{.git,.gitignore,.gitattributes,.gitkeep,.gitconfig,.gitmodules}",
	])
	.expect("this is hardcoded and should always work")],
});

pub static GITIGNORE: LazyLock<SystemIndexerRule> = LazyLock::new(|| SystemIndexerRule {
	name: "Gitignore",
	default: true,
	// Empty rules because this rule is only used to allow frontend to toggle GitIgnoreRules
	rules: vec![],
});

pub static ONLY_IMAGES: LazyLock<SystemIndexerRule> = LazyLock::new(|| SystemIndexerRule {
	name: "Only Images",
	default: false,
	rules: vec![RulePerKind::new_accept_files_by_globs_str([
		"*.{avif,bmp,gif,ico,jpeg,jpg,png,svg,tif,tiff,webp}",
	])
	.expect("this is hardcoded and should always work")],
});
