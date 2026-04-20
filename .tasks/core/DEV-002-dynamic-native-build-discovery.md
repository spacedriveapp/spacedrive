---
id: DEV-002
title: "Dynamic Native Build Discovery for xtask"
status: "Done"
assignee: "jamiepine"
priority: "High"
tags: ["core", "build", "windows", "android"]
---

## Description

The Spacedrive daemon relies heavily on native dependencies, particularly FFmpeg via `ffmpeg-sys-next`. Previously, compilation on Windows/MSVC would frequently fail with `E0080` integer overflow and layout mismatch errors. This occurred because `bindgen` failed to resolve the correct Clang header paths and fallback SDK locations if the developer had not perfectly configured their environment variables (e.g., `LIBCLANG_PATH`, Windows SDK paths). 

Hardcoding these paths is brittle and causes friction for new contributors and CI runners. We need to implement dynamic, OS-aware discovery for MSVC, Windows SDK, `libclang.dll`, and Android SDK/NDK paths to ensure the build succeeds out of the box on Windows and Android cross-compilation environments.

## The Why

Relying on developers to manually specify paths for `vswhere`, LLVM, and Android SDKs introduces significant setup friction. By dynamically probing standard installation paths and registry keys (via `vswhere`), we can eliminate setup errors and ensure that `bindgen` parses headers using the correct ABI, resolving opaque struct errors.

## The How (Implementation Steps)

We implemented several robust discovery functions in `xtask/src/config.rs`:

1.  **MSVC Path Discovery (`find_windows_include_paths`)**:
    We now invoke `vswhere.exe` programmatically to locate the latest MSVC installation.
    ```rust
    let vs_path = Command::new(vswhere_installer)
        .args(["-latest", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property", "installationPath"])
        .output();
    ```
2.  **Windows SDK Resolution**:
    We search `C:\Program Files (x86)\Windows Kits\10\Include` for the latest installed SDK version, appending `ucrt`, `um`, and `shared` to the include paths.
3.  **Libclang Fallback (`find_libclang_path`)**:
    We first check `LIBCLANG_PATH`, then query `where.exe clang`, and finally probe standard LLVM installation directories.
4.  **Android SDK/NDK Discovery (`find_android_sdk`, `find_android_ndk`)**:
    We probe common installation paths across Windows, macOS, and Linux, and sort NDK versions to use the latest available.

### Example Diff

```diff
- // Previous configuration relied on environmental variables
+ /// Dynamically find the path to libclang.dll
+ fn find_libclang_path() -> Option<String> {
+ 	if let Ok(path) = std::env::var("LIBCLANG_PATH") {
+ 		return Some(path.replace('\\', "/"));
+ 	}
+   // ... automated discovery via where.exe and default paths
+ }
```

## Acceptance Criteria
- `cargo build` completes successfully on Windows without manual path setup.
- `bindgen` successfully locates MSVC and LLVM headers.
- Android tools are correctly discovered when cross-compiling.

## Review Refinements
- **Mocked Discovery Tests:** Refactored `find_windows_include_paths` and `find_libclang_path` to accept optional override paths, enabling testing via temporary directory injection instead of relying on real-machine toolchains. The legacy, environment-dependent test (`test_windows_dynamic_discovery`) was removed entirely to maintain test suite cleanliness.
- **Android NDK Fallback:** Updated the fallback for failed NDK discovery to return `None` (rather than an empty string), explicitly disable `has_android`, and log gracefully via `tracing::warn!` instead of `println!`.
- **Template Wiring:** Updated `.cargo/config.toml.mustache` to successfully consume the injected `{{#bindgenExtraClangArgs}}` and `{{#libclangPath}}` block directives.
- **Numeric Comparator Logic:** Refactored the discovery paths for MSVC, Windows SDK, and Android NDK to use a unified `get_latest_numeric_version` helper. This ensures reliable selection of the highest semantic version rather than incorrectly choosing non-version directories based on lexicographical string sorting.
- **Android Mock Tests:** Replaced the non-deterministic `test_android_discovery` environment probe with `test_android_discovery_mocked`, injecting temporary directories to formally assert the correct resolution of NDK components.
