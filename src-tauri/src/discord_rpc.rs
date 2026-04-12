use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

            // Artwork cache shared with background fetch threads.
            // Stores real URL (found) or "" (not found this session).
            let artwork_cache: Arc<Mutex<HashMap<String, String>>> =
                Arc::new(Mutex::new(HashMap::new()));

            // In-flight fetch keys — removed by background threads on completion (success or failure).
            let pending_fetches: Arc<Mutex<HashSet<String>>> =
                Arc::new(Mutex::new(HashSet::new()));

            // (title, album, is_playing, image_url_sent, sent_at)
            let mut last_sent: Option<(String, String, bool, String, Instant)> = None;

            loop {
                if wake_rx.recv().is_err() {
                    break;
                }

                // Drain any additional signals that piled up (debounce rapid updates).
                while wake_rx.try_recv().is_ok() {}

                let presence_opt = state_clone.lock().unwrap().presence.clone();

                match presence_opt {
                    Some(p) => {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let start = now.saturating_sub(p.position_ms / 1000);

                        // Resolve artwork — non-blocking; background thread does the fetch.
                        let mut image_url = "icon".to_string();
                        if p.is_playing && !p.album.is_empty() {
                            let cache_key = format!("{}|{}", p.artist, p.album);
                            let cached = {
                                let cache = artwork_cache.lock().unwrap();
                                cache.get(&cache_key).cloned()
                            };
                            match cached {
                                Some(url) if !url.is_empty() => {
                                    image_url = url;
                                }
                                Some(_) => {
                                    // Empty string = previously searched, not found — skip.
                                }
                                None => {
                                    // Not searched yet — spawn fetch if not already in flight.
                                    let in_flight = pending_fetches.lock().unwrap().contains(&cache_key);
                                    if !in_flight {
                                        pending_fetches.lock().unwrap().insert(cache_key.clone());
                                        let cache_arc = artwork_cache.clone();
                                        let pending_arc = pending_fetches.clone();
                                        let artist = p.artist.clone();
                                        let album_name = p.album.clone();
                                        let wake = wake_tx_clone.clone();
                                        thread::spawn(move || {
                                            let rt = tokio::runtime::Builder::new_current_thread()
                                                .enable_all()
                                                .build()
                                                .unwrap();
                                            let url = rt
                                                .block_on(async {
                                                    let req_client = reqwest::Client::builder()
                                                        .timeout(Duration::from_secs(10))
                                                        .build()
                                                        .unwrap_or_default();
                                                    fetch_artwork(&req_client, &artist, &album_name).await
                                                });
                                            pending_arc.lock().unwrap().remove(&cache_key);
                                            match url {
                                                Some(u) => {
                                                    cache_arc.lock().unwrap().insert(cache_key, u);
                                                    let _ = wake.send(());
                                                }
                                                None => {
                                                    // Store empty string = "checked, not found".
                                                    // Prevents infinite retries for albums not on iTunes/MusicBrainz.
                                                    cache_arc.lock().unwrap().insert(cache_key, String::new());
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        }

                        // Deduplicate — stay within Discord's 5-per-20-seconds rate limit.
                        // Use wall-clock cooldown instead of start-timestamp comparison:
                        // position bounces at track start (stale values from previous track),
                        // making timestamp diffs unreliable as a dedup signal.
                        let mut duplicate = false;
                        if let Some((last_title, last_album, last_playing, last_image, last_time)) =
                            &last_sent
                        {
                            if p.is_playing == *last_playing {
                                if !p.is_playing {
                                    // Both paused/stopped — Discord is already clear.
                                    duplicate = true;
                                } else if last_title == &p.title && last_album == &p.album {
                                    // Same track. Only resend if artwork was upgraded
                                    // (icon → real URL) or enough time has passed to refresh
                                    // Discord's progress bar timestamps.
                                    let artwork_upgraded =
                                        image_url != "icon" && last_image == "icon";
                                    if !artwork_upgraded && last_time.elapsed().as_secs() < 30 {
                                        duplicate = true;
                                    }
                                }
                            }
                        }

                        if duplicate {
                            continue;
                        }

                        // Send to Discord.
                        let mut ok = false;
                        if let Some(ref mut c) = client {
                            if !p.is_playing {
                                ok = c.clear_activity().is_ok();
                            } else {
                                let end = start + (p.duration_ms / 1000);
                                let assets = activity::Assets::new()
                                    .large_image(&image_url)
                                    .large_text(&p.album);
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
                            last_sent = Some((
                                p.title.clone(),
                                p.album.clone(),
                                p.is_playing,
                                image_url,
                                Instant::now(),
                            ));
                        } else {
                            // Attempt reconnect.
                            if let Some(ref mut c) = client {
                                let _ = c.close();
                            }
                            client = DiscordIpcClient::new(client_id).ok();
                            if let Some(ref mut c) = client {
                                let _ = c.connect();

                                let retry_ok = if !p.is_playing {
                                    c.clear_activity().is_ok()
                                } else {
                                    let now2 = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                    let start2 = now2.saturating_sub(p.position_ms / 1000);
                                    let end2 = start2 + (p.duration_ms / 1000);
                                    let assets = activity::Assets::new()
                                        .large_image(&image_url)
                                        .large_text(&p.album);
                                    let timestamps = activity::Timestamps::new()
                                        .start(start2 as i64)
                                        .end(end2 as i64);
                                    let payload = activity::Activity::new()
                                        .state(&p.artist)
                                        .details(&p.title)
                                        .assets(assets)
                                        .timestamps(timestamps);
                                    c.set_activity(payload).is_ok()
                                };

                                if retry_ok {
                                    last_sent = Some((
                                        p.title.clone(),
                                        p.album.clone(),
                                        p.is_playing,
                                        image_url,
                                        Instant::now(),
                                    ));
                                }
                            }
                        }
                    }

                    None => {
                        // RPC disabled or no track — clear Discord status.
                        let already_cleared = last_sent
                            .as_ref()
                            .map(|(_, _, last_playing, _, _)| !last_playing)
                            .unwrap_or(false);

                        if already_cleared {
                            continue;
                        }

                        let mut ok = false;
                        if let Some(ref mut c) = client {
                            ok = c.clear_activity().is_ok();
                        }
                        if ok {
                            last_sent = Some((
                                "".to_string(),
                                "".to_string(),
                                false,
                                "".to_string(),
                                Instant::now(),
                            ));
                        } else {
                            if let Some(ref mut c) = client {
                                let _ = c.close();
                            }
                            client = DiscordIpcClient::new(client_id).ok();
                            if let Some(ref mut c) = client {
                                let _ = c.connect();
                                if c.clear_activity().is_ok() {
                                    last_sent = Some((
                                        "".to_string(),
                                        "".to_string(),
                                        false,
                                        "".to_string(),
                                        Instant::now(),
                                    ));
                                }
                            }
                        }
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

async fn fetch_musicbrainz_cover(client: &reqwest::Client, mbid: &str) -> Option<String> {
    let art_url = format!("https://coverartarchive.org/release/{}/front", mbid);
    let art_response = client
        .get(&art_url)
        .header("User-Agent", "MemoryCard/1.0 (music player)")
        .send()
        .await
        .ok()?;

    if art_response.status().is_success() || art_response.status().as_u16() == 307 {
        Some(art_response.url().to_string())
    } else {
        None
    }
}

async fn musicbrainz_search(client: &reqwest::Client, query: &str, album: &str) -> Option<String> {
    let response = client
        .get("https://musicbrainz.org/ws/2/release/")
        .query(&[("query", query), ("limit", "5"), ("fmt", "json")])
        .header("User-Agent", "MemoryCard/1.0 (music player)")
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = response.json().await.ok()?;
    let releases = json["releases"].as_array()?;

    if releases.is_empty() {
        return None;
    }

    let album_lower = album.to_lowercase();
    let best = releases.iter().find(|r| {
        r["title"]
            .as_str()
            .map(|n| n.to_lowercase().contains(&album_lower))
            .unwrap_or(false)
    });

    let hit = best.or_else(|| releases.first())?;
    let mbid = hit["id"].as_str()?;
    fetch_musicbrainz_cover(client, mbid).await
}

async fn fetch_musicbrainz_artwork(client: &reqwest::Client, artist: &str, album: &str) -> Option<String> {
    // Try strict artist+album query first.
    let strict_query = format!("artist:\"{}\" AND release:\"{}\"", artist, album);
    if let Some(url) = musicbrainz_search(client, &strict_query, album).await {
        return Some(url);
    }

    // Fall back to album-only query — handles cases where the track artist differs
    // from the album artist (e.g., compilation tracks, featured artists).
    let album_query = format!("release:\"{}\"", album);
    musicbrainz_search(client, &album_query, album).await
}

async fn fetch_artwork(client: &reqwest::Client, artist: &str, album: &str) -> Option<String> {
    if let Some(url) = fetch_itunes_artwork(client, artist, album).await {
        return Some(url);
    }
    fetch_musicbrainz_artwork(client, artist, album).await
}
