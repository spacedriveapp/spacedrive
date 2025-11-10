mod events;
mod session;
pub mod commands;

pub use events::*;
pub use session::*;
pub use commands::*;

use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragItem {
    pub kind: DragItemKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DragItemKind {
    File { path: String },
    FilePromise { name: String, mime_type: String },
    Text { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragConfig {
    pub items: Vec<DragItem>,
    pub overlay_url: String,
    pub overlay_size: (f64, f64),
    pub allowed_operations: Vec<DragOperation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DragOperation {
    Copy,
    Move,
    Link,
}

#[derive(Debug, Clone)]
pub struct DragCoordinator {
    state: Arc<RwLock<Option<DragSession>>>,
}

impl DragCoordinator {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
        }
    }

    pub fn begin_drag(&self, app: &AppHandle, config: DragConfig, source_window: String) -> Result<(), String> {
        let mut state = self.state.write();
        if state.is_some() {
            return Err("A drag operation is already in progress".to_string());
        }

        let session = DragSession::new(config, source_window);
        *state = Some(session.clone());

        app.emit("drag:began", session.to_event()).ok();
        Ok(())
    }

    pub fn update_position(&self, app: &AppHandle, x: f64, y: f64) {
        if let Some(session) = self.state.read().as_ref() {
            app.emit("drag:moved", DragMoveEvent {
                session_id: session.id.clone(),
                x,
                y
            }).ok();
        }
    }

    pub fn enter_window(&self, app: &AppHandle, window_label: String) {
        if let Some(session) = self.state.read().as_ref() {
            app.emit("drag:entered", DragWindowEvent {
                session_id: session.id.clone(),
                window_label,
            }).ok();
        }
    }

    pub fn leave_window(&self, app: &AppHandle, window_label: String) {
        if let Some(session) = self.state.read().as_ref() {
            app.emit("drag:left", DragWindowEvent {
                session_id: session.id.clone(),
                window_label,
            }).ok();
        }
    }

    pub fn end_drag(&self, app: &AppHandle, result: DragResult) {
        if let Some(session) = self.state.write().take() {
            app.emit("drag:ended", DragEndEvent {
                session_id: session.id,
                result,
            }).ok();
        }
    }

    pub fn current_session(&self) -> Option<DragSession> {
        self.state.read().clone()
    }
}

impl Default for DragCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
