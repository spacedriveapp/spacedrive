use serde::Deserialize;
use serde_json;
use std::env;
use std::process::Command;

const MACOS_TARGET_VERSION: &str = "11";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwiftTargetInfo {
  triple: String,
  unversioned_triple: String,
  module_triple: String,
  swift_runtime_compatibility_version: String,
  #[serde(rename = "librariesRequireRPath")]
  libraries_require_rpath: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwiftPaths {
  runtime_library_paths: Vec<String>,
  runtime_library_import_paths: Vec<String>,
  runtime_resource_path: String,
}

#[derive(Debug, Deserialize)]
struct SwiftTarget {
  target: SwiftTargetInfo,
  paths: SwiftPaths,
}

/// Builds mac_ddc library Swift project, sets the library search options right so we link
/// against Swift run-time correctly.
fn build_swift_natives() {
  let profile = env::var("PROFILE").unwrap();
  let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
  let target = format!("{}-apple-macosx{}", arch, MACOS_TARGET_VERSION);

  let swift_target_info_str = Command::new("swift")
    .args(&["-target", &target, "-print-target-info"])
    .output()
    .unwrap()
    .stdout;
  let swift_target_info: SwiftTarget = serde_json::from_slice(&swift_target_info_str).unwrap();
  if swift_target_info.target.libraries_require_rpath {
    panic!("Libraries require RPath! Change minimum MacOS value to fix.")
  }

  if !Command::new("swift")
    .args(&["build", "-c", &profile])
    .current_dir("./swift-lib")
    .status()
    .unwrap()
    .success()
  {
    panic!("Swift natives compilation failed")
  }

  swift_target_info
    .paths
    .runtime_library_paths
    .iter()
    .for_each(|path| {
      println!("cargo:rustc-link-search=native={}", path);
    });
  println!(
    "cargo:rustc-link-search=native=./swift-lib/.build/{}/{}",
    swift_target_info.target.unversioned_triple, profile
  );
  println!("cargo:rustc-link-lib=static=swift-lib");
  println!("cargo:rerun-if-changed=swift-lib/src/*.swift");
  println!(
    "cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET={}",
    MACOS_TARGET_VERSION
  );
}

fn main() {
  build_swift_natives();
  tauri_build::build();
}
