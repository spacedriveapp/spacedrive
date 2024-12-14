const COMMANDS: &[&str] = &["cors_request", "cancel_cors_request"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
