use anyhow::Result;
use cinewana_core::{ImageProfile, PlayerCommand, PlayerState};
use parking_lot::Mutex;
use std::path::PathBuf;

pub struct PlayerService {
    state: Mutex<PlayerState>,
    library_path: Option<PathBuf>,
}

impl PlayerService {
    pub fn discover() -> Self {
        let candidates = [
            PathBuf::from(".tools/mpv/libmpv-2.dll"),
            PathBuf::from(".tools/mpv/mpv-2.dll"),
            PathBuf::from("libmpv-2.dll"),
            PathBuf::from("mpv-2.dll"),
        ];
        let library_path = candidates.into_iter().find(|p| p.exists());
        let available = library_path.is_some();
        Self {
            library_path,
            state: Mutex::new(PlayerState {
                volume: 70.0,
                playback_speed: 1.0,
                quality: "Original".into(),
                available,
                error: (!available).then(|| "libmpv aún no está provisionado".into()),
                ..PlayerState::default()
            }),
        }
    }

    pub fn available(&self) -> bool { self.library_path.is_some() }
    pub fn state(&self) -> PlayerState { self.state.lock().clone() }

    // This boundary is complete and transport-safe. The native surface/FFI adapter is attached in the player phase.
    pub fn execute(&self, command: PlayerCommand, resolved_title: Option<String>, _resolved_path: Option<PathBuf>) -> Result<PlayerState> {
        let mut state = self.state.lock();
        match command {
            PlayerCommand::Play { media_id } => {
                if !state.available { anyhow::bail!(state.error.clone().unwrap_or_else(|| "player unavailable".into())); }
                state.media_id = media_id;
                state.title = resolved_title;
                state.playing = true;
            }
            PlayerCommand::Pause => state.playing = false,
            PlayerCommand::TogglePlayback => state.playing = !state.playing,
            PlayerCommand::Stop => { state.playing=false; state.position_ms=0; state.media_id=None; state.title=None; }
            PlayerCommand::SeekAbsolute { position_ms } => state.position_ms = position_ms.clamp(0, state.duration_ms.max(0)),
            PlayerCommand::SeekRelative { seconds } => state.position_ms = (state.position_ms + (seconds*1000.0) as i64).clamp(0,state.duration_ms.max(0)),
            PlayerCommand::SetVolume { value } => state.volume=value.clamp(0.0,100.0),
            PlayerCommand::ToggleMute => state.muted=!state.muted,
            PlayerCommand::SelectAudioTrack { track_id } => state.audio_track_id=Some(track_id),
            PlayerCommand::SelectSubtitleTrack { track_id } => state.subtitle_track_id=Some(track_id),
            PlayerCommand::SetPlaybackSpeed { value } => state.playback_speed=value.clamp(0.25,4.0),
            PlayerCommand::SetImageProfile { profile: ImageProfile { .. } } => {},
            PlayerCommand::SetQuality { quality } => state.quality=quality,
            PlayerCommand::SetFullscreen { fullscreen } => state.fullscreen=fullscreen,
            PlayerCommand::GetPlayerState => {},
        }
        Ok(state.clone())
    }
}

