use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DiscordManager {
    client: Arc<Mutex<Option<DiscordIpcClient>>>,
}

impl DiscordManager {
    pub fn new() -> Self {
        // "1325852528766189628" is a placeholder or you can use your own.
        // For a public app, you should create one at https://discord.com/developers/applications
        let client_id = "1490773670384369784";

        let mut client = match DiscordIpcClient::new(client_id) {
            Ok(c) => Some(c),
            Err(_) => None,
        };

        if let Some(ref mut c) = client {
            let _ = c.connect();
        }

        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }

pub fn update_presence(
        &self,
        title: &str,
        artist: &str,
        // Альбом убрали из параметров
        is_playing: bool,
        position_ms: u64,
        duration_ms: u64,
    ) {
        let mut client_lock = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_lock {
            if !is_playing {
                let _ = client.clear_activity();
                return;
            }

            let assets = activity::Assets::new()
                .large_image("icon")
                .large_text("Memory Card");

            let mut timestamps = activity::Timestamps::new();

            // Высчитываем старт и конец. 
            // ВАЖНО: вызывай эту функцию только при смене трека, паузе или перемотке!
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let start = now.saturating_sub(position_ms / 1000);
            let end = start + (duration_ms / 1000);

            timestamps = timestamps.start(start as i64).end(end as i64);

            // Оставляем только артиста
            let state = artist.to_string(); 
            
            let payload = activity::Activity::new()
                .state(&state) // Сюда пойдет имя артиста
                .details(title) // Сюда пойдет название трека
                .assets(assets)
                .timestamps(timestamps);

            let _ = client.set_activity(payload);
        }
    }

    pub fn clear(&self) {
        let mut client_lock = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_lock {
            let _ = client.clear_activity();
        }
    }
}
