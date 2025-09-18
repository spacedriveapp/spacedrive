# File Type System

A modern, extensible file type identification system for Spacedrive.

## Features

- **Data-driven**: File types defined in TOML files
- **Runtime extensibility**: Add new types without recompiling
- **Magic bytes**: Flexible pattern matching with wildcards
- **Priority resolution**: Smart handling of conflicting extensions
- **Rich metadata**: Arbitrary metadata per file type
- **Standards compliant**: MIME types and UTIs included

## Usage

```rust
use sd_core::file_type::FileTypeRegistry;

// Create registry with built-in types
let registry = FileTypeRegistry::new();

// Identify by extension
let jpeg_types = registry.get_by_extension("jpg");

// Identify by MIME type
let png_type = registry.get_by_mime("image/png");

// Full identification with magic bytes
let result = registry.identify(Path::new("photo.jpg")).await?;
println!("{} ({}% confidence)", result.file_type.name, result.confidence);
```

## Architecture

- `registry.rs` - Main API and identification logic
- `magic.rs` - Magic byte pattern matching
- `builtin.rs` - Embedded TOML definitions
- `definitions/` - TOML files with file type definitions

## Adding New Types

Create a TOML file:

```toml
[[file_types]]
id = "application/x-custom"
name = "Custom Format"
extensions = ["custom"]
mime_types = ["application/x-custom"]
category = "document"
priority = 100

[[file_types.magic_bytes]]
pattern = "43 55 53 54"  # "CUST"
offset = 0
priority = 100
```

## Categories

- `image` - Photos, graphics, etc.
- `video` - Movies, animations
- `audio` - Music, podcasts
- `document` - PDFs, office files
- `code` - Source code files
- `archive` - Compressed files
- `text` - Plain text, markdown
- `config` - Configuration files
- `database` - Database files
- `book` - E-books
- `font` - Font files
- `mesh` - 3D models
- `encrypted` - Encrypted/secure files
- `key` - Certificates, keys
- `executable` - Apps, scripts
- `unknown` - Unidentified files
