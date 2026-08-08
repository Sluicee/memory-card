use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::clip_scanner::scan_clip_folder as run_clip_scan;
use crate::clip_stream_server::ClipStreamServer;

#[tauri::command]
pub async fn scan_clip_folder(path: String, app: AppHandle) -> Result<(), String> {
    let thumbs_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("clip_thumbs");
    std::fs::create_dir_all(&thumbs_dir).map_err(|e| e.to_string())?;

    let app_clone = app.clone();
    let thumbs_clone = thumbs_dir.clone();
    let path_clone = path.clone();

    let clips = tokio::task::spawn_blocking(move || run_clip_scan(&path_clone, &app_clone, &thumbs_clone))
        .await
        .map_err(|e| e.to_string())?;

    for chunk in clips.chunks(50) {
        app.emit("clip:scan:clips", chunk).ok();
    }
    app.emit("clip:scan:done", ()).ok();

    Ok(())
}

#[tauri::command]
pub fn clear_clip_thumbs(app: AppHandle) -> Result<(), String> {
    let thumbs_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("clip_thumbs");
    if thumbs_dir.exists() {
        std::fs::remove_dir_all(&thumbs_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn save_clip_cache(data: String, app: AppHandle) -> Result<(), String> {
    let path = app.path().app_data_dir().map_err(|e| e.to_string())?.join("clips_cache.json");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_clip_cache(app: AppHandle) -> Result<bool, String> {
    let path = app.path().app_data_dir().map_err(|e| e.to_string())?.join("clips_cache.json");

    let data = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };

    let clips: Vec<serde_json::Value> = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };

    if clips.is_empty() {
        return Ok(false);
    }

    for chunk in clips.chunks(50) {
        app.emit("clip:scan:clips", chunk).ok();
    }
    app.emit("clip:scan:done", ()).ok();

    Ok(true)
}

/// (port, token) for the local clip-stream HTTP server — the frontend
/// builds the actual stream URL from these plus a path/seek/size/playable-
/// codecs query string (see clips.ts's buildClipStreamUrl).
#[tauri::command]
pub fn get_clip_stream_info(state: State<Arc<ClipStreamServer>>) -> (u16, String) {
    (state.port, state.token.clone())
}
