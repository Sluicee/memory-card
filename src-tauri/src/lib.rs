mod audio;
mod scanner;
mod media_controls;
mod discord_rpc;
mod ffmpeg_source;

use audio::{create_player, PlaybackState, SharedPlayer};
use media_controls::MediaControlsManager;
use scanner::{calculate_library_size, scan_folder, Album, cover_filename};
use tauri_plugin_dialog::DialogExt;
use tauri::{Manager, Emitter, State};
use semver::Version;

#[derive(serde::Serialize)]
struct UpdateCheck {
    version: String,
    url: String,
    is_available: bool,
}

// ── Dialog command ────────────────────────────────────────────────────────────

#[tauri::command]
fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

// ── Native Helpers ────────────────────────────────────────────────────────────

fn save_cover_bytes(bytes: &[u8], id: &str, dir: &std::path::Path) -> Option<String> {
    let filename = cover_filename(id, "image/jpeg");
    let path = dir.join(&filename);
    std::fs::write(&path, bytes).ok()?;
    Some(path.to_string_lossy().into_owned())
}

// ── Scanner commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn scan_music_folder(path: String, app: tauri::AppHandle) -> Result<String, String> {
    let covers_dir = app.path().app_data_dir()
        .map_err(|e| e.to_string())?
        .join("covers");
    std::fs::create_dir_all(&covers_dir).map_err(|e| e.to_string())?;

    let app_clone = app.clone();
    let covers_clone = covers_dir.clone();
    let path_clone = path.clone();

    // Phase 1: parallel scan (blocking) — embedded tags + folder images
    let albums = tokio::task::spawn_blocking(move || {
        scan_folder(&path_clone, &app_clone, &covers_clone)
    })
    .await
    .map_err(|e| e.to_string())?;

    // Emit all albums immediately so the UI renders
    for album in &albums {
        app.emit("scan:album", album).ok();
    }
    app.emit("scan:done", ()).ok();

    // Phase 2: internet cover fetch — fire and forget, doesn't block scan return
    let app2 = app.clone();
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        // Fetch in parallel — up to 8 concurrent requests
        use futures::stream::{self, StreamExt};
        stream::iter(albums.into_iter().filter(|a| a.cover_art.is_none()))
            .map(|album| {
                let client = client.clone();
                let app2 = app2.clone();
                let covers_dir = covers_dir.clone();
                async move {
                    if let Some(cover_path) = fetch_cover_online(&client, &album, &covers_dir).await {
                        app2.emit("cover:update", serde_json::json!({
                            "id": album.id,
                            "cover_art": cover_path,
                        })).ok();
                    }
                }
            })
            .buffer_unordered(8)
            .collect::<()>()
            .await;

        // Signal frontend to re-save cache with updated cover paths
        app2.emit("cover:fetch:done", ()).ok();
    });

    let size = tokio::task::spawn_blocking(move || calculate_library_size(&path))
        .await
        .map_err(|e| e.to_string())?;

    Ok(format_size(size))
}

async fn fetch_cover_online(
    client: &reqwest::Client,
    album: &Album,
    covers_dir: &std::path::Path,
) -> Option<String> {
    // Skip if already cached from a previous scan
    let dest = covers_dir.join(cover_filename(&album.id, "image/jpeg"));
    if dest.exists() {
        return Some(dest.to_string_lossy().into_owned());
    }

    let query = format!("{} {}", album.artist, album.title);

    let resp: serde_json::Value = client
        .get("https://itunes.apple.com/search")
        .query(&[
            ("term", query.as_str()),
            ("media", "music"),
            ("entity", "album"),
            ("limit", "1"),
        ])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    // artworkUrl100 → replace with 600x600
    let artwork_url = resp["results"][0]["artworkUrl100"]
        .as_str()?
        .replace("100x100bb", "600x600bb");

    let img_bytes = client
        .get(&artwork_url)
        .send()
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;

    save_cover_bytes(&img_bytes, &album.id, covers_dir)
}

#[tauri::command]
async fn get_library_size(path: String) -> String {
    let bytes = tokio::task::spawn_blocking(move || calculate_library_size(&path))
        .await
        .unwrap_or(0);
    format_size(bytes)
}

fn format_size(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    if bytes >= GB {
        format!("{:.3} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.3} MB", bytes as f64 / MB as f64)
    }
}

// ── Library cache commands ────────────────────────────────────────────────────

#[tauri::command]
fn save_library_cache(data: String, app: tauri::AppHandle) -> Result<(), String> {
    let path = app.path().app_data_dir()
        .map_err(|e| e.to_string())?
        .join("library_cache.json");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_library_cache(app: tauri::AppHandle) -> Result<bool, String> {
    let path = app.path().app_data_dir()
        .map_err(|e| e.to_string())?
        .join("library_cache.json");

    let data = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };

    let albums: Vec<serde_json::Value> = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };

    if albums.is_empty() {
        return Ok(false);
    }

    for album in albums {
        app.emit("scan:album", &album).ok();
    }
    app.emit("scan:done", ()).ok();

    Ok(true)
}

// ── Audio commands ────────────────────────────────────────────────────────────

#[tauri::command]
fn audio_play(
    path: String,
    duration: f64,
    player: tauri::State<SharedPlayer>,
) -> Result<(), String> {
    player.play(&path, duration)
}

#[tauri::command]
fn audio_preload(path: String, player: tauri::State<SharedPlayer>) {
    player.preload(&path);
}

#[tauri::command]
fn audio_pause(player: tauri::State<SharedPlayer>) {
    player.pause();
}

#[tauri::command]
fn audio_resume(player: tauri::State<SharedPlayer>) {
    player.resume();
}

#[tauri::command]
fn audio_stop(player: tauri::State<SharedPlayer>) {
    player.stop();
}

#[tauri::command]
fn audio_seek(position: f64, player: tauri::State<SharedPlayer>) {
    player.seek(position);
}

#[tauri::command]
fn audio_set_volume(volume: f32, player: tauri::State<SharedPlayer>) {
    player.set_volume(volume);
}

#[tauri::command]
fn audio_get_state(player: tauri::State<SharedPlayer>) -> PlaybackState {
    player.get_state()
}

#[tauri::command]
fn audio_is_finished(player: tauri::State<SharedPlayer>) -> bool {
    player.is_finished()
}

#[tauri::command]
fn audio_get_position(player: tauri::State<SharedPlayer>) -> f64 {
    player.get_position()
}

// ── OS Media Controls Commands ────────────────────────────────────────────────

#[tauri::command]
fn update_media_metadata(
    title: String,
    artist: String,
    cover_url: Option<String>,
    duration_ms: u64,
    media_controls: State<'_, MediaControlsManager>,
) {
    media_controls.inner().update_metadata(&title, &artist, cover_url.as_deref(), duration_ms);
}

#[tauri::command]
fn update_media_playback_state(
    is_playing: bool,
    position_ms: u64,
    media_controls: State<'_, MediaControlsManager>,
) {
    media_controls.inner().update_playback(is_playing, position_ms);
}

// ── Update Commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateCheck, String> {
    let current_version_str = app.config().version.as_ref().ok_or("No version in tauri.conf.json")?;
    
    let repo_owner = "Sluicee";
    let repo_name = "memory-card";
    let url = format!("https://api.github.com/repos/{}/{}/releases/latest", repo_owner, repo_name);

    let client = reqwest::Client::builder()
        .user_agent("MemoryCard-App")
        .build()
        .map_err(|e| e.to_string())?;

    let resp: serde_json::Value = client.get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let latest_tag = resp["tag_name"].as_str().ok_or("No latest release found on GitHub")?;
    let download_url = resp["html_url"].as_str().ok_or("No download URL found")?;

    // Parse versions, stripping 'v' prefix if present
    let current_v = Version::parse(current_version_str.trim_start_matches('v'))
        .map_err(|e| format!("Failed to parse process version: {}", e))?;
    let latest_v = Version::parse(latest_tag.trim_start_matches('v'))
        .map_err(|e| format!("Failed to parse GitHub version: {}", e))?;

    Ok(UpdateCheck {
        version: latest_tag.to_string(),
        url: download_url.to_string(),
        is_available: latest_v > current_v,
    })
}

// ── App entry ─────────────────────────────────────────────────────────────────

#[cfg(windows)]
fn set_app_id() {
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    use windows::core::w;
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(w!("com.sluic.musicplayer"));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    set_app_id();


    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let player = create_player(app.handle().clone());
            app.manage(player);

            let manager = MediaControlsManager::new(app.handle());
            app.manage(manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            scan_music_folder,
            get_library_size,
            audio_play,
            audio_preload,
            audio_pause,
            audio_resume,
            audio_stop,
            audio_seek,
            audio_set_volume,
            audio_get_state,
            audio_is_finished,
            audio_get_position,
            save_library_cache,
            load_library_cache,
            update_media_metadata,
            update_media_playback_state,
            check_for_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
