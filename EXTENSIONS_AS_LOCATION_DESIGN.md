# Extension Archive Format & Location Integration Design

**Status:** Design Proposal
**Created:** 2025-11-11
**Author:** @jamiepine

## Overview

This document specifies the `.sdext` archive format and the integration of extensions into Spacedrive's file explorer as a managed Location. Users will manage extensions like any other files: drag to install, delete to uninstall, browse, search, and organize.

## Goals

1. **Single-file distribution** - Extensions are atomic `.sdext` archives
2. **Native file operations** - Drag-and-drop install, delete to uninstall
3. **Unambiguous identification** - Custom magic numbers + file extension
4. **Security** - Validation before loading, sandboxed execution
5. **Zero special UI** - Extensions Location is just another location in the explorer
6. **Future-proof** - Versioned format, extensible manifest

## Archive Format Specification

### File Structure

```
extension-name.sdext
├── [Magic Header: 16 bytes]
├── [Format Version: 2 bytes]
├── [Reserved: 6 bytes]
└── [Tar+Gzip payload]
    ├── manifest.json       (required)
    ├── extension.wasm      (required)
    └── assets/             (optional)
        ├── icon.png
        ├── icon@2x.png
        └── ...
```

### Magic Header

**Bytes 0-15: Magic Number**
```
0x53 0x50 0x41 0x43 0x45 0x44 0x52 0x49   // "SPACEDRI"
0x56 0x45 0x2D 0x45 0x58 0x54 0x00 0x00   // "VE-EXT\0\0"
```

**Bytes 16-17: Format Version (Big Endian)**
```
0x00 0x01  // Version 1.0
```

**Bytes 18-23: Reserved**
```
0x00 0x00 0x00 0x00 0x00 0x00  // Reserved for future use
```

**Byte 24+: Tar+Gzip Payload**
Standard tar archive compressed with gzip.

### Rationale

- **16-byte magic**: Unambiguous, impossible to collide with other formats
- **Version field**: Format can evolve without breaking old readers
- **24-byte header**: Aligned, leaves room for future metadata
- **Tar+Gzip**: Industry standard, excellent tooling, all platforms
- **`.sdext` extension**: Clear intent, custom icon support

## Manifest Schema

### Required Fields

```json
{
  "id": "com.spacedrive.photos",
  "name": "Photos",
  "version": "1.0.0",
  "description": "AI-powered photo management",
  "author": "Spacedrive",
  "wasm_file": "photos.wasm",
  "permissions": {
    "methods": ["vdfs.", "ai."],
    "libraries": ["*"],
    "rate_limits": {
      "requests_per_minute": 1000,
      "concurrent_jobs": 10
    },
    "network_access": [],
    "max_memory_mb": 512
  }
}
```

### New Optional Fields for UI

```json
{
  "icon": "assets/icon.png",
  "icon_2x": "assets/icon@2x.png",
  "homepage": "https://spacedrive.com/extensions/photos",
  "repository": "https://github.com/spacedrive/extensions",
  "license": "MIT",
  "min_core_version": "2.0.0",
  "categories": ["media", "ai"],
  "screenshots": [
    "assets/screenshot1.png",
    "assets/screenshot2.png"
  ]
}
```

## Core Implementation

### 1. File Type Registration

**Location:** `core/src/filetype/builtin/`

Create new TOML definition:

```toml
# extensions.toml
[[file_types]]
id = "spacedrive_extension"
name = "Spacedrive Extension"
extensions = ["sdext"]
mime_types = ["application/vnd.spacedrive.extension"]
category = "archive"
priority = 255

[[file_types.magic_bytes]]
pattern = "5350414345445249564520455854"  # "SPACEDRIVE-EXT" in hex
offset = 0
priority = 255
```

**Add to ContentKind enum:**

```rust
// core/src/domain/content_identity.rs
pub enum ContentKind {
    // ... existing variants
    Extension = 20,  // Next available number
}
```

### 2. Archive Extraction & Validation

**Location:** `core/src/infra/extension/archive.rs` (new file)

```rust
use flate2::read::GzDecoder;
use std::io::{Read, Seek, SeekFrom};
use tar::Archive;
use thiserror::Error;

const MAGIC_HEADER: &[u8] = b"SPACEDRIVE-EXT\0\0";
const CURRENT_VERSION: u16 = 1;

#[derive(Error, Debug)]
pub enum ArchiveError {
    #[error("Invalid magic number")]
    InvalidMagic,

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u16),

    #[error("Missing required file: {0}")]
    MissingFile(String),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Archive too large (max {max}MB, got {actual}MB)")]
    TooLarge { max: usize, actual: usize },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct ExtensionArchive {
    pub manifest: ExtensionManifest,
    pub wasm_bytes: Vec<u8>,
    pub assets: HashMap<String, Vec<u8>>,
}

impl ExtensionArchive {
    /// Validate and extract a .sdext archive
    pub fn load<R: Read + Seek>(mut reader: R) -> Result<Self, ArchiveError> {
        // 1. Validate magic header
        let mut header = [0u8; 24];
        reader.read_exact(&mut header)?;

        if &header[0..16] != MAGIC_HEADER {
            return Err(ArchiveError::InvalidMagic);
        }

        // 2. Check version
        let version = u16::from_be_bytes([header[16], header[17]]);
        if version > CURRENT_VERSION {
            return Err(ArchiveError::UnsupportedVersion(version));
        }

        // 3. Extract tar+gzip payload
        let gz = GzDecoder::new(reader);
        let mut archive = Archive::new(gz);

        let mut manifest_json = None;
        let mut wasm_bytes = None;
        let mut assets = HashMap::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents)?;

            match path.to_str() {
                Some("manifest.json") => manifest_json = Some(contents),
                Some(p) if p.ends_with(".wasm") => wasm_bytes = Some(contents),
                Some(p) if p.starts_with("assets/") => {
                    assets.insert(p.to_string(), contents);
                }
                _ => {} // Ignore other files
            }
        }

        // 4. Validate required files exist
        let manifest_json = manifest_json
            .ok_or_else(|| ArchiveError::MissingFile("manifest.json".into()))?;
        let wasm_bytes = wasm_bytes
            .ok_or_else(|| ArchiveError::MissingFile("*.wasm".into()))?;

        // 5. Parse manifest
        let manifest: ExtensionManifest = serde_json::from_slice(&manifest_json)
            .map_err(|e| ArchiveError::InvalidManifest(e.to_string()))?;

        Ok(Self {
            manifest,
            wasm_bytes,
            assets,
        })
    }

    /// Extract icon from assets
    pub fn get_icon(&self) -> Option<&[u8]> {
        self.manifest.icon.as_ref()
            .and_then(|icon_path| self.assets.get(icon_path))
            .map(|v| v.as_slice())
    }
}
```

### 3. Plugin Manager Updates

**Location:** `core/src/infra/extension/manager.rs`

Add new method to load from `.sdext` files:

```rust
impl PluginManager {
    /// Load plugin from .sdext archive file
    pub async fn load_from_archive(&mut self, archive_path: &Path) -> Result<(), PluginError> {
        tracing::info!("Loading extension from archive: {:?}", archive_path);

        // 1. Open and validate archive
        let file = std::fs::File::open(archive_path)?;
        let archive = ExtensionArchive::load(file)
            .map_err(|e| PluginError::ManifestLoadFailed(e.to_string()))?;

        let plugin_id = archive.manifest.id.clone();

        // 2. Check if already loaded
        if self.plugins.read().await.contains_key(&plugin_id) {
            return Err(PluginError::AlreadyLoaded(plugin_id));
        }

        // 3. Extract to runtime cache
        let cache_dir = self.plugin_dir.join(".cache").join(&plugin_id);
        tokio::fs::create_dir_all(&cache_dir).await?;

        // Write WASM
        let wasm_path = cache_dir.join(format!("{}.wasm", plugin_id));
        tokio::fs::write(&wasm_path, &archive.wasm_bytes).await?;

        // Write manifest
        let manifest_json = serde_json::to_string_pretty(&archive.manifest)?;
        tokio::fs::write(cache_dir.join("manifest.json"), manifest_json).await?;

        // Write assets
        for (asset_path, data) in archive.assets {
            let full_path = cache_dir.join(&asset_path);
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(full_path, data).await?;
        }

        // 4. Load using existing directory-based loader
        self.load_from_cache(&plugin_id).await?;

        Ok(())
    }

    /// Load plugin from extracted cache directory
    async fn load_from_cache(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let cache_dir = self.plugin_dir.join(".cache").join(plugin_id);

        // Read manifest
        let manifest_path = cache_dir.join("manifest.json");
        let manifest_str = tokio::fs::read_to_string(&manifest_path).await?;
        let manifest: ExtensionManifest = serde_json::from_str(&manifest_str)?;

        // Load WASM module
        let wasm_path = cache_dir.join(format!("{}.wasm", plugin_id));
        let wasm_bytes = tokio::fs::read(&wasm_path).await?;

        // Compile and instantiate (existing logic)
        // ... rest of load_plugin logic

        Ok(())
    }
}
```

### 4. Extensions Location Auto-Creation

**Location:** `core/src/lib.rs` (startup initialization)

```rust
impl Spacedrive {
    pub async fn new_with_config(
        data_dir: PathBuf,
        system_device_name: Option<String>,
        port: Option<u16>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // ... existing initialization

        // Initialize plugin directory
        let plugin_dir = data_dir.join("extensions");
        tokio::fs::create_dir_all(&plugin_dir).await?;

        // Auto-create Extensions location for each library
        for library in self.library_manager.list_libraries().await? {
            self.ensure_extensions_location(&library, &plugin_dir).await?;
        }

        Ok(self)
    }

    async fn ensure_extensions_location(
        &self,
        library: &Library,
        plugin_dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let location_manager = LocationManager::new(self.events.clone());

        // Check if extensions location already exists
        let existing = location_manager
            .list_locations(library.clone())
            .await?
            .into_iter()
            .find(|loc| matches!(loc.sd_path, SdPath::Physical { ref path, .. } if path == plugin_dir));

        if existing.is_some() {
            return Ok(());
        }

        // Create system location
        let sd_path = SdPath::Physical {
            device_id: self.device.id(),
            path: plugin_dir.to_path_buf(),
        };

        location_manager.add_system_location(
            library.clone(),
            sd_path,
            "Extensions".to_string(),
            self.device.id(),
            IndexMode::Shallow, // Don't need deep indexing for extensions
            None,
        ).await?;

        tracing::info!("Created Extensions location at {:?}", plugin_dir);
        Ok(())
    }
}
```

### 5. System Location Support

**Location:** `core/src/domain/location.rs`

```rust
pub struct Location {
    // ... existing fields

    /// Whether this is a system-managed location (non-removable)
    pub is_system: bool,
}
```

**Location:** `core/src/location/manager.rs`

```rust
impl LocationManager {
    /// Add a system location (non-removable, managed by core)
    pub async fn add_system_location(
        &self,
        library: Arc<Library>,
        sd_path: SdPath,
        name: String,
        device_id: i32,
        index_mode: IndexMode,
        action_context: Option<ActionContext>,
    ) -> LocationResult<(Uuid, String)> {
        // Same as add_location but set is_system = true
        let (id, display_name) = self.add_location(
            library,
            sd_path,
            Some(name),
            device_id,
            index_mode,
            action_context,
        ).await?;

        // Update location to mark as system
        // ... (database update logic)

        Ok((id, display_name))
    }
}
```

### 6. File Watcher Integration

**Location:** `core/src/service/watcher/worker.rs`

```rust
// When .sdext file is added to Extensions location
async fn handle_file_created(&self, path: &Path) {
    if path.extension() == Some(OsStr::new("sdext")) {
        // Trigger extension installation
        if let Err(e) = self.install_extension(path).await {
            tracing::error!("Failed to install extension: {}", e);
        }
    }
}

// When .sdext file is deleted
async fn handle_file_deleted(&self, path: &Path) {
    if path.extension() == Some(OsStr::new("sdext")) {
        // Trigger extension uninstall
        if let Err(e) = self.uninstall_extension(path).await {
            tracing::error!("Failed to uninstall extension: {}", e);
        }
    }
}
```

## UI Implementation

### 1. Extensions Location in Sidebar

**Location:** `packages/interface/src/components/Sidebar.tsx`

Extensions location appears automatically like any other location, but with:
- Custom icon (puzzle piece or similar)
- Badge showing count of installed extensions
- Non-removable (system location flag)

### 2. Inspector Panel for `.sdext` Files

**Location:** `packages/interface/src/components/Inspector/ExtensionInspector.tsx`

```typescript
interface ExtensionInspector {
  // Show when selected file is .sdext in Extensions location
  // Display:
  // - Icon (from manifest or default)
  // - Name, version, author
  // - Description
  // - Status (installed/disabled/error)
  // - Toggle enable/disable
  // - View permissions button
  // - Uninstall button (just deletes file)
  // - Homepage/repo links
}
```

### 3. Install Flow

**Drag-and-drop handler:**

1. User drags `.sdext` file into Extensions location
2. UI validates file before copy (check magic number)
3. Shows permission grant dialog if needed
4. Copies file to Extensions location
5. File watcher triggers auto-install
6. UI shows progress notification

**Validation dialog:**

```typescript
interface ExtensionInstallDialog {
  // Before install, show:
  // - Extension metadata
  // - Required permissions (with explanations)
  // - Security warnings if needed
  // - Accept/Cancel buttons
}
```

### 4. Custom Icon Rendering

**Location:** `packages/interface/src/components/FileIcon.tsx`

```typescript
function FileIcon({ entry }) {
  if (entry.kind === 'Extension') {
    // Check if custom icon exists in cache
    const iconPath = getExtensionIcon(entry.id);
    return iconPath ? (
      <img src={iconPath} alt={entry.name} />
    ) : (
      <DefaultExtensionIcon />
    );
  }
  // ... existing logic
}
```

### 5. Extension Management Buttons

**Quick actions in file row:**
- **Toggle** - Enable/disable extension (doesn't delete file)
- **Info** - Open detailed view
- **Delete** - Uninstall (removes file)

## Wire Operations

### New Operations

```rust
// core/src/ops/extensions/ (new module)

// Query extension info from .sdext file
#[wire_operation("extensions.get_info")]
pub async fn get_extension_info(
    ctx: &QueryContext,
    path: SdPath,
) -> Result<ExtensionInfo> {
    // Read .sdext file, extract manifest, return info
}

// Toggle extension enabled state
#[wire_operation("extensions.toggle")]
pub async fn toggle_extension(
    ctx: &ActionContext,
    extension_id: String,
    enabled: bool,
) -> Result<()> {
    // Update extension state in plugin manager
}

// Validate extension before install
#[wire_operation("extensions.validate")]
pub async fn validate_extension(
    ctx: &QueryContext,
    file_data: Vec<u8>,
) -> Result<ValidationResult> {
    // Check magic number, parse manifest, validate permissions
}

// List all installed extensions
#[wire_operation("extensions.list")]
pub async fn list_extensions(
    ctx: &QueryContext,
) -> Result<Vec<ExtensionInfo>> {
    // Query Extensions location, read all .sdext files
}
```

## Security Considerations

### 1. Validation Pipeline

```
.sdext file
    ↓
[1] Magic number check (reject invalid archives)
    ↓
[2] Archive size limit (prevent DoS)
    ↓
[3] Manifest validation (schema check)
    ↓
[4] Permission analysis (security warnings)
    ↓
[5] WASM validation (wasmer compile check)
    ↓
[6] User approval (permission grant)
    ↓
Installed & loaded
```

### 2. Runtime Security

- **WASM sandbox** - Extensions run in Wasmer with no filesystem access
- **Permission system** - Fine-grained method access control
- **Rate limiting** - Prevent resource exhaustion
- **Memory limits** - Configurable per-extension caps
- **No eval/exec** - WASM cannot execute arbitrary code

### 3. Future: Code Signing

Reserve space in header for future signature verification:

```
Bytes 18-23: Reserved
    → Could become signature offset/length
```

## Migration Path

### Phase 1: Archive Format (Week 1)
- Implement `ExtensionArchive` loader
- Add `.sdext` file type registration
- Update `PluginManager` to support archives
- Create build tool for packing `.sdext` files

### Phase 2: Location Integration (Week 2)
- Implement system location support
- Auto-create Extensions location on startup
- Add file watcher hooks for install/uninstall
- Wire operations for extension management

### Phase 3: UI (Week 3)
- Extension inspector panel
- Install validation dialog
- Custom icon support
- Management buttons (toggle/delete)

### Phase 4: Polish (Week 4)
- Marketplace integration (future)
- Auto-updates (future)
- Extension search/browse
- Developer tools

## Developer Experience

### Building Extensions

**Before (loose files):**
```bash
cargo build --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/my_ext.wasm extensions/my-ext/
cp manifest.json extensions/my-ext/
```

**After (.sdext archive):**
```bash
cargo build --target wasm32-unknown-unknown
sd-cli extension pack \
  --manifest manifest.json \
  --wasm target/wasm32-unknown-unknown/release/my_ext.wasm \
  --assets assets/ \
  --output my-extension.sdext
```

### CLI Tool: `sd-cli extension`

```bash
# Pack an extension
sd-cli extension pack --manifest manifest.json --wasm my_ext.wasm -o output.sdext

# Unpack for inspection
sd-cli extension unpack my-extension.sdext -o ./extracted

# Validate without installing
sd-cli extension validate my-extension.sdext

# Sign (future)
sd-cli extension sign my-extension.sdext --key ~/.spacedrive/signing.key
```

## Future Enhancements

### 1. Extension Marketplace
- Central registry at `extensions.spacedrive.com`
- Direct install from marketplace (downloads `.sdext`)
- Automatic updates
- Ratings and reviews

### 2. Hot Reload
- Watch `.sdext` files for changes
- Reload extension without restart
- Preserve extension state where possible

### 3. Extension Dependencies
- Extensions can depend on other extensions
- Manifest declares dependencies with version ranges
- Automatic dependency resolution

### 4. Sandboxed Asset Access
- Extensions can bundle web UI assets
- Served via secure iframe with limited API access
- Custom UI beyond declarative manifest

### 5. Code Signing
- Official Spacedrive extensions signed by core team
- Community extensions show verification status
- Warning for unsigned extensions

## Open Questions

1. **Icon size requirements?**
   - Suggest: 256x256 PNG, optional 512x512 for @2x

2. **Max archive size?**
   - Suggest: 100MB limit (WASM + assets)

3. **Extension updates?**
   - Overwrite `.sdext` file? Automatic via marketplace?

4. **Multiple versions installed?**
   - Allow side-by-side or enforce single version?

5. **Extension data storage?**
   - Extensions get `{data_dir}/extensions/.data/{extension_id}/` for persistence?

## Appendix: Complete File Format Spec

```
Offset  | Size | Description
--------|------|--------------------------------------------------
0x00    | 16   | Magic: "SPACEDRIVE-EXT\0\0"
0x10    | 2    | Version (big-endian u16)
0x12    | 6    | Reserved (must be zero)
0x18    | *    | Tar+Gzip payload containing:
        |      |   - manifest.json (required)
        |      |   - *.wasm (required, one file)
        |      |   - assets/* (optional, any structure)
```

**Example Hex Dump:**

```
00000000: 5350 4143 4544 5249 5645 2d45 5854 0000  SPACEDRIVE-EXT..
00000010: 0001 0000 0000 0000 1f8b 0800 0000 0000  ................
00000020: 00ff ec5d 6b73 1c37 92fe 2fc0 0fec a996  ...]ks.7../.....
          └─────────────┘ └──────────────────────
          Version 1.0     Gzip header begins
```

## References

- [Tar format specification](https://www.gnu.org/software/tar/manual/html_node/Standard.html)
- [Magic number registry](https://en.wikipedia.org/wiki/List_of_file_signatures)
- [WebAssembly security model](https://webassembly.org/docs/security/)
- [Spacedrive WASM extension SDK](../../workbench/sdk/EXTENSION_SDK_SPECIFICATION_V2.md)

---

**End of Design Document**
