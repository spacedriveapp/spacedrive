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
	if client.indexer_rule().count(vec![]).exec().await? > 0 {
		return Ok(());
	}

	let mut no_os_globs = [
		vec![
			"**/.spacedrive",
		],
		// https://github.com/github/gitignore/blob/main/Global/Windows.gitignore
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
			// Windows can create a swapfile on any drive
			"[A-Z]:/swapfile.sys",
			// NTFS internal files, can exists on any drive
			"[A-Z]:/System Volume Information",
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
	.collect::<Result<Vec<Glob>, _>>().map_err(IndexerError::GlobBuilderError)?;

	if cfg!(windows) {
		// https://en.wikipedia.org/wiki/Directory_structure#Windows_10
		let mut windows_globs = [
			// User special files
			"/Users/*/NTUSER.DAT*",
			"/Users/*/ntuser.dat*",
			"/Users/*/{ntuser.ini,ntuser.dat,NTUSER.DAT}",
			// User special folders (most of these the user dont even have permission to access)
			"/Users/*/{Cookies,AppData,NetHood,Recent,PrintHood,SendTo,Templates,Start Menu,Application Data,Local Settings}",
			// System special folders
			"/{$Recycle.Bin,$WinREAgent,Documents and Settings,Program Files,Program Files (x86),ProgramData,Recovery,PerfLogs,Windows,Windows.old}",
			// System special files
			"/{config,pagefile,hiberfil,swapfile}.sys",
			"/DumpStack.log.tmp",
			// NTFS internal files
			"/System Volume Information",
		].into_iter().flat_map(|g| {
			// Need both C:/ and / matches because globset treat them differently
			[Glob::new(g), Glob::new(format!("C:{}", g).as_str())]
		}).collect::<Result<Vec<_>, _>>().map_err(IndexerError::GlobBuilderError)?;

		no_os_globs.append(&mut windows_globs);
	}

	for rule in [
		IndexerRule::new(
			RuleKind::RejectFilesByGlob,
			// TODO: On windows, beside the listed files, any file with the FILE_ATTRIBUTE_SYSTEM should be considered a system file
			// https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants#FILE_ATTRIBUTE_SYSTEM
			"No OS protected".to_string(),
			true,
			ParametersPerKind::RejectFilesByGlob(no_os_globs),
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

	Ok(())
}
