mod scanner;

use scanner::{Album, calculate_library_size, scan_folder};

#[tauri::command]
fn scan_music_folder(path: String) -> Result<Vec<Album>, String> {
    scan_folder(&path)
}

#[tauri::command]
fn get_library_size(path: String) -> String {
    let bytes = calculate_library_size(&path);
    format_size(bytes)
}

fn format_size(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_music_folder,
            get_library_size,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
