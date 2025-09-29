use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{DateTime, Utc};
use futures_concurrency::future::TryJoin;
use gix_ignore::{glob::pattern::Case, Search};
use globset::{Glob, GlobSet, GlobSetBuilder};
use once_cell::sync::Lazy;
use rmp::Marker;
use rmp_serde::{decode, encode};
use serde::{de::VariantAccess, Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("invalid indexer rule kind integer: {0}")]
	InvalidRuleKindInt(i32),
	#[error("glob builder error: {0}")]
	Glob(#[from] globset::Error),

	#[error("indexer rule parameters encode error: {0}")]
	RuleParametersRMPEncode(#[from] encode::Error),
	#[error("indexer rule parameters decode error: {0}")]
	RuleParametersRMPDecode(#[from] decode::Error),
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
}

#[repr(i32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum RuleKind {
	AcceptFilesByGlob = 0,
	RejectFilesByGlob = 1,
	AcceptIfChildrenDirectoriesArePresent = 2,
	RejectIfChildrenDirectoriesArePresent = 3,
	IgnoredByGit = 4,
}

impl RuleKind {
	#[must_use]
	pub const fn variant_count() -> usize {
		5
	}
}

#[derive(Debug, Clone)]
pub enum RulePerKind {
	AcceptFilesByGlob(Vec<Glob>, GlobSet),
	RejectFilesByGlob(Vec<Glob>, GlobSet),
	AcceptIfChildrenDirectoriesArePresent(HashSet<String>),
	RejectIfChildrenDirectoriesArePresent(HashSet<String>),
	IgnoredByGit(PathBuf, Search),
}

impl RulePerKind {
	fn new_files_by_globs_str_and_kind(
		globs_str: impl IntoIterator<Item = impl AsRef<str>>,
		kind_fn: impl Fn(Vec<Glob>, GlobSet) -> Self,
	) -> Result<Self, Error> {
		globs_str
			.into_iter()
			.map(|s| s.as_ref().parse::<Glob>())
			.collect::<Result<Vec<_>, _>>()
			.and_then(|globs| {
				globs
					.iter()
					.cloned()
					.fold(&mut GlobSetBuilder::new(), |builder, glob| {
						builder.add(glob)
					})
					.build()
					.map(move |glob_set| kind_fn(globs, glob_set))
					.map_err(Into::into)
			})
			.map_err(Into::into)
	}

	pub fn new_accept_files_by_globs_str(
		globs_str: impl IntoIterator<Item = impl AsRef<str>>,
	) -> Result<Self, Error> {
		Self::new_files_by_globs_str_and_kind(globs_str, Self::AcceptFilesByGlob)
	}

	pub fn new_reject_files_by_globs_str(
		globs_str: impl IntoIterator<Item = impl AsRef<str>>,
	) -> Result<Self, Error> {
		Self::new_files_by_globs_str_and_kind(globs_str, Self::RejectFilesByGlob)
	}
}

pub trait MetadataForIndexerRules: Send + Sync + 'static {
	fn is_dir(&self) -> bool;
}

impl MetadataForIndexerRules for std::fs::Metadata {
	fn is_dir(&self) -> bool {
		self.is_dir()
	}
}

impl RulePerKind {
	pub async fn apply(
		&self,
		source: impl AsRef<Path> + Send,
		metadata: &impl MetadataForIndexerRules,
	) -> Result<(RuleKind, bool), Error> {
		match self {
			Self::AcceptIfChildrenDirectoriesArePresent(children) => {
				accept_dir_for_its_children(source, metadata, children)
					.await
					.map(|accepted| (RuleKind::AcceptIfChildrenDirectoriesArePresent, accepted))
			}
			Self::RejectIfChildrenDirectoriesArePresent(children) => {
				reject_dir_for_its_children(source, metadata, children)
					.await
					.map(|rejected| (RuleKind::RejectIfChildrenDirectoriesArePresent, rejected))
			}
			Self::AcceptFilesByGlob(_globs, accept_glob_set) => Ok((
				RuleKind::AcceptFilesByGlob,
				accept_by_glob(source, accept_glob_set),
			)),
			Self::RejectFilesByGlob(_globs, reject_glob_set) => Ok((
				RuleKind::RejectFilesByGlob,
				reject_by_glob(source, reject_glob_set),
			)),
			Self::IgnoredByGit(base_dir, patterns) => Ok((
				RuleKind::IgnoredByGit,
				accept_by_git_pattern(source, base_dir, patterns),
			)),
		}
	}
}

fn accept_by_git_pattern(
	source: impl AsRef<Path>,
	base_dir: impl AsRef<Path>,
	search: &Search,
) -> bool {
	fn inner(source: &Path, base_dir: &Path, search: &Search) -> bool {
		let relative = match source.strip_prefix(base_dir) {
			Ok(p) => p,
			Err(_) => return true,
		};
		let Some(src) = relative.to_str().map(|s| s.as_bytes().into()) else {
			return false;
		};
		search
			.pattern_matching_relative_path(src, Some(source.is_dir()), Case::Fold)
			.map_or(true, |rule| rule.pattern.is_negative())
	}
	inner(source.as_ref(), base_dir.as_ref(), search)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexerRule {
	pub id: Option<i32>,
	pub name: String,
	pub default: bool,
	pub rules: Vec<RulePerKind>,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
}

impl IndexerRule {
	pub async fn apply(
		&self,
		source: impl AsRef<Path> + Send,
		metadata: &impl MetadataForIndexerRules,
	) -> Result<Vec<(RuleKind, bool)>, Error> {
		self.rules
			.iter()
			.map(|rule| rule.apply(source.as_ref(), metadata))
			.collect::<Vec<_>>()
			.try_join()
			.await
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerDecision {
	Accept,
	Reject,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IndexerRuler {
	base: Arc<Vec<IndexerRule>>,
	extra: Vec<IndexerRule>,
}

impl Clone for IndexerRuler {
	fn clone(&self) -> Self {
		Self {
			base: Arc::clone(&self.base),
			extra: Vec::new(),
		}
	}
}

impl IndexerRuler {
	#[must_use]
	pub fn new(rules: Vec<IndexerRule>) -> Self {
		Self {
			base: Arc::new(rules),
			extra: Vec::new(),
		}
	}

	pub async fn evaluate_path(
		&self,
		source: impl AsRef<Path> + Send,
		metadata: &impl MetadataForIndexerRules,
	) -> Result<RulerDecision, Error> {
		let acceptance_per_rule_kind = self.apply_all(source, metadata).await?;
		Ok(
			if Self::reject_path(Path::new(""), false, &acceptance_per_rule_kind) {
				RulerDecision::Reject
			} else {
				RulerDecision::Accept
			},
		)
	}

	pub async fn apply_all(
		&self,
		source: impl AsRef<Path> + Send,
		metadata: &impl MetadataForIndexerRules,
	) -> Result<HashMap<RuleKind, Vec<bool>>, Error> {
		let results = self
			.base
			.iter()
			.chain(self.extra.iter())
			.map(|rule| rule.apply(source.as_ref(), metadata))
			.collect::<Vec<_>>()
			.try_join()
			.await?;
		Ok(results.into_iter().flatten().fold(
			HashMap::<_, Vec<_>>::with_capacity(RuleKind::variant_count()),
			|mut map, (kind, result)| {
				map.entry(kind).or_default().push(result);
				map
			},
		))
	}

	pub fn extend(&mut self, iter: impl IntoIterator<Item = IndexerRule> + Send) {
		self.extra.extend(iter);
	}

	fn reject_path(
		_current_path: &Path,
		is_dir: bool,
		acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
	) -> bool {
		Self::rejected_by_reject_glob(acceptance_per_rule_kind)
			|| Self::rejected_by_git_ignore(acceptance_per_rule_kind)
			|| (is_dir && Self::rejected_by_children_directories(acceptance_per_rule_kind))
			|| Self::rejected_by_accept_glob(acceptance_per_rule_kind)
	}

	pub fn rejected_by_accept_glob(
		acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
	) -> bool {
		acceptance_per_rule_kind
			.get(&RuleKind::AcceptFilesByGlob)
			.map_or(false, |accept_rules| {
				accept_rules.iter().all(|accept| !accept)
			})
	}

	pub fn rejected_by_children_directories(
		acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
	) -> bool {
		acceptance_per_rule_kind
			.get(&RuleKind::RejectIfChildrenDirectoriesArePresent)
			.map_or(false, |reject_results| {
				reject_results.iter().any(|reject| !reject)
			})
	}

	pub fn rejected_by_reject_glob(
		acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
	) -> bool {
		acceptance_per_rule_kind
			.get(&RuleKind::RejectFilesByGlob)
			.map_or(false, |reject_results| {
				reject_results.iter().any(|reject| !reject)
			})
	}

	pub fn rejected_by_git_ignore(acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>) -> bool {
		acceptance_per_rule_kind
			.get(&RuleKind::IgnoredByGit)
			.map_or(false, |reject_results| {
				reject_results.iter().any(|reject| !reject)
			})
	}
}

// Serialization for RulePerKind (GlobSet is not serializable)
impl Serialize for RulePerKind {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::ser::Serializer,
	{
		use serde::ser;
		match *self {
			Self::AcceptFilesByGlob(ref globs, _) => serializer.serialize_newtype_variant(
				"ParametersPerKind",
				0,
				"AcceptFilesByGlob",
				globs,
			),
			Self::RejectFilesByGlob(ref globs, _) => serializer.serialize_newtype_variant(
				"ParametersPerKind",
				1,
				"RejectFilesByGlob",
				globs,
			),
			Self::AcceptIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					2,
					"AcceptIfChildrenDirectoriesArePresent",
					children,
				),
			Self::RejectIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					3,
					"RejectIfChildrenDirectoriesArePresent",
					children,
				),
			Self::IgnoredByGit(_, _) => {
				unreachable!("git ignore rules are dynamic and not serialized")
			}
		}
	}
}

impl<'de> Deserialize<'de> for RulePerKind {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::de::Deserializer<'de>,
	{
		use serde::de;
		use std::marker::PhantomData;

		const VARIANTS: &[&str] = &[
			"AcceptFilesByGlob",
			"RejectFilesByGlob",
			"AcceptIfChildrenDirectoriesArePresent",
			"RejectIfChildrenDirectoriesArePresent",
		];

		enum Fields {
			AcceptFilesByGlob,
			RejectFilesByGlob,
			AcceptIfChildrenDirectoriesArePresent,
			RejectIfChildrenDirectoriesArePresent,
		}

		struct FieldsVisitor;
		impl de::Visitor<'_> for FieldsVisitor {
			type Value = Fields;
			fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.write_str("`AcceptFilesByGlob` or `RejectFilesByGlob` or `AcceptIfChildrenDirectoriesArePresent` or `RejectIfChildrenDirectoriesArePresent`")
			}
			fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
				Ok(match value {
					"AcceptFilesByGlob" => Fields::AcceptFilesByGlob,
					"RejectFilesByGlob" => Fields::RejectFilesByGlob,
					"AcceptIfChildrenDirectoriesArePresent" => {
						Fields::AcceptIfChildrenDirectoriesArePresent
					}
					"RejectIfChildrenDirectoriesArePresent" => {
						Fields::RejectIfChildrenDirectoriesArePresent
					}
					_ => return Err(E::unknown_variant(value, VARIANTS)),
				})
			}
		}
		impl<'de> Deserialize<'de> for Fields {
			fn deserialize<D2: de::Deserializer<'de>>(d: D2) -> Result<Self, D2::Error> {
				d.deserialize_identifier(FieldsVisitor)
			}
		}

		struct Visitor<'de> {
			marker: PhantomData<RulePerKind>,
			lifetime: PhantomData<&'de ()>,
		}
		impl<'de> de::Visitor<'de> for Visitor<'de> {
			type Value = RulePerKind;
			fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.write_str("enum ParametersPerKind")
			}
			fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
			where
				A: de::EnumAccess<'de>,
			{
				use de::Error;
				let (field, access) = data.variant::<Fields>()?;
				match field {
					Fields::AcceptFilesByGlob => {
						let globs = access.newtype_variant::<Vec<Glob>>()?;
						let mut builder = GlobSetBuilder::new();
						for g in &globs {
							builder.add(g.clone());
						}
						let set = builder.build().map_err(A::Error::custom)?;
						Ok(RulePerKind::AcceptFilesByGlob(globs, set))
					}
					Fields::RejectFilesByGlob => {
						let globs = access.newtype_variant::<Vec<Glob>>()?;
						let mut builder = GlobSetBuilder::new();
						for g in &globs {
							builder.add(g.clone());
						}
						let set = builder.build().map_err(A::Error::custom)?;
						Ok(RulePerKind::RejectFilesByGlob(globs, set))
					}
					Fields::AcceptIfChildrenDirectoriesArePresent => {
						let children = access.newtype_variant::<HashSet<String>>()?;
						Ok(RulePerKind::AcceptIfChildrenDirectoriesArePresent(children))
					}
					Fields::RejectIfChildrenDirectoriesArePresent => {
						let children = access.newtype_variant::<HashSet<String>>()?;
						Ok(RulePerKind::RejectIfChildrenDirectoriesArePresent(children))
					}
				}
			}
		}
		deserializer.deserialize_enum(
			"ParametersPerKind",
			VARIANTS,
			Visitor {
				marker: std::marker::PhantomData,
				lifetime: std::marker::PhantomData,
			},
		)
	}
}

async fn accept_dir_for_its_children(
	source: impl AsRef<Path> + Send,
	metadata: &impl MetadataForIndexerRules,
	children: &HashSet<String>,
) -> Result<bool, Error> {
	if !metadata.is_dir() {
		return Ok(false);
	}
	let mut read_dir = tokio::fs::read_dir(source.as_ref()).await.map_err(|_| {
		Error::RuleParametersRMPDecode(rmp_serde::decode::Error::TypeMismatch(Marker::FixPos(0)))
	})?;
	while let Ok(Some(entry)) = read_dir.next_entry().await {
		let entry_name = match entry.file_name().into_string() {
			Ok(s) => s,
			Err(_) => continue,
		};
		if entry.metadata().await.map(|m| m.is_dir()).unwrap_or(false)
			&& children.contains(&entry_name)
		{
			return Ok(true);
		}
	}
	Ok(false)
}

async fn reject_dir_for_its_children(
	source: impl AsRef<Path> + Send,
	metadata: &impl MetadataForIndexerRules,
	children: &HashSet<String>,
) -> Result<bool, Error> {
	if !metadata.is_dir() {
		return Ok(true);
	}
	let mut read_dir = tokio::fs::read_dir(source.as_ref()).await.map_err(|_| {
		Error::RuleParametersRMPDecode(rmp_serde::decode::Error::TypeMismatch(Marker::FixPos(0)))
	})?;
	while let Ok(Some(entry)) = read_dir.next_entry().await {
		let name = match entry.file_name().into_string() {
			Ok(s) => s,
			Err(_) => continue,
		};
		if entry.metadata().await.map(|m| m.is_dir()).unwrap_or(false) && children.contains(&name) {
			return Ok(false);
		}
	}
	Ok(true)
}

fn accept_by_glob(source: impl AsRef<Path>, accept_glob_set: &GlobSet) -> bool {
	accept_glob_set.is_match(source.as_ref())
}
fn reject_by_glob(source: impl AsRef<Path>, reject_glob_set: &GlobSet) -> bool {
	!accept_by_glob(source, reject_glob_set)
}

// -------- System rules and builder (no DB seeding) --------

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

pub static NO_SYSTEM_FILES: Lazy<SystemIndexerRule> = Lazy::new(|| {
	SystemIndexerRule {
    name: "No System files",
    default: true,
    rules: vec![
        RulePerKind::new_reject_files_by_globs_str(
            [
                vec!["**/.spacedrive"],
                #[cfg(target_os = "windows")]
                vec![
                    "**/{Thumbs.db,Thumbs.db:encryptable,ehthumbs.db,ehthumbs_vista.db}",
                    "**/*.stackdump",
                    "**/[Dd]esktop.ini",
                    "**/$RECYCLE.BIN",
                    "**/FOUND.[0-9][0-9][0-9]",
                    "**/{CON,PRN,AUX,NUL,COM0,COM1,COM2,COM3,COM4,COM5,COM6,COM7,COM8,COM9,LPT0,LPT1,LPT2,LPT3,LPT4,LPT5,LPT6,LPT7,LPT8,LPT9}",
                    "**/{CON,PRN,AUX,NUL,COM0,COM1,COM2,COM3,COM4,COM5,COM6,COM7,COM8,COM9,LPT0,LPT1,LPT2,LPT3,LPT4,LPT5,LPT6,LPT7,LPT8,LPT9}.*",
                    "C:/Users/*/NTUSER.DAT*",
                    "C:/Users/*/ntuser.dat*",
                    "C:/Users/*/{ntuser.ini,ntuser.dat,NTUSER.DAT}",
                    "C:/Users/*/{Cookies,AppData,NetHood,Recent,PrintHood,SendTo,Templates,Start Menu,Application Data,Local Settings,My Documents}",
                    "C:/{\\$Recycle.Bin,\\$WinREAgent,Documents and Settings,Program Files,Program Files (x86),ProgramData,Recovery,PerfLogs,Windows,Windows.old}",
                    "[A-Z]:/System Volume Information",
                    "C:/{config,pagefile,hiberfil}.sys",
                    "[A-Z]:/swapfile.sys",
                    "C:/DumpStack.log.tmp",
                ],
                #[cfg(any(target_os = "ios", target_os = "macos"))]
                vec![
                    "**/.{DS_Store,AppleDouble,LSOverride}",
                    "**/Icon\r\r",
                    "**/._*",
                ],
                #[cfg(target_os = "macos")]
                vec![
                    "/{System,Network,Library,Applications,.PreviousSystemInformation,.com.apple.templatemigration.boot-install}",
                    "/System/Volumes/Data/{System,Network,Library,Applications,.PreviousSystemInformation,.com.apple.templatemigration.boot-install}",
                    "/Users/*/{Library,Applications}",
                    "/System/Volumes/Data/Users/*/{Library,Applications}",
                    "**/*.photoslibrary/{database,external,private,resources,scope}",
                    "**/.{DocumentRevisions-V100,fseventsd,Spotlight-V100,TemporaryItems,Trashes,VolumeIcon.icns,com.apple.timemachine.donotpresent}",
                    "**/.{AppleDB,AppleDesktop,apdisk}",
                    "**/{Network Trash Folder,Temporary Items}",
                ],
                #[cfg(target_os = "linux")]
                vec![
                    "**/*~",
                    "**/.fuse_hidden*",
                    "**/.directory",
                    "**/.Trash-*",
                    "**/.nfs*",
                ],
                #[cfg(target_os = "android")]
                vec!["**/.nomedia", "**/.thumbnails"],
                #[cfg(target_family = "unix")]
                vec!["/{dev,sys,proc}", "/{run,var,boot}", "**/lost+found"],
            ]
            .into_iter()
            .flatten(),
        )
        .expect("hardcoded globs valid"),
    ],
}
});

pub static NO_HIDDEN: Lazy<SystemIndexerRule> = Lazy::new(|| SystemIndexerRule {
	name: "No Hidden files",
	default: false,
	rules: vec![RulePerKind::new_reject_files_by_globs_str(["**/.*"]).expect("valid")],
});

pub static NO_GIT: Lazy<SystemIndexerRule> = Lazy::new(|| SystemIndexerRule {
	name: "No Git files",
	default: true,
	rules: vec![RulePerKind::new_reject_files_by_globs_str([
		"**/{.git,.gitignore,.gitattributes,.gitkeep,.gitconfig,.gitmodules}",
	])
	.expect("valid")],
});

pub static NO_DEV_DIRS: Lazy<SystemIndexerRule> = Lazy::new(|| SystemIndexerRule {
	name: "No Dev Directories",
	default: true,
	rules: vec![RulePerKind::new_reject_files_by_globs_str([
		"**/node_modules",
		"**/target",
		"**/dist",
		"**/build",
		"**/.idea",
		"**/.vscode",
		"**/.vs",
		"**/__pycache__",
		"**/.pytest_cache",
		"**/.mypy_cache",
		"**/.tox",
		"**/.nox",
		"**/.coverage",
		"**/.hypothesis",
		"**/.cache",
		"**/Cache",
		"**/Caches",
		"**/CachedData",
		"**/Code Cache",
		"**/tmp",
		"**/temp",
		"**/.tmp",
		"**/.temp",
	])
	.expect("valid")],
});

pub static GITIGNORE: Lazy<SystemIndexerRule> = Lazy::new(|| SystemIndexerRule {
	name: "Gitignore",
	default: true,
	rules: vec![],
});

pub static ONLY_IMAGES: Lazy<SystemIndexerRule> = Lazy::new(|| SystemIndexerRule {
	name: "Only Images",
	default: false,
	rules: vec![RulePerKind::new_accept_files_by_globs_str([
		"*.{avif,bmp,gif,ico,jpeg,jpg,png,svg,tif,tiff,webp}",
	])
	.expect("valid")],
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub struct RuleToggles {
	pub no_system_files: bool,
	pub no_hidden: bool,
	pub no_git: bool,
	pub gitignore: bool,
	pub only_images: bool,
	pub no_dev_dirs: bool,
}

impl Default for RuleToggles {
	fn default() -> Self {
		Self {
			no_system_files: true, // NO_SYSTEM_FILES.default = true
			no_hidden: false,      // NO_HIDDEN.default = false
			no_git: true,          // NO_GIT.default = true
			gitignore: true,       // GITIGNORE.default = true
			only_images: false,    // ONLY_IMAGES.default = false
			no_dev_dirs: true,     // NO_DEV_DIRS.default = true
		}
	}
}

pub struct GitIgnoreRules {
	rules: RulePerKind,
}

impl GitIgnoreRules {
	pub async fn get_rules_if_in_git_repo(location_root: &Path, current: &Path) -> Option<Self> {
		let mut git_repo: Option<PathBuf> = None;
		let mut ignores: Vec<PathBuf> = Vec::new();
		for ancestor in current
			.ancestors()
			.take_while(|p| p.starts_with(location_root))
		{
			let gi = ancestor.join(".gitignore");
			if tokio::fs::try_exists(&gi).await.ok()? {
				ignores.push(gi);
			}
			if Self::is_git_repo(ancestor).await {
				git_repo = Some(ancestor.to_path_buf());
				break;
			}
		}
		let git_repo = git_repo?;
		Self::parse_git_repo(&git_repo, ignores).await.ok()
	}

	async fn parse_git_repo(git_repo: &Path, git_ignores: Vec<PathBuf>) -> Result<Self, ()> {
		use gix_ignore::{glob::search::pattern::List, search::Ignore};
		let mut search = Search::default();
		let mut lists: Vec<List<Ignore>> = Vec::new();
		for p in git_ignores {
			if let Ok(list) = Self::parse_git_ignore(p).await {
				lists.push(list);
			}
		}
		for list in lists {
			search.patterns.push(list);
		}
		if let Ok(extra) = Self::parse_git_exclude(git_repo.join(".git")).await {
			search.patterns.extend(extra);
		}
		Ok(Self {
			rules: RulePerKind::IgnoredByGit(git_repo.to_path_buf(), search),
		})
	}

	async fn parse_git_ignore(
		path: PathBuf,
	) -> Result<gix_ignore::glob::search::pattern::List<gix_ignore::search::Ignore>, ()> {
		tokio::task::spawn_blocking(move || {
			use gix_ignore::{glob::search::pattern::List, search::Ignore};
			let mut buf = Vec::with_capacity(30);
			List::from_file(path, None, true, &mut buf)
				.map_err(|_| ())?
				.ok_or(())
		})
		.await
		.map_err(|_| ())?
	}

	async fn parse_git_exclude(
		dot_git: PathBuf,
	) -> Result<Vec<gix_ignore::glob::search::pattern::List<gix_ignore::search::Ignore>>, ()> {
		tokio::task::spawn_blocking(move || {
			let mut buf = Vec::new();
			Search::from_git_dir(dot_git.as_ref(), None, &mut buf)
				.map(|s| s.patterns)
				.map_err(|_| ())
		})
		.await
		.map_err(|_| ())?
	}

	async fn is_git_repo(path: &Path) -> bool {
		let p = path.join(".git");
		tokio::task::spawn_blocking(move || p.is_dir())
			.await
			.unwrap_or(false)
	}
}

pub async fn build_default_ruler(
	toggles: RuleToggles,
	location_root: &Path,
	current: &Path,
) -> IndexerRuler {
	let mut base: Vec<IndexerRule> = Vec::new();
	if toggles.no_system_files {
		base.push((&*NO_SYSTEM_FILES).into());
	}
	if toggles.no_hidden {
		base.push((&*NO_HIDDEN).into());
	}
	if toggles.no_git {
		base.push((&*NO_GIT).into());
	}
	if toggles.no_dev_dirs {
		base.push((&*NO_DEV_DIRS).into());
	}
	if toggles.only_images {
		base.push((&*ONLY_IMAGES).into());
	}
	if toggles.gitignore {
		if let Some(gi) = GitIgnoreRules::get_rules_if_in_git_repo(location_root, current).await {
			let rule = IndexerRule {
				id: None,
				name: "Gitignore".to_string(),
				default: true,
				rules: vec![gi.rules],
				date_created: Utc::now(),
				date_modified: Utc::now(),
			};
			base.push(rule);
		}
	}
	IndexerRuler::new(base)
}
