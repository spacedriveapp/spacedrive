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
			// `No OS protected` must be first indexer rule, because the first indexer rule is enabled by default in the UI
			IndexerRule::new(
				RuleKind::RejectFilesByGlob,
				// TODO: On windows, beside the listed files, any file with the FILE_ATTRIBUTE_SYSTEM should be considered a system file
				// https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants#FILE_ATTRIBUTE_SYSTEM
				"No OS protected".to_string(),
				ParametersPerKind::RejectFilesByGlob([
					vec![
						Glob::new("**/.spacedrive"),
					],
					// https://github.com/github/gitignore/blob/main/Global/Windows.gitignore
					// https://en.wikipedia.org/wiki/Directory_structure#Windows_10
					#[cfg(target_os = "windows")]
					vec![
						// Windows thumbnail cache files
						Glob::new("**/{Thumbs.db,Thumbs.db:encryptable,ehthumbs.db,ehthumbs_vista.db}"),
						// Dump file
						Glob::new("**/*.stackdump"),
						// Folder config file
						Glob::new("**/[Dd]esktop.ini"),
						// Recycle Bin used on file shares
						Glob::new("**/$RECYCLE.BIN"),
						// Chkdsk recovery directory
						Glob::new("**/FOUND.[0-9][0-9][0-9]"),
						// Need both C:/ and / matches because globset treat them differently
						Glob::new("C:/{$WinREAgent,Program Files,Program Files (x86),ProgramData,Recovery,PerfLogs,Windows,Windows.old}"),
						Glob::new("/{$WinREAgent,Program Files,Program Files (x86),ProgramData,Recovery,PerfLogs,Windows,Windows.old}"),
						// Windows can create a swapfile on any drive
						Glob::new("[A-Z]:/swapfile.sys"),
						Glob::new("C:/{config,pagefile,hiberfil}.sys"),
						Glob::new("/{config,pagefile,hiberfil,swapfile}.sys"),
						// NTFS internal files, can exists on any drive
						Glob::new("[A-Z]:/System Volume Information"),
						Glob::new("/System Volume Information"),
					],
					// https://github.com/github/gitignore/blob/main/Global/macOS.gitignore
					// https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html#//apple_ref/doc/uid/TP40010672-CH2-SW14
					#[cfg(any(target_os = "ios", target_os = "macos"))]
					vec![
						Glob::new("**/.{DS_Store,AppleDouble,LSOverride}"),
						// Icon must end with two \r
						Glob::new("**/Icon\r\r"),
						// Thumbnails
						Glob::new("**/._*"),
					],
					#[cfg(target_os = "macos")]
					vec![
						Glob::new("/{System,Network,Library,Applications}"),
						Glob::new("/Users/*/{Library,Applications}"),
						// Files that might appear in the root of a volume
						Glob::new("**/.{DocumentRevisions-V100,fseventsd,Spotlight-V100,TemporaryItems,Trashes,VolumeIcon.icns,com.apple.timemachine.donotpresent}"),
						// Directories potentially created on remote AFP share
						Glob::new("**/.{AppleDB,AppleDesktop,apdisk}"),
						Glob::new("**/{Network Trash Folder,Temporary Items}"),
					],
					// https://github.com/github/gitignore/blob/main/Global/Linux.gitignore
					#[cfg(target_os = "linux")]
					vec![
						Glob::new("**/*~"),
						// temporary files which can be created if a process still has a handle open of a deleted file
						Glob::new("**/.fuse_hidden*"),
						// KDE directory preferences
						Glob::new("**/.directory"),
						// Linux trash folder which might appear on any partition or disk
						Glob::new("**/.Trash-*"),
						// .nfs files are created when an open file is removed but is still being accessed
						Glob::new("**/.nfs*"),
					],
					#[cfg(target_os = "android")]
					vec![
						Glob::new("**/.nomedia"),
						Glob::new("**/.thumbnails"),
					],
					#[cfg(target_family = "unix")]
					// https://en.wikipedia.org/wiki/Unix_filesystem#Conventional_directory_layout
					// https://en.wikipedia.org/wiki/Filesystem_Hierarchy_Standard
					vec![
						// Directories containing unix memory/device mapped files/dirs
						Glob::new("/{dev,sys,proc}"),
						// Directories containing special files for current running programs
						Glob::new("/{run,var,boot}"),
						// ext2-4 recovery directory
						Glob::new("**/lost+found"),
					],
				]
				.into_iter()
				.flatten()
				.collect::<Result<Vec<Glob>, _>>().map_err(IndexerError::GlobBuilderError)?),
			),
			IndexerRule::new(
				RuleKind::RejectFilesByGlob,
				"No Hidden".to_string(),
				ParametersPerKind::RejectFilesByGlob(
					vec![Glob::new("**/.*").map_err(IndexerError::GlobBuilderError)?],
				),
			),
			IndexerRule::new(
				RuleKind::AcceptIfChildrenDirectoriesArePresent,
				"Only Git Repositories".into(),
				ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(
					[".git".to_string()].into_iter().collect(),
				),
			),
			IndexerRule::new(
				RuleKind::AcceptFilesByGlob,
				"Only Images".to_string(),
				ParametersPerKind::AcceptFilesByGlob(vec![
					Glob::new("*.{avif,bmp,gif,ico,jpeg,jpg,png,svg,tif,tiff,webp}")
					.map_err(IndexerError::GlobBuilderError)?
				]),
			),
		] {
			rule.save(client).await?;
		}
	}

	Ok(())
}
