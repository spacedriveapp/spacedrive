# xtask

Build automation tasks for Spacedrive using the xtask pattern.

## What is xtask?

The **xtask pattern** is the idiomatic Rust way to handle project-specific build automation. Instead of using shell scripts, Makefiles, JavaScript, or external build tools, you write your build tasks as Rust code in a workspace member called `xtask`.

This approach is used by major Rust projects including:

- **rust-analyzer** - IDE support
- **tokio** - async runtime
- **cargo** itself - Rust's package manager

## Benefits

- **Pure Rust** - No shell scripts or JavaScript tooling
- **Type-safe** - Catch errors at compile time
- **Debuggable** - Use standard Rust debugging tools
- **Cross-platform** - Works on Windows, macOS, Linux
- **No external tools** - Just `cargo` and `rustup`
- **Self-contained** - No need for pnpm, node, or JavaScript

## Usage

Run tasks using `cargo xtask`:

```bash
# Setup development environment (replaces pnpm prep)
cargo xtask setup

# Build iOS framework (device + simulator)
cargo xtask build-ios

# Or use the convenient alias:
cargo ios
```

## Available Commands

### `setup`

**Replaces `pnpm prep` with a pure Rust implementation!**

Sets up your development environment:

1. Downloads native dependencies (FFmpeg, protoc, OpenSSL, libheif, etc.)
2. Extracts them to `apps/.deps/`
3. Downloads iOS-specific deps if iOS targets are installed
4. Creates symlinks for shared libraries (macOS/Linux)
5. Generates `.cargo/config.toml` from the template

**Usage:**
```bash
cargo xtask setup
```

**First time setup:**
```bash
# Install Rust if you haven't already
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add iOS targets (macOS only, optional)
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

# Run setup
cargo xtask setup

# Build the CLI
cargo build
```

### `build-ios`

Builds the `sd-ios-core` library for iOS (macOS only):

1. Compiles for `aarch64-apple-ios` (physical devices)
2. Compiles for `aarch64-apple-ios-sim` (M1/M2 simulator)
3. Compiles for `x86_64-apple-ios` (Intel simulator)
4. Creates universal simulator library (ARM64 + x86_64) using `lipo`
5. Updates the existing XCFramework that Xcode is already using

**Output:** `apps/ios/sd-ios-core/sd_ios_core.xcframework/`

The XCFramework structure looks like:

```
sd_ios_core.xcframework/
├── Info.plist                           # Top-level XCFramework metadata
├── ios-arm64/
│   ├── libsd_ios_core.a                 # Device library
│   └── Info.plist                       # Device metadata
└── ios-arm64-simulator/
    ├── libsd_ios_core.a                 # Universal simulator library (ARM64 + x86_64)
    └── Info.plist                       # Simulator metadata
```

## How It Works

The xtask pattern works through Cargo's workspace and alias features. When you run:

```bash
cargo ios
```

Cargo:

1. Looks for an alias in `.cargo/config.toml`
2. Expands it to `cargo run --package xtask -- build-ios`
3. Builds and runs the `xtask` binary
4. Passes `build-ios` as an argument

The `xtask` binary is just a regular Rust program that uses `std::process::Command` to invoke cargo builds and file operations.

## Migration from Old Setup

### Replaced: `pnpm prep` (JavaScript)

**Old way:**
```bash
pnpm i              # Install JS dependencies
pnpm prep           # Run JavaScript setup script
```

**New way:**
```bash
cargo xtask setup   # Pure Rust, no JS needed!
```

### Replaced: `scripts/build_ios_xcframework.sh` (Bash)

**Old way:**
```bash
./scripts/build_ios_xcframework.sh
```

**New way:**
```bash
cargo ios           # Convenient alias
# or
cargo xtask build-ios
```

### Why the Change?

- **Old:** Required pnpm, node, JavaScript dependencies, bash scripts
- **New:** Only requires Rust toolchain (cargo/rustup)
- **Result:** Faster setup, fewer dependencies, more maintainable

## Requirements

- Rust toolchain with iOS targets installed:
  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
  ```
- Xcode with iOS SDK (for `lipo` command)

## Adding New Tasks

To add a new task:

1. Add a new function in `src/main.rs`
2. Add a match arm in `main()` to call your function
3. Document it in the help message
4. Optionally add a cargo alias in `.cargo/config.toml`

Example:

```rust
fn clean_ios() -> Result<()> {
    // Clean iOS builds
    todo!()
}

// In main():
match args[1].as_str() {
    "build-ios" => build_ios()?,
    "clean-ios" => clean_ios()?, // Add this
    _ => { /* ... */ }
}
```

## Further Reading

- [matklad's blog post on xtask](https://matklad.github.io/2018/01/03/make-your-own-make.html) (creator of rust-analyzer)
- [cargo-xtask documentation](https://github.com/matklad/cargo-xtask)
