use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, PartialEq)]
pub struct PresenceData {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
}

struct DiscordState {
    presence: Option<PresenceData>,
}

pub struct DiscordManager {
    state: Arc<Mutex<DiscordState>>,
    wake_tx: mpsc::Sender<()>,
}

impl DiscordManager {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(DiscordState {
            presence: None,
        }));
        let (wake_tx, wake_rx) = mpsc::channel::<()>();

        let state_clone = Arc::clone(&state);
        let wake_tx_clone = wake_tx.clone();

        thread::spawn(move || {
            let client_id = "1490773670384369784";
            let mut client = DiscordIpcClient::new(client_id).ok();
            if let Some(ref mut c) = client {
                let _ = c.connect();
            }

            let mut artwork_cache: HashMap<String, String> = HashMap::new();
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

            let mut last_sent: Option<(String, String, bool, u64)> = None;

            loop {
                // Wait for a wake signal
                if wake_rx.recv().is_err() {
                    break; // sender dropped
                }
                
                // Read all pending wake signals (debounce)
                while wake_rx.try_recv().is_ok() {}

                let presence_opt = state_clone.lock().unwrap().presence.clone();

                match presence_opt {
                    Some(p) => {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                        let start = now.saturating_sub(p.position_ms / 1000);
                        let end = start + (p.duration_ms / 1000);
                        
                        // Deduplicate identical states to avoid hitting Discord's 5-per-20-seconds rate limit
                        let mut duplicate = false;
                        if let Some((last_title, last_album, last_playing, last_start)) = &last_sent {
                            if p.is_playing == *last_playing {
                                if !p.is_playing {
                                    duplicate = true; // both paused
                                } else if last_title == &p.title && last_album == &p.album {
                                    let diff = if start > *last_start { start - last_start } else { last_start - start };
                                    if diff <= 4 {
                                        duplicate = true; // linear time progression, no need to update Discord
                                    }
                                }
                            }
                        }

                        if duplicate {
                            continue;
                        }

                        let mut image_url = "icon".to_string();

                        if p.is_playing {
                            if !p.album.is_empty() {
                                let cache_key = format!("{}|{}", p.artist, p.album);
                                if let Some(url) = artwork_cache.get(&cache_key) {
                                    image_url = url.clone();
                                } else {
                                    // Fetch synchronously because we are in a background thread
                                    let url = rt.block_on(async {
                                        let req_client = reqwest::Client::builder()
                                            .timeout(std::time::Duration::from_secs(5))
                                            .build()
                                            .unwrap_or_default();
                                        fetch_itunes_artwork(&req_client, &p.artist, &p.album).await
                                    }).unwrap_or_else(|| {
                                        "icon".to_string()
                                    });
                                    artwork_cache.insert(cache_key, url.clone());
                                    image_url = url;
                                }
                            }
                        }

                        // BEFORE sending to Discord, check if the state was updated while we were fetching!
                        // If there is a new wake signal, it means a new update arrived. Skip this one!
                        if wake_rx.try_recv().is_ok() {
                            // Put the token back so the next loop iteration will run immediately
                            let _ = wake_tx_clone.send(());
                            continue;
                        }

                        let mut ok = false;
                        if let Some(ref mut c) = client {
                            if !p.is_playing {
                                ok = c.clear_activity().is_ok();
                            } else {
                                let assets = activity::Assets::new()
                                    .large_image(&image_url)
                                    .large_text(&p.album);

                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let start = now.saturating_sub(p.position_ms / 1000);
                                let end = start + (p.duration_ms / 1000);
                                let timestamps = activity::Timestamps::new()
                                    .start(start as i64)
                                    .end(end as i64);

                                let payload = activity::Activity::new()
                                    .state(&p.artist)
                                    .details(&p.title)
                                    .assets(assets)
                                    .timestamps(timestamps);

                                ok = c.set_activity(payload).is_ok();
                            }
                        }

                        if ok {
                            last_sent = Some((p.title.clone(), p.album.clone(), p.is_playing, start));
                        } else {
                            // Try reconnecting and retry once
                            if let Some(ref mut c) = client {
                                let _ = c.close();
                            }
                            client = DiscordIpcClient::new(client_id).ok();
                            if let Some(ref mut c) = client {
                                let _ = c.connect();
                                
                                // Retry immediately
                                if !p.is_playing {
                                    let _ = c.clear_activity();
                                } else {
                                    let assets = activity::Assets::new()
                                        .large_image(&image_url)
                                        .large_text(&p.album);

                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                    let start = now.saturating_sub(p.position_ms / 1000);
                                    let end = start + (p.duration_ms / 1000);
                                    let timestamps = activity::Timestamps::new()
                                        .start(start as i64)
                                        .end(end as i64);

                                    let payload = activity::Activity::new()
                                        .state(&p.artist)
                                        .details(&p.title)
                                        .assets(assets)
                                        .timestamps(timestamps);
                                    let _ = c.set_activity(payload);
                                }
                                
                                // We update last_sent anyway so we don't spam reconnects if Discord is dead
                                last_sent = Some((p.title.clone(), p.album.clone(), p.is_playing, start));
                            }
                        }
                    },
                    None => {
                        let mut duplicate = false;
                        if let Some((_, _, last_playing, _)) = &last_sent {
                            if !last_playing { duplicate = true; }
                        }
                        if duplicate { continue; }
                        
                        // Clear activity
                        if let Some(ref mut c) = client {
                            let _ = c.clear_activity();
                        }
                        last_sent = Some(("".to_string(), "".to_string(), false, 0));
                    }
                }
            }
        });

        Self { state, wake_tx }
    }

    pub fn update_presence(
        &self,
        title: &str,
        artist: &str,
        album: &str,
        is_playing: bool,
        position_ms: u64,
        duration_ms: u64,
    ) {
        let mut s = self.state.lock().unwrap();
        s.presence = Some(PresenceData {
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            is_playing,
            position_ms,
            duration_ms,
        });
        let _ = self.wake_tx.send(());
    }

    pub fn clear(&self) {
        let mut s = self.state.lock().unwrap();
        s.presence = None;
        let _ = self.wake_tx.send(());
    }
}

async fn fetch_itunes_artwork(client: &reqwest::Client, artist: &str, album: &str) -> Option<String> {
    let query = format!("{} {}", artist, album);

    let response = client
        .get("https://itunes.apple.com/search")
        .query(&[("term", query.as_str()), ("entity", "album"), ("limit", "5")])
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = response.json().await.ok()?;
    let results = json["results"].as_array()?;

    if results.is_empty() {
        return None;
    }

    let album_lower = album.to_lowercase();
    let best = results.iter().find(|r| {
        r["collectionName"]
            .as_str()
            .map(|n| n.to_lowercase().contains(&album_lower))
            .unwrap_or(false)
    });

    let hit = best.or_else(|| results.first())?;
    let url = hit["artworkUrl100"].as_str()?;

    Some(url.replace("100x100bb", "600x600bb"))
}
