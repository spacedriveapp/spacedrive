//! Script adapter runtime: spawn external processes, parse JSONL protocol.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::adapter::{Adapter, AdapterKind, SyncReport};
use crate::db::SourceDb;
use crate::error::{Error, Result};
use crate::safety::TrustTier;
use crate::schema::{DataTypeMeta, DataTypeSchema, SearchContract};

/// A parsed `adapter.toml` manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterManifest {
	pub adapter: AdapterMeta,
	#[serde(default)]
	pub data_type: Option<DataTypeMeta>,
	#[serde(default)]
	pub models: HashMap<String, toml::Value>,
	#[serde(default)]
	pub search: Option<SearchContract>,
	#[serde(skip)]
	raw_toml: String,
}

/// The `[adapter]` section of `adapter.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterMeta {
	pub id: String,
	pub name: String,
	#[serde(default)]
	pub description: String,
	#[serde(default = "default_version")]
	pub version: String,
	#[serde(default)]
	pub author: String,
	#[serde(default)]
	pub license: String,
	#[serde(default)]
	pub homepage: String,
	#[serde(default)]
	pub icon: Option<String>,
	#[serde(default)]
	pub min_spacedrive: Option<String>,
	#[serde(default)]
	pub trust_tier: Option<String>,
	pub runtime: RuntimeConfig,
	#[serde(default)]
	pub config: Vec<ConfigField>,
}

fn default_version() -> String {
	"0.1.0".to_string()
}

/// The `[adapter.runtime]` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
	pub command: String,
	#[serde(default)]
	pub watch_command: Option<String>,
	#[serde(default = "default_timeout")]
	pub timeout: u64,
	#[serde(default)]
	pub schedule: Option<String>,
	#[serde(default)]
	pub requires: Vec<String>,
	#[serde(default)]
	pub setup: Option<String>,
	#[serde(default)]
	pub env: Vec<String>,
}

fn default_timeout() -> u64 {
	300
}

/// A `[[adapter.config]]` field declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
	pub key: String,
	pub name: String,
	#[serde(default)]
	pub description: String,
	#[serde(rename = "type", default = "default_config_type")]
	pub field_type: String,
	#[serde(default)]
	pub required: bool,
	#[serde(default)]
	pub secret: bool,
	#[serde(default)]
	pub default: Option<toml::Value>,
	#[serde(default)]
	pub options: Vec<String>,
	#[serde(default)]
	pub path_type: Option<String>,
}

fn default_config_type() -> String {
	"string".to_string()
}

impl AdapterManifest {
	/// Parse an `adapter.toml` file.
	pub fn parse(toml_str: &str) -> Result<Self> {
		let mut manifest: Self = toml::from_str(toml_str)
			.map_err(|e| Error::SchemaParse(format!("adapter.toml: {e}")))?;
		manifest.raw_toml = toml_str.to_string();
		Ok(manifest)
	}

	/// Parse from a file path.
	pub fn from_file(path: &Path) -> Result<Self> {
		let content = std::fs::read_to_string(path)?;
		Self::parse(&content)
	}

	/// Extract the embedded data type schema.
	pub fn extract_schema(&self) -> Result<DataTypeSchema> {
		let raw_table: toml::Table = toml::from_str(&self.raw_toml)
			.map_err(|e| Error::SchemaParse(format!("adapter.toml re-parse: {e}")))?;

		let mut schema_table = toml::Table::new();

		if let Some(dt) = raw_table.get("data_type") {
			schema_table.insert("data_type".to_string(), dt.clone());
		} else {
			return Err(Error::SchemaParse(
				"adapter.toml missing [data_type] section".into(),
			));
		}

		if let Some(models) = raw_table.get("models") {
			schema_table.insert("models".to_string(), models.clone());
		} else {
			return Err(Error::SchemaParse(
				"adapter.toml missing [models] section".into(),
			));
		}

		if let Some(search) = raw_table.get("search") {
			schema_table.insert("search".to_string(), search.clone());
		} else {
			return Err(Error::SchemaParse(
				"adapter.toml missing [search] section".into(),
			));
		}

		let schema_toml = toml::to_string_pretty(&schema_table)
			.map_err(|e| Error::SchemaParse(format!("schema rebuild: {e}")))?;

		crate::schema::parser::parse(&schema_toml)
	}

	/// List config fields marked as secret.
	pub fn secret_fields(&self) -> Vec<&ConfigField> {
		self.adapter.config.iter().filter(|f| f.secret).collect()
	}
}

/// A single operation from the script's JSONL stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SyncOperation {
	Upsert {
		upsert: String,
		external_id: String,
		fields: serde_json::Value,
	},
	Delete {
		delete: String,
		external_id: String,
	},
	Link {
		link: String,
		id: String,
		to: String,
		to_id: String,
	},
	Unlink {
		unlink: String,
		id: String,
		to: String,
		to_id: String,
	},
	Cursor {
		cursor: String,
	},
	Log {
		log: String,
		message: String,
	},
}

impl SyncOperation {
	/// Parse a single JSONL line into a SyncOperation.
	pub fn parse_line(line: &str) -> Result<Self> {
		let line = line.trim();
		if line.is_empty() {
			return Err(Error::AdapterSync("empty JSONL line".into()));
		}

		let value: serde_json::Value = serde_json::from_str(line)
			.map_err(|e| Error::AdapterSync(format!("invalid JSON: {e}")))?;

		let obj = value
			.as_object()
			.ok_or_else(|| Error::AdapterSync("JSONL line must be a JSON object".into()))?;

		if obj.contains_key("upsert") {
			Ok(SyncOperation::Upsert {
				upsert: obj["upsert"]
					.as_str()
					.ok_or_else(|| Error::AdapterSync("upsert field must be a string".into()))?
					.to_string(),
				external_id: obj
					.get("external_id")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("upsert missing external_id".into()))?
					.to_string(),
				fields: obj
					.get("fields")
					.cloned()
					.unwrap_or(serde_json::Value::Object(Default::default())),
			})
		} else if obj.contains_key("delete") {
			Ok(SyncOperation::Delete {
				delete: obj["delete"]
					.as_str()
					.ok_or_else(|| Error::AdapterSync("delete field must be a string".into()))?
					.to_string(),
				external_id: obj
					.get("external_id")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("delete missing external_id".into()))?
					.to_string(),
			})
		} else if obj.contains_key("link") {
			Ok(SyncOperation::Link {
				link: obj["link"]
					.as_str()
					.ok_or_else(|| Error::AdapterSync("link field must be a string".into()))?
					.to_string(),
				id: obj
					.get("id")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("link missing id".into()))?
					.to_string(),
				to: obj
					.get("to")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("link missing to".into()))?
					.to_string(),
				to_id: obj
					.get("to_id")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("link missing to_id".into()))?
					.to_string(),
			})
		} else if obj.contains_key("unlink") {
			Ok(SyncOperation::Unlink {
				unlink: obj["unlink"]
					.as_str()
					.ok_or_else(|| Error::AdapterSync("unlink field must be a string".into()))?
					.to_string(),
				id: obj
					.get("id")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("unlink missing id".into()))?
					.to_string(),
				to: obj
					.get("to")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("unlink missing to".into()))?
					.to_string(),
				to_id: obj
					.get("to_id")
					.and_then(|v| v.as_str())
					.ok_or_else(|| Error::AdapterSync("unlink missing to_id".into()))?
					.to_string(),
			})
		} else if obj.contains_key("cursor") {
			Ok(SyncOperation::Cursor {
				cursor: obj["cursor"]
					.as_str()
					.ok_or_else(|| Error::AdapterSync("cursor field must be a string".into()))?
					.to_string(),
			})
		} else if obj.contains_key("log") {
			Ok(SyncOperation::Log {
				log: obj["log"]
					.as_str()
					.ok_or_else(|| Error::AdapterSync("log field must be a string".into()))?
					.to_string(),
				message: obj
					.get("message")
					.and_then(|v| v.as_str())
					.unwrap_or("")
					.to_string(),
			})
		} else {
			Err(Error::AdapterSync(format!(
				"unknown JSONL operation: {}",
				serde_json::to_string(&value).unwrap_or_default()
			)))
		}
	}
}

/// A script adapter loaded from a directory containing `adapter.toml`.
pub struct ScriptAdapter {
	dir: PathBuf,
	manifest: AdapterManifest,
	schema: DataTypeSchema,
	icon_svg: Option<String>,
}

impl ScriptAdapter {
	/// Load a script adapter from its directory.
	pub fn from_dir(dir: &Path) -> Result<Self> {
		let manifest_path = dir.join("adapter.toml");
		if !manifest_path.exists() {
			return Err(Error::AdapterNotFound(format!(
				"no adapter.toml in {}",
				dir.display()
			)));
		}

		let manifest = AdapterManifest::from_file(&manifest_path)?;
		let schema = manifest.extract_schema()?;

		let icon_path = dir.join("icon.svg");
		let icon_svg = if icon_path.exists() {
			std::fs::read_to_string(&icon_path).ok()
		} else {
			None
		};

		Ok(Self {
			dir: dir.to_path_buf(),
			manifest,
			schema,
			icon_svg,
		})
	}

	/// Get the manifest.
	pub fn manifest(&self) -> &AdapterManifest {
		&self.manifest
	}

	/// Get the extracted data type schema.
	pub fn schema(&self) -> &DataTypeSchema {
		&self.schema
	}

	/// Get the adapter kind.
	pub fn kind(&self) -> AdapterKind {
		AdapterKind::Script
	}

	/// Build the sanitized environment for the subprocess.
	fn build_env(&self, config: &serde_json::Value) -> HashMap<String, String> {
		let mut env = HashMap::new();

		env.insert(
			"SPACEDRIVE_ADAPTER_ID".to_string(),
			self.manifest.adapter.id.clone(),
		);
		env.insert(
			"SPACEDRIVE_ADAPTER_VERSION".to_string(),
			self.manifest.adapter.version.clone(),
		);

		if let Some(obj) = config.as_object() {
			for (key, value) in obj {
				env.insert(
					format!("SPACEDRIVE_CONFIG_{}", key.to_uppercase()),
					value
						.as_str()
						.map(|s| s.to_string())
						.unwrap_or_else(|| value.to_string()),
				);
			}
		}

		for var_name in &self.manifest.adapter.runtime.env {
			if let Ok(value) = std::env::var(var_name) {
				env.insert(var_name.clone(), value);
			}
		}

		env
	}
}

impl Adapter for ScriptAdapter {
	fn id(&self) -> &str {
		&self.manifest.adapter.id
	}

	fn name(&self) -> &str {
		&self.manifest.adapter.name
	}

	fn data_type(&self) -> &str {
		&self.schema.data_type.id
	}

	fn description(&self) -> &str {
		&self.manifest.adapter.description
	}

	fn version(&self) -> &str {
		&self.manifest.adapter.version
	}

	fn author(&self) -> &str {
		&self.manifest.adapter.author
	}

	fn icon_svg(&self) -> Option<&str> {
		self.icon_svg.as_deref()
	}

	fn trust_tier(&self) -> TrustTier {
		self.manifest
			.adapter
			.trust_tier
			.as_deref()
			.map(TrustTier::from_str_or_default)
			.unwrap_or(TrustTier::External)
	}

	fn sync<'a>(
		&'a self,
		db: &'a SourceDb,
		config: &'a serde_json::Value,
	) -> std::pin::Pin<Box<dyn Future<Output = Result<SyncReport>> + Send + 'a>> {
		Box::pin(async move {
			let start = Instant::now();
			let mut report = SyncReport {
				records_upserted: 0,
				records_deleted: 0,
				links_created: 0,
				links_removed: 0,
				duration_ms: 0,
				error: None,
			};

			let env = self.build_env(config);
			let cmd = &self.manifest.adapter.runtime.command;

			let mut child = Command::new("sh")
				.arg("-c")
				.arg(cmd)
				.current_dir(&self.dir)
				.envs(&env)
				.stdin(std::process::Stdio::piped())
				.stdout(std::process::Stdio::piped())
				.stderr(std::process::Stdio::piped())
				.spawn()
				.map_err(|e| Error::AdapterSync(format!("failed to spawn adapter: {e}")))?;

			let mut stdin = child
				.stdin
				.take()
				.ok_or_else(|| Error::AdapterSync("failed to open stdin".into()))?;
			let stdout = child
				.stdout
				.take()
				.ok_or_else(|| Error::AdapterSync("failed to open stdout".into()))?;
			let stderr = child
				.stderr
				.take()
				.ok_or_else(|| Error::AdapterSync("failed to open stderr".into()))?;

			let config_json = serde_json::to_string(config)
				.map_err(|e| Error::AdapterSync(format!("failed to serialize config: {e}")))?;

			tokio::spawn(async move {
				let _ = stdin.write_all(config_json.as_bytes()).await;
				let _ = stdin.shutdown().await;
			});

			let mut reader = BufReader::new(stdout).lines();
			while let Some(line) = reader.next_line().await? {
				match SyncOperation::parse_line(&line) {
					Ok(op) => match op {
						SyncOperation::Upsert {
							upsert: model,
							external_id,
							fields,
						} => {
							if let Err(e) = db.upsert(&model, &external_id, &fields).await {
								tracing::warn!(error = %e, "upsert failed");
							} else {
								report.records_upserted += 1;
							}
						}
						SyncOperation::Delete {
							delete: model,
							external_id,
						} => {
							if let Err(e) = db.delete(&model, &external_id).await {
								tracing::warn!(error = %e, "delete failed");
							} else {
								report.records_deleted += 1;
							}
						}
						SyncOperation::Link {
							link: model_a,
							id: ext_id_a,
							to: model_b,
							to_id: ext_id_b,
						} => {
							if let Err(e) = db.link(&model_a, &ext_id_a, &model_b, &ext_id_b).await
							{
								tracing::warn!(error = %e, "link failed");
							} else {
								report.links_created += 1;
							}
						}
						SyncOperation::Unlink {
							unlink: model_a,
							id: ext_id_a,
							to: model_b,
							to_id: ext_id_b,
						} => {
							if let Err(e) =
								db.unlink(&model_a, &ext_id_a, &model_b, &ext_id_b).await
							{
								tracing::warn!(error = %e, "unlink failed");
							} else {
								report.links_removed += 1;
							}
						}
						SyncOperation::Cursor { cursor } => {
							if let Err(e) = db.set_cursor("default", &cursor).await {
								tracing::warn!(error = %e, "cursor update failed");
							}
						}
						SyncOperation::Log {
							log: level,
							message,
						} => match level.as_str() {
							"info" => tracing::info!("{message}"),
							"warn" => tracing::warn!("{message}"),
							"error" => tracing::error!("{message}"),
							_ => tracing::debug!("{message}"),
						},
					},
					Err(e) => {
						tracing::warn!(line = %line, error = %e, "failed to parse JSONL line");
					}
				}
			}

			let status = child.wait().await?;
			report.duration_ms = start.elapsed().as_millis() as u64;

			if !status.success() {
				report.error = Some(format!("adapter exited with status: {status}"));
			}

			Ok(report)
		})
	}
}
