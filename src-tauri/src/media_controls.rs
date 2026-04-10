use crate::discord_rpc::DiscordManager;
use souvlaki::{MediaControls, MediaPlayback, MediaPosition, PlatformConfig};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::CreateBitmap;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{
    ITaskbarList3, TaskbarList, THUMBBUTTON, THUMBBUTTONFLAGS,
    THBF_DISABLED, THBF_ENABLED, THB_FLAGS, THB_ICON, THB_TOOLTIP,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateIconIndirect, ICONINFO,
    RegisterWindowMessageW, SetWindowLongPtrW,
    GWLP_WNDPROC, HICON, WM_COMMAND, WNDPROC,
};


// Windows numeric constants
#[cfg(target_os = "windows")]
const THBN_CLICKED: u16 = 0x1800;
#[cfg(target_os = "windows")]
const BTN_PREV: u16 = 1;
#[cfg(target_os = "windows")]
const BTN_PLAY_PAUSE: u16 = 2;
#[cfg(target_os = "windows")]
const BTN_NEXT: u16 = 3;

// Global state for subclassing
#[cfg(target_os = "windows")]
static mut ORIGINAL_WNDPROC: Option<WNDPROC> = None;
#[cfg(target_os = "windows")]
static mut APP_HANDLE: Option<AppHandle> = None;
#[cfg(target_os = "windows")]
static mut TASKBAR_CREATED_MSG: u32 = 0;
#[cfg(target_os = "windows")]
static mut THUMBNAIL_HWND: HWND = HWND(std::ptr::null_mut());
#[cfg(target_os = "windows")]
static mut ICON_PREV:  HICON = HICON(std::ptr::null_mut());
#[cfg(target_os = "windows")]
static mut ICON_PLAY:  HICON = HICON(std::ptr::null_mut());
#[cfg(target_os = "windows")]
static mut ICON_PAUSE: HICON = HICON(std::ptr::null_mut());
#[cfg(target_os = "windows")]
static mut ICON_NEXT:  HICON = HICON(std::ptr::null_mut());

#[derive(Default, Clone)]
struct TrackMetadata {
    title: String,
    artist: String,
    album: String,
    duration_ms: u64,
}

pub struct MediaControlsManager {
    controls: Arc<Mutex<Option<MediaControls>>>,
    hwnd: isize,
    discord: DiscordManager,
    discord_enabled: Arc<AtomicBool>,
    current_metadata: Arc<Mutex<TrackMetadata>>,
    current_playback: Arc<Mutex<(bool, u64)>>, // (is_playing, position_ms)
}

unsafe impl Send for MediaControlsManager {}
unsafe impl Sync for MediaControlsManager {}

impl MediaControlsManager {
    pub fn new(app: &AppHandle) -> Self {
        #[cfg(target_os = "windows")]
        let hwnd_val = get_hwnd_val(app);
        #[cfg(not(target_os = "windows"))]
        let hwnd_val = 0;

        #[cfg(target_os = "windows")]
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
            discord_enabled: Arc::new(AtomicBool::new(true)),
            current_metadata: Arc::new(Mutex::new(TrackMetadata::default())),
            current_playback: Arc::new(Mutex::new((false, 0))),
        };

        manager.init_smtc(app);
        manager
    }

    fn init_smtc(&mut self, app: &AppHandle) {
        #[cfg(target_os = "windows")]
        let raw_hwnd = unsafe {
            let root_hwnd = windows::Win32::UI::WindowsAndMessaging::GetAncestor(
                HWND(self.hwnd as *mut _),
                windows::Win32::UI::WindowsAndMessaging::GA_ROOT,
            );
            Some(root_hwnd.0 as isize as *mut _)
        };
        #[cfg(not(target_os = "windows"))]
        let raw_hwnd = None;

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
        #[cfg(target_os = "windows")]
        unsafe { update_thumbnail_play_state(is_playing) };

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
        album: &str,
        cover_url: Option<&str>,
        duration_ms: u64,
    ) {
        {
            let mut meta_lock = self.current_metadata.lock().unwrap();
            meta_lock.title = title.to_string();
            meta_lock.artist = artist.to_string();
            meta_lock.album = album.to_string();
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
        if !self.discord_enabled.load(Ordering::Relaxed) {
            return;
        }

        let meta = self.current_metadata.lock().unwrap();
        let (is_playing, position_ms) = *self.current_playback.lock().unwrap();

        if meta.title.is_empty() {
            return;
        }

        self.discord.update_presence(
            &meta.title,
            &meta.artist,
            &meta.album,
            is_playing,
            position_ms,
            meta.duration_ms,
        );
    }

    pub fn set_discord_enabled(&self, enabled: bool) {
        self.discord_enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            self.discord.clear();
        } else {
            self.update_discord();
        }
    }
}

// ── Native Helpers ────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_hwnd_val<R: Runtime>(app: &tauri::AppHandle<R>) -> isize {
    let window = app
        .get_webview_window("main")
        .expect("No main window found");
    match window.hwnd() {
        Ok(hwnd) => hwnd.0 as isize,
        Err(_) => 0,
    }
}

#[cfg(target_os = "windows")]
fn encode_tip(s: &str) -> [u16; 260] {
    let mut buf = [0u16; 260];
    for (i, c) in s.encode_utf16().enumerate().take(259) {
        buf[i] = c;
    }
    buf
}

#[cfg(target_os = "windows")]
unsafe fn make_thumb_button(id: u16, hicon: HICON, tip: &str, flags: THUMBBUTTONFLAGS) -> THUMBBUTTON {
    let mut b: THUMBBUTTON = std::mem::zeroed();
    b.dwMask = THB_ICON | THB_TOOLTIP | THB_FLAGS;
    b.iId = id as u32;
    b.hIcon = hicon;
    b.szTip = encode_tip(tip);
    b.dwFlags = flags;
    b
}

// ── Icon creation ─────────────────────────────────────────────────────────────
//
// 16×16 monochrome icons built from pixel coordinate lists.
// Windows XOR/AND icon rendering:
//   AND=0, XOR=1  → white pixel
//   AND=1, XOR=0  → transparent (background shows through)
//
// All shapes use rows 2-13 (12 rows), centered at y=7.5.
// d = distance from center row = narrowing factor (0 = widest, 5 = tip)
//
// ◀| PREV:  left-pointing triangle (x=5+d..11) + bar (x=2,3)
// ▶  PLAY:  right-pointing triangle (x=3..9-d)
// ⏸  PAUSE: two bars (x=4,5 and x=9,10)
// ▶| NEXT:  right-pointing triangle (x=4..10-d) + bar (x=12,13)

#[cfg(target_os = "windows")]
fn d_at(y: usize) -> usize {
    if y <= 7 { 7 - y } else { y - 8 }
}

#[cfg(target_os = "windows")]
fn prev_pixels(p: &mut Vec<(usize, usize)>) {
    for y in 2..=13usize {
        let d = d_at(y);
        p.push((y, 2)); p.push((y, 3));          // bar
        for x in (5 + d)..=11 { p.push((y, x)); } // ◀ triangle
    }
}
#[cfg(target_os = "windows")]
fn play_pixels(p: &mut Vec<(usize, usize)>) {
    for y in 2..=13usize {
        for x in 3..=(9 - d_at(y)) { p.push((y, x)); } // ▶ triangle
    }
}
#[cfg(target_os = "windows")]
fn pause_pixels(p: &mut Vec<(usize, usize)>) {
    for y in 2..=13usize {
        p.push((y, 4)); p.push((y, 5));   // bar 1
        p.push((y, 9)); p.push((y, 10));  // bar 2
    }
}
#[cfg(target_os = "windows")]
fn next_pixels(p: &mut Vec<(usize, usize)>) {
    for y in 2..=13usize {
        let d = d_at(y);
        for x in 4..=(10 - d) { p.push((y, x)); } // ▶ triangle
        p.push((y, 12)); p.push((y, 13));           // bar
    }
}

/// Build 1bpp color + AND-mask byte arrays.
/// CreateBitmap (DDB) uses WORD-alignment: 2 bytes per row for a 16-px-wide bitmap.
#[cfg(target_os = "windows")]
fn build_icon_bits(pixels: &[(usize, usize)]) -> ([u8; 32], [u8; 32]) {
    let mut color = [0x00_u8; 32];
    let mut mask  = [0xFF_u8; 32]; // all transparent
    for &(y, x) in pixels {
        let byte = y * 2 + x / 8;  // 2 bytes per row
        let bit  = 0x80_u8 >> (x % 8);
        color[byte] |= bit;
        mask[byte]  &= !bit; // 0 = opaque
    }
    (color, mask)
}

#[cfg(target_os = "windows")]
unsafe fn create_monochrome_icon(color: &[u8; 32], mask: &[u8; 32]) -> HICON {
    let hbm_mask  = CreateBitmap(16, 16, 1, 1, Some(mask.as_ptr()  as *const _));
    let hbm_color = CreateBitmap(16, 16, 1, 1, Some(color.as_ptr() as *const _));
    let info = ICONINFO {
        fIcon: BOOL(1),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask:  hbm_mask,
        hbmColor: hbm_color,
    };
    // GDI bitmaps are intentionally not freed — icons live for the process lifetime
    CreateIconIndirect(&info).unwrap_or(HICON(std::ptr::null_mut()))
}

#[cfg(target_os = "windows")]
unsafe fn create_icon(fill: fn(&mut Vec<(usize, usize)>)) -> HICON {
    let mut pixels = Vec::new();
    fill(&mut pixels);
    let (color, mask) = build_icon_bits(&pixels);
    create_monochrome_icon(&color, &mask)
}

// ── Thumbnail toolbar ─────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
unsafe fn init_thumbnail_toolbar(hwnd: HWND) {
    THUMBNAIL_HWND = hwnd;

    let taskbar: ITaskbarList3 = match CoCreateInstance(&TaskbarList, None, CLSCTX_ALL) {
        Ok(t) => t,
        Err(e) => { eprintln!("ThumbBar: CoCreateInstance failed: {:?}", e); return; }
    };
    if let Err(e) = taskbar.HrInit() {
        eprintln!("ThumbBar: HrInit failed: {:?}", e); return;
    }

    ICON_PREV  = create_icon(prev_pixels);
    ICON_PLAY  = create_icon(play_pixels);
    ICON_PAUSE = create_icon(pause_pixels);
    ICON_NEXT  = create_icon(next_pixels);

    let buttons = [
        make_thumb_button(BTN_PREV,       ICON_PREV, "Previous", THBF_ENABLED),
        make_thumb_button(BTN_PLAY_PAUSE, ICON_PLAY, "Play",     THBF_ENABLED),
        make_thumb_button(BTN_NEXT,       ICON_NEXT, "Next",     THBF_ENABLED),
    ];
    if let Err(e) = taskbar.ThumbBarAddButtons(hwnd, &buttons) {
        eprintln!("ThumbBar: ThumbBarAddButtons failed: {:?}", e);
    } else {
        println!("ThumbBar: initialized");
    }
}

#[cfg(target_os = "windows")]
unsafe fn update_thumbnail_play_state(is_playing: bool) {
    let hwnd = THUMBNAIL_HWND;
    if hwnd.0.is_null() { return; }

    let taskbar: ITaskbarList3 = match CoCreateInstance(&TaskbarList, None, CLSCTX_ALL) {
        Ok(t) => t,
        Err(_) => return,
    };
    let _ = taskbar.HrInit();

    let side_flags          = if is_playing { THBF_ENABLED } else { THBF_DISABLED };
    let (pp_icon, pp_tip)   = if is_playing { (ICON_PAUSE, "Pause") } else { (ICON_PLAY, "Play") };

    let buttons = [
        make_thumb_button(BTN_PREV,       ICON_PREV, "Previous", side_flags),
        make_thumb_button(BTN_PLAY_PAUSE, pp_icon,   pp_tip,     THBF_ENABLED),
        make_thumb_button(BTN_NEXT,       ICON_NEXT, "Next",     side_flags),
    ];
    let _ = taskbar.ThumbBarUpdateButtons(hwnd, &buttons);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn wndproc_hook(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // TaskbarButtonCreated fires once Explorer is ready — safe to add buttons here
    if TASKBAR_CREATED_MSG != 0 && msg == TASKBAR_CREATED_MSG {
        init_thumbnail_toolbar(hwnd);
    }

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
