use crate::{
	location::indexer::{
		rules::{IndexerRule, ParametersPerKind, RuleKind},
		IndexerError,
	},
	prisma::PrismaClient,
};
use globset::Glob;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SeederError {
	#[error("Failed to run indexer rules seeder: {0}")]
	IndexerRules(#[from] IndexerError),
	#[error("An error occurred with the database while applying migrations: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
}

pub async fn indexer_rules_seeder(client: &PrismaClient) -> Result<(), SeederError> {
	if client.indexer_rule().count(vec![]).exec().await? == 0 {
		for rule in [
			IndexerRule::new(
				RuleKind::RejectFilesByGlob,
				// TODO: On windows, beside the listed files, any file with the FILE_ATTRIBUTE_SYSTEM should be considered a system file
				// https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants#FILE_ATTRIBUTE_SYSTEM
				"No OS protected".to_string(),
				true,
				ParametersPerKind::RejectFilesByGlob([
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
						"/{System,Network,Library,Applications}",
						"/Users/*/{Library,Applications}",
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
				.map(Glob::new)
				.collect::<Result<Vec<Glob>, _>>().map_err(IndexerError::GlobBuilderError)?),
			),
			IndexerRule::new(
				RuleKind::RejectFilesByGlob,
				"No Hidden".to_string(),
				true,
				ParametersPerKind::RejectFilesByGlob(vec![
					Glob::new("**/.*").map_err(IndexerError::GlobBuilderError)?
				]),
			),
			IndexerRule::new(
				RuleKind::AcceptIfChildrenDirectoriesArePresent,
				"Only Git Repositories".into(),
				false,
				ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(
					[".git".to_string()].into_iter().collect(),
				),
			),
			IndexerRule::new(
				RuleKind::AcceptFilesByGlob,
				"Only Images".to_string(),
				false,
				ParametersPerKind::AcceptFilesByGlob(vec![Glob::new(
					"*.{avif,bmp,gif,ico,jpeg,jpg,png,svg,tif,tiff,webp}",
				)
				.map_err(IndexerError::GlobBuilderError)?]),
			),
		] {
			rule.save(client).await?;
		}
	}

	Ok(())
}
