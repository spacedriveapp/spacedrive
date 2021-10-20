use swift_rs::build_utils::{link_swift, link_swift_package};

fn main() {
  link_swift();
  link_swift_package("swift-lib", "./swift-lib/");
  
  tauri_build::build();
}
