use swift_rs::build_utils;

fn main() {
  build_utils::link_swift();
  build_utils::link_swift_package("swift-lib", "./swift-lib/");
  tauri_build::build();
}
