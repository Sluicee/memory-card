use crate::discord_rpc::DiscordManager;
use souvlaki::{MediaControls, MediaPlayback, MediaPosition, PlatformConfig};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, RegisterWindowMessageW, SetWindowLongPtrW, GWLP_WNDPROC, WM_COMMAND, WNDPROC,
};

// Windows numeric constants
const THBN_CLICKED: u16 = 0x1800;
const BTN_PREV: u16 = 1;
const BTN_PLAY_PAUSE: u16 = 2;
const BTN_NEXT: u16 = 3;

// Global state for subclassing
static mut ORIGINAL_WNDPROC: Option<WNDPROC> = None;
static mut APP_HANDLE: Option<AppHandle> = None;
static mut TASKBAR_CREATED_MSG: u32 = 0;

#[derive(Default, Clone)]
struct TrackMetadata {
    title: String,
    artist: String,
    duration_ms: u64,
}

pub struct MediaControlsManager {
    controls: Arc<Mutex<Option<MediaControls>>>,
    hwnd: isize,
    discord: DiscordManager,
    current_metadata: Arc<Mutex<TrackMetadata>>,
    current_playback: Arc<Mutex<(bool, u64)>>, // (is_playing, position_ms)
}

unsafe impl Send for MediaControlsManager {}
unsafe impl Sync for MediaControlsManager {}

impl MediaControlsManager {
    pub fn new(app: &AppHandle) -> Self {
        let hwnd_val = get_hwnd_val(app);

        unsafe {
            APP_HANDLE = Some(app.clone());
            TASKBAR_CREATED_MSG = RegisterWindowMessageW(windows::core::w!("TaskbarButtonCreated"));

            let original = SetWindowLongPtrW(
                HWND(hwnd_val as *mut _),
                GWLP_WNDPROC,
                wndproc_hook as *const () as usize as isize,
            );
            ORIGINAL_WNDPROC = Some(std::mem::transmute(original));
        }

        let mut manager = Self {
            controls: Arc::new(Mutex::new(None)),
            hwnd: hwnd_val,
            discord: DiscordManager::new(),
            current_metadata: Arc::new(Mutex::new(TrackMetadata::default())),
            current_playback: Arc::new(Mutex::new((false, 0))),
        };

        manager.init_smtc(app);

        // Thumbnail Toolbar disabled due to Explorer crashes.
        // manager.init_thumbnail_toolbar();

        manager
    }

    fn init_smtc(&mut self, app: &AppHandle) {
        let root_hwnd = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetAncestor(
                HWND(self.hwnd as *mut _),
                windows::Win32::UI::WindowsAndMessaging::GA_ROOT,
            )
        };
        let raw_hwnd = Some(root_hwnd.0 as isize as *mut _);

        let config = PlatformConfig {
            dbus_name: "com.sluic.musicplayer",
            display_name: "Memory Card",
            hwnd: raw_hwnd,
        };

        if let Ok(mut controls) = MediaControls::new(config) {
            let app_clone = app.clone();
            let _ = controls.attach(move |event| {
                println!("SMTC Event received: {:?}", event);
                let action = match event {
                    souvlaki::MediaControlEvent::Play => "play",
                    souvlaki::MediaControlEvent::Pause => "pause",
                    souvlaki::MediaControlEvent::Toggle => "toggle",
                    souvlaki::MediaControlEvent::Next => "next",
                    souvlaki::MediaControlEvent::Previous => "previous",
                    _ => "",
                };
                if !action.is_empty() {
                    let _ = app_clone.emit("smtc-event", action);
                }
            });
            *self.controls.lock().unwrap() = Some(controls);
            println!("SMTC initialized successfully");
        } else {
            println!("Failed to initialize SMTC with config");
        }
    }

    pub fn update_playback(&self, is_playing: bool, position_ms: u64) {
        {
            let mut play_lock = self.current_playback.lock().unwrap();
            *play_lock = (is_playing, position_ms);
        }
        self.update_discord();

        if let Some(controls) = self.controls.lock().unwrap().as_mut() {
            let state = if is_playing {
                MediaPlayback::Playing {
                    progress: Some(MediaPosition(std::time::Duration::from_millis(position_ms))),
                }
            } else {
                MediaPlayback::Paused {
                    progress: Some(MediaPosition(std::time::Duration::from_millis(position_ms))),
                }
            };
            let _ = controls.set_playback(state);
        }
    }

    pub fn update_metadata(
        &self,
        title: &str,
        artist: &str,
        // album: &str,
        cover_url: Option<&str>,
        duration_ms: u64,
    ) {
        {
            let mut meta_lock = self.current_metadata.lock().unwrap();
            meta_lock.title = title.to_string();
            meta_lock.artist = artist.to_string();
            meta_lock.duration_ms = duration_ms;
        }
        self.update_discord();

        if let Some(controls) = self.controls.lock().unwrap().as_mut() {
            // Copy cover to a temporary file in %TEMP% to ensure SMTC/Windows Widget 
            // has permission to access it. Windows is notoriously picky about AppData files.
            let temp_path_buf = std::env::temp_dir().join("musicplayer_smtc_cover.jpg");
            let mut final_cover_url = cover_url;

            if let Some(path) = cover_url {
                let _ = std::fs::copy(path, &temp_path_buf);
                final_cover_url = temp_path_buf.to_str();
            }

            let metadata = souvlaki::MediaMetadata {
                title: Some(title),
                artist: Some(artist),
                cover_url: final_cover_url,
                duration: Some(std::time::Duration::from_millis(duration_ms)),
                ..Default::default()
            };

            let _ = controls.set_metadata(metadata);
        }
    }

    fn update_discord(&self) {
        let meta = self.current_metadata.lock().unwrap();
        let (is_playing, position_ms) = *self.current_playback.lock().unwrap();

        if meta.title.is_empty() {
            return;
        }

        self.discord.update_presence(
            &meta.title,
            &meta.artist,
            is_playing,
            position_ms,
            meta.duration_ms,
        );
    }
}

// ── Native Helpers ────────────────────────────────────────────────────────────

fn get_hwnd_val<R: Runtime>(app: &tauri::AppHandle<R>) -> isize {
    let window = app
        .get_webview_window("main")
        .expect("No main window found");
    match window.hwnd() {
        Ok(hwnd) => hwnd.0 as isize,
        Err(_) => 0,
    }
}

// init_thumbnail_toolbar_raw removed to prevent crashes.

unsafe extern "system" fn wndproc_hook(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_COMMAND {
        let hiw = (wparam.0 >> 16) as u16;
        let low = (wparam.0 & 0xFFFF) as u16;
        if hiw == THBN_CLICKED {
            if let Some(Some(app)) = unsafe { (&raw const APP_HANDLE).as_ref() } {
                let action = match low {
                    BTN_PREV => "previous",
                    BTN_PLAY_PAUSE => "toggle",
                    BTN_NEXT => "next",
                    _ => "",
                };
                if !action.is_empty() {
                    let _ = app.emit("thumbnail-event", action);
                }
            }
        }
    }

    if let Some(orig) = ORIGINAL_WNDPROC {
        CallWindowProcW(orig, hwnd, msg, wparam, lparam)
    } else {
        LRESULT(0)
    }
}
