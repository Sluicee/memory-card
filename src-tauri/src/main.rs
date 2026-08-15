// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    {
        // Do NOT set WEBKIT_DISABLE_DMABUF_RENDERER here: it forces WebKit onto a
        // CPU readback path, which caps video playback at a few FPS. The two vars
        // below are what make the GPU path actually usable.

        // Without this, the compositor kills the connection with a Wayland
        // protocol error on NVIDIA.
        if std::env::var_os("__NV_DISABLE_EXPLICIT_SYNC").is_none() {
            std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
        }
        // Without this, NVDEC hands WebKit a frame it can't sample and video
        // renders as a black rectangle.
        if std::env::var_os("WEBKIT_GST_DMABUF_SINK_DISABLED").is_none() {
            std::env::set_var("WEBKIT_GST_DMABUF_SINK_DISABLED", "1");
        }
    }

    musicplayer_lib::run()
}
