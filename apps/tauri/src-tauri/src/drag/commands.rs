use super::*;
use crate::windows::SpacedriveWindow;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn begin_drag(
    app: AppHandle,
    config: DragConfig,
    source_window_label: String,
) -> Result<String, String> {
    tracing::info!("begin_drag called for window: {}", source_window_label);
    tracing::debug!("Drag config: {:?}", config);

    let coordinator = app.state::<DragCoordinator>();
    let session_id = uuid::Uuid::new_v4().to_string();

    match coordinator.begin_drag(&app, config.clone(), source_window_label.clone()) {
        Ok(_) => {},
        Err(e) => {
            tracing::error!("Failed to begin drag: {}", e);
            return Err(e);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let _source_window = app
            .get_webview_window(&source_window_label)
            .ok_or("Source window not found")?;

        let _overlay_window = SpacedriveWindow::DragOverlay {
            session_id: session_id.clone(),
        }
        .show(&app)
        .await?;

        let items_json = serde_json::to_string(&config.items)
            .map_err(|e| format!("Failed to serialize items: {}", e))?;

        tracing::info!("Calling Swift begin_native_drag on main thread...");

        let session_id_clone = session_id.clone();
        let app_clone = app.clone();

        // AppKit requires main thread - do ALL AppKit calls there
        let (tx, rx) = std::sync::mpsc::channel();

        app.run_on_main_thread(move || {
            let result: Result<bool, String> = (|| {
                let source_window = app_clone
                    .get_webview_window(&source_window_label)
                    .ok_or("Source window not found on main thread")?;

                let overlay_label = format!("drag-overlay-{}", session_id_clone);
                let overlay_window = app_clone
                    .get_webview_window(&overlay_label)
                    .ok_or("Overlay window not found on main thread")?;

                let source_ns_window = source_window
                    .ns_window()
                    .map_err(|e| format!("Failed to get source NSWindow: {}", e))?;

                let overlay_ns_window = overlay_window
                    .ns_window()
                    .map_err(|e| format!("Failed to get overlay NSWindow: {}", e))?;

                let success = unsafe {
                    sd_desktop_macos::begin_native_drag(
                        &source_ns_window,
                        &items_json.as_str().into(),
                        &overlay_ns_window,
                        &session_id_clone.as_str().into(),
                    )
                };

                overlay_window.show().ok();

                Ok(success)
            })();

            tx.send(result).ok();
        }).map_err(|e| format!("Failed to run on main thread: {:?}", e))?;

        let result = rx.recv().map_err(|e| format!("Failed to receive result: {}", e))?;
        let success = result?;

        tracing::info!("Swift begin_native_drag returned: {}", success);

        if !success {
            tracing::error!("Native drag failed, cleaning up state");
            coordinator.force_clear_state(&app);

            if let Some(overlay) = app.get_webview_window(&format!("drag-overlay-{}", session_id)) {
                overlay.close().ok();
            }

            return Err("Failed to begin native drag".to_string());
        }

        tracing::info!("Drag started successfully: session_id={}", session_id);
    }

    #[cfg(not(target_os = "macos"))]
    {
        return Err("Drag and drop is only supported on macOS currently".to_string());
    }

    Ok(session_id)
}

#[tauri::command]
pub async fn end_drag(
    app: AppHandle,
    session_id: String,
    result: DragResult,
) -> Result<(), String> {
    tracing::info!("end_drag called: session_id={}, result={:?}", session_id, result);

    let coordinator = app.state::<DragCoordinator>();
    coordinator.end_drag(&app, result);

    #[cfg(target_os = "macos")]
    {
        unsafe {
            sd_desktop_macos::end_native_drag(&session_id.as_str().into());
        }

        let overlay_label = format!("drag-overlay-{}", session_id);
        if let Some(overlay) = app.get_webview_window(&overlay_label) {
            tracing::debug!("Closing overlay window: {}", overlay_label);
            overlay.close().ok();
        } else {
            tracing::warn!("Overlay window not found during cleanup: {}", overlay_label);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_drag_session(app: AppHandle) -> Result<Option<DragSession>, String> {
    let coordinator = app.state::<DragCoordinator>();
    Ok(coordinator.current_session())
}

#[tauri::command]
pub async fn force_clear_drag_state(app: AppHandle) -> Result<(), String> {
    tracing::warn!("force_clear_drag_state called - this should only be used for debugging");
    let coordinator = app.state::<DragCoordinator>();
    coordinator.force_clear_state(&app);
    Ok(())
}
