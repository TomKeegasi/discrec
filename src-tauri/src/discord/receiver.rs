use anyhow::Result;
use parking_lot::Mutex;
use serenity::all::{GuildId, UserId};
use serenity::async_trait;
use serenity::cache::Cache;
use serenity::http::Http;
use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::audio::encoder::{create_encoder, AudioEncoder, AudioFormat};

/// Shared state between all VoiceHandler clones registered with songbird.
pub struct ReceiverState {
    ssrc_map: Mutex<HashMap<u32, u64>>,
    /// Discord user ID -> display name, resolved when a user first speaks.
    user_names: Mutex<HashMap<u64, String>>,
    encoders: Mutex<HashMap<u32, Box<dyn AudioEncoder>>>,
    cache: Arc<Cache>,
    http: Arc<Http>,
    guild_id: GuildId,
    /// One timestamp for the whole session so all speaker tracks sort together.
    session_timestamp: String,
    output_dir: String,
    format: AudioFormat,
    sample_rate: u32,
    channels: u16,
    pub is_recording: Arc<AtomicBool>,
    pub peak_level_bits: Arc<AtomicU32>,
}

impl ReceiverState {
    pub fn new(
        cache: Arc<Cache>,
        http: Arc<Http>,
        guild_id: GuildId,
        output_dir: &str,
        format: AudioFormat,
        is_recording: Arc<AtomicBool>,
        peak_level_bits: Arc<AtomicU32>,
    ) -> Arc<Self> {
        Arc::new(Self {
            ssrc_map: Mutex::new(HashMap::new()),
            user_names: Mutex::new(HashMap::new()),
            encoders: Mutex::new(HashMap::new()),
            cache,
            http,
            guild_id,
            session_timestamp: chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string(),
            output_dir: output_dir.to_string(),
            format,
            sample_rate: 48000,
            channels: 1, // mono per speaker
            is_recording,
            peak_level_bits,
        })
    }

    /// Finalize all per-speaker encoders and return saved file paths.
    ///
    /// Files are created as soon as a speaker's first audio arrives, which can
    /// be before their name has been resolved, so each file is renamed to the
    /// speaker's display name here, once the encoder has been closed.
    pub fn finalize_all(&self) -> Result<Vec<String>> {
        let mut encoders = self.encoders.lock();
        let ssrc_map = self.ssrc_map.lock();
        let user_names = self.user_names.lock();
        let mut paths = Vec::new();

        for (ssrc, encoder) in encoders.drain() {
            let path = encoder.path().to_string();
            let user_id = ssrc_map.get(&ssrc).copied();
            log::info!("Finalizing speaker {} (user {:?}): {}", ssrc, user_id, path);
            encoder.finalize()?;

            let name = user_id.and_then(|id| user_names.get(&id));
            let final_path = match name {
                Some(name) => self.rename_to_speaker(&path, ssrc, name),
                None => path,
            };
            paths.push(final_path);
        }

        Ok(paths)
    }

    /// Rename a finished track to `discord-<session>-<name>.<ext>`. Falls back
    /// to `<name>-<ssrc>` if that name is taken (e.g. two users with the same
    /// display name, or one user who rejoined), and keeps the original path if
    /// renaming fails.
    fn rename_to_speaker(&self, path: &str, ssrc: u32, name: &str) -> String {
        let dir = std::path::Path::new(&self.output_dir);
        let ext = self.format.extension();
        let mut target = dir.join(format!(
            "discord-{}-{}.{}",
            self.session_timestamp, name, ext
        ));
        if target.exists() {
            target = dir.join(format!(
                "discord-{}-{}-{}.{}",
                self.session_timestamp, name, ssrc, ext
            ));
        }
        match std::fs::rename(path, &target) {
            Ok(()) => target.to_string_lossy().to_string(),
            Err(e) => {
                log::warn!("Could not rename {} to {}: {}", path, target.display(), e);
                path.to_string()
            }
        }
    }

    /// Look up a user's display name (server nickname, then global display
    /// name, then username), cache first and HTTP as a fallback, and remember
    /// it for `finalize_all`.
    async fn resolve_user_name(&self, user_id: u64) {
        if self.user_names.lock().contains_key(&user_id) {
            return;
        }

        let uid = UserId::new(user_id);
        let cached = self
            .cache
            .guild(self.guild_id)
            .and_then(|g| g.members.get(&uid).map(|m| m.display_name().to_string()));

        let name = match cached {
            Some(name) => Some(name),
            None => match self.guild_id.member(&self.http, uid).await {
                Ok(member) => Some(member.display_name().to_string()),
                Err(e) => {
                    log::warn!("Could not look up user {}: {}", user_id, e);
                    None
                }
            },
        };

        if let Some(name) = name.map(|n| sanitize_file_component(&n)) {
            if !name.is_empty() {
                log::info!("User {} is \"{}\"", user_id, name);
                self.user_names.lock().insert(user_id, name);
            }
        }
    }

    fn get_or_create_encoder(&self, ssrc: u32) -> Result<()> {
        let mut encoders = self.encoders.lock();
        if encoders.contains_key(&ssrc) {
            return Ok(());
        }

        let ssrc_map = self.ssrc_map.lock();
        let label = if let Some(user_id) = ssrc_map.get(&ssrc) {
            format!("user-{}", user_id)
        } else {
            format!("ssrc-{}", ssrc)
        };
        drop(ssrc_map);

        let filename = format!(
            "discord-{}-{}.{}",
            self.session_timestamp,
            label,
            self.format.extension()
        );
        let path = std::path::Path::new(&self.output_dir)
            .join(&filename)
            .to_string_lossy()
            .to_string();

        let encoder = create_encoder(&path, self.channels, self.sample_rate, self.format, false)?;
        log::info!("Created encoder for speaker {} -> {}", ssrc, path);
        encoders.insert(ssrc, encoder);
        Ok(())
    }
}

/// Songbird event handler — wraps shared state via Arc so it can be cloned
/// and registered for multiple event types.
pub struct VoiceHandler(pub Arc<ReceiverState>);

impl VoiceHandler {
    pub fn new(state: Arc<ReceiverState>) -> Self {
        Self(state)
    }
}

#[async_trait]
impl VoiceEventHandler for VoiceHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let state = &self.0;

        match ctx {
            EventContext::SpeakingStateUpdate(speaking) => {
                if let Some(user_id) = speaking.user_id {
                    state.ssrc_map.lock().insert(speaking.ssrc, user_id.0);
                    log::info!(
                        "Speaker mapping: SSRC {} -> user {}",
                        speaking.ssrc,
                        user_id.0
                    );
                    state.resolve_user_name(user_id.0).await;
                }
            }
            EventContext::VoiceTick(tick) => {
                if !state.is_recording.load(Ordering::Relaxed) {
                    return None;
                }

                let mut global_peak: f32 = 0.0;

                for (&ssrc, voice_data) in &tick.speaking {
                    if let Some(ref audio) = voice_data.decoded_voice {
                        // Track peak level across all speakers
                        let peak = audio
                            .iter()
                            .fold(0.0f32, |max, &s| max.max((s as f32).abs()));
                        let norm_peak = peak / i16::MAX as f32;
                        if norm_peak > global_peak {
                            global_peak = norm_peak;
                        }

                        // Ensure we have an encoder for this speaker
                        if let Err(e) = state.get_or_create_encoder(ssrc) {
                            log::error!("Failed to create encoder for SSRC {}: {}", ssrc, e);
                            continue;
                        }

                        // Write samples
                        let mut encoders = state.encoders.lock();
                        if let Some(encoder) = encoders.get_mut(&ssrc) {
                            for &sample in audio.iter() {
                                let float_sample = sample as f32 / i16::MAX as f32;
                                if let Err(e) = encoder.write_sample(float_sample) {
                                    log::error!("Failed to write sample: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }

                state
                    .peak_level_bits
                    .store(global_peak.to_bits(), Ordering::Relaxed);
            }
            _ => {}
        }
        None
    }
}

/// Make a display name safe to use in a file name on Windows, macOS and Linux.
fn sanitize_file_component(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    cleaned.trim_matches('_').chars().take(40).collect()
}

#[cfg(test)]
mod tests {
    use super::sanitize_file_component;

    #[test]
    fn sanitizes_display_names() {
        assert_eq!(sanitize_file_component("Alice"), "Alice");
        assert_eq!(sanitize_file_component("bob/../x"), "bob____x");
        assert_eq!(sanitize_file_component("  Zoë ✨ "), "Zoë");
        assert_eq!(sanitize_file_component("C:\\con?"), "C__con");
        assert_eq!(sanitize_file_component("✨✨"), "");
        assert_eq!(sanitize_file_component(&"a".repeat(80)).len(), 40);
    }
}
