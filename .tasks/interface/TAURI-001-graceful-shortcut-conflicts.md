---
id: TAURI-001
title: "Gracefully handle Alt+Space shortcut conflicts"
status: "Done"
assignee: "jamiepine"
priority: "Medium"
tags: ["interface", "tauri", "desktop"]
---

## Description

The Spacedrive Tauri application maps `Alt+Space` as the default global shortcut to toggle the voice overlay. Previously, this shortcut was registered at builder-time during the app's initialization sequence. If another application (such as Windows PowerToys Run, macOS Spotlight, or Wox) already had `Alt+Space` bound, the `expect("failed to register Alt+Space global shortcut")` call would cause the entire Tauri application to panic and crash on startup.

## The Why

Global shortcut collisions are extremely common on user machines. A failure to register a non-critical global shortcut (like the voice overlay) should never result in a fatal crash of the main application. By moving the registration from builder-time to runtime, we can catch the error, log a graceful warning via `tracing`, and allow the app to boot normally.

## The How (Implementation Steps)

1.  **Remove Builder-Time Registration**:
    We removed the hardcoded `.with_shortcut("Alt+Space").expect(...)` from the `tauri_plugin_global_shortcut` builder initialization.
2.  **Implement Runtime Registration**:
    After the app is built and running, we use `app.handle().global_shortcut().register("Alt+Space")` to dynamically register the shortcut.
3.  **Graceful Error Handling**:
    We match the `Result` of the registration attempt. On success, we log an `info!` tracing event. On failure, we log a `warn!` event and continue execution.

### Example Diff

```diff
- .plugin(
-     tauri_plugin_global_shortcut::Builder::new()
-         .with_shortcut("Alt+Space")
-         .expect("failed to register Alt+Space global shortcut")
-         .with_handler(|app, _shortcut, event| {
+ // Registration moved to runtime:
+ #[cfg(not(target_os = "linux"))]
+ {
+     use tauri_plugin_global_shortcut::GlobalShortcutExt;
+     match app.handle().global_shortcut().register("Alt+Space") {
+         Ok(_) => tracing::info!("Registered Alt+Space global shortcut"),
+         Err(error) => tracing::warn!(?error, "Failed to register Alt+Space global shortcut, voice overlay disabled"),
+     }
+ }
```

> **Note**: The implementation uses `#[cfg(not(target_os = "linux"))]` because global shortcuts on Linux (especially under Wayland) are handled at the desktop environment level and are not reliably supported by the Tauri plugin architecture.

## Acceptance Criteria
- Tauri app boots normally even if `Alt+Space` is bound by another program.
- A descriptive warning is printed to the daemon logs when a shortcut collision occurs.

## Review Refinements
- **Neutral Log Messaging:** Updated the `tracing::warn!` message for `Alt+Space` registration failure to be error-agnostic and explicitly log the underlying `Err(e)` details, rather than assuming the shortcut was already in use.
