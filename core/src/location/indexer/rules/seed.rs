use crate::{
	library::Library,
	location::indexer::rules::{IndexerRule, IndexerRuleError, RulePerKind},
};
use chrono::Utc;
use sd_prisma::prisma::indexer_rule;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum SeederError {
	#[error("Failed to run indexer rules seeder: {0}")]
	IndexerRules(#[from] IndexerRuleError),
	#[error("An error occurred with the database while applying migrations: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
}

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
pub async fn new_or_existing_library(library: &Library) -> Result<(), SeederError> {
	// DO NOT REORDER THIS ARRAY!
	for (i, rule) in [no_os_protected(), no_hidden(), no_git(), only_images()]
		.into_iter()
		.enumerate()
	{
		let pub_id = sd_utils::uuid_to_bytes(Uuid::from_u128(i as u128));
		let rules = rmp_serde::to_vec_named(&rule.rules).map_err(IndexerRuleError::from)?;

		use indexer_rule::*;

		let data = vec![
			name::set(Some(rule.name.to_string())),
			rules_per_kind::set(Some(rules.clone())),
			default::set(Some(rule.default)),
			date_created::set(Some(Utc::now().into())),
			date_modified::set(Some(Utc::now().into())),
		];

		library
			.db
			.indexer_rule()
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
                        // User special folders (most of these the user dont even have permission to access)
                        "C:/Users/*/{Cookies,AppData,NetHood,Recent,PrintHood,SendTo,Templates,Start Menu,Application Data,Local Settings}",
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

pub fn no_hidden() -> SystemIndexerRule {
	SystemIndexerRule {
		name: "No Hidden",
		default: true,
		rules: vec![RulePerKind::new_reject_files_by_globs_str(["**/.*"])
			.expect("this is hardcoded and should always work")],
	}
}

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
