use anyhow::{Context, Result};
use cinewana_core::{ImageProfile, PlayerCommand, PlayerState};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

pub struct PlayerService {
    state: Mutex<PlayerState>,
    executable: Option<PathBuf>,
    child: Mutex<Option<Child>>,
    ipc_pipe: Mutex<Option<String>>,
}

impl PlayerService {
    pub fn discover() -> Self {
        let candidates = [
            PathBuf::from(r"C:\Program Files\MPV Player\mpv.exe"),
            PathBuf::from(r"C:\Program Files\mpv\mpv.exe"),
            PathBuf::from(r"C:\Program Files (x86)\MPV Player\mpv.exe"),
            PathBuf::from(".tools/mpv/mpv.exe"),
            PathBuf::from("mpv.exe"),
        ];
        let executable = candidates
            .into_iter()
            .find(|p| p.is_file())
            .or_else(find_mpv_on_path);
        let available = cfg!(windows) || executable.is_some();
        Self {
            executable,
            child: Mutex::new(None),
            ipc_pipe: Mutex::new(None),
            state: Mutex::new(PlayerState {
                volume: 70.0,
                playback_speed: 1.0,
                quality: "Original".into(),
                available,
                error: (!available).then(|| "No se encontró un reproductor disponible".into()),
                ..PlayerState::default()
            }),
        }
    }

    pub fn available(&self) -> bool {
        cfg!(windows) || self.executable.is_some()
    }
    pub fn state(&self) -> PlayerState {
        self.state.lock().clone()
    }

    pub fn is_running(&self) -> bool {
        let mut child = self.child.lock();
        let running = child
            .as_mut()
            .and_then(|process| process.try_wait().ok())
            .flatten()
            .is_none()
            && child.is_some();
        if !running {
            *child = None;
            self.state.lock().playing = false;
        }
        running
    }

    pub fn stop(&self) {
        let _ = self.send(&json!({"command":["quit"]}));
        std::thread::sleep(Duration::from_millis(80));
        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        *self.ipc_pipe.lock() = None;
        let mut state = self.state.lock();
        state.playing = false;
        state.position_ms = 0;
        state.media_id = None;
        state.title = None;
    }

    pub fn execute(
        &self,
        command: PlayerCommand,
        resolved_title: Option<String>,
        resolved_path: Option<PathBuf>,
        parent_hwnd: Option<isize>,
    ) -> Result<PlayerState> {
        match command {
            PlayerCommand::Play { media_id } => {
                let path = resolved_path.context("El archivo ya no está disponible")?;
                self.stop();
                let title = resolved_title.unwrap_or_else(|| "CINE WANA".into());
                #[cfg(windows)]
                if std::env::var_os("CINE_WANA_USE_MPV").is_none() {
                    open_with_windows_player(&path)?;
                    let mut state = self.state.lock();
                    state.media_id = media_id;
                    state.title = Some(title);
                    state.playing = true;
                    state.error = None;
                    state.available = true;
                    return Ok(state.clone());
                }

                let executable = self.executable.as_ref().context("No se encontró mpv")?;
                let pipe = format!(r"\\.\pipe\cine-wana-{}", Uuid::new_v4());
                let mut process = Command::new(executable);
                if let Some(hwnd) = parent_hwnd {
                    process.arg(format!("--wid={hwnd}"));
                } else {
                    process.arg("--fs=yes");
                }
                process
                    .arg(format!("--input-ipc-server={pipe}"))
                    .args([
                        "--force-window=immediate",
                        "--keep-open=no",
                        "--osc=yes",
                        "--input-default-bindings=yes",
                        "--input-cursor=yes",
                        "--hwdec=auto-safe",
                        "--no-terminal",
                        "--no-border",
                        "--autofit=100%x100%",
                    ])
                    .arg(format!("--title=CINE WANA — {title}"))
                    .arg(&path)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    process.creation_flags(0x08000000);
                }
                let child = process.spawn().context("No se pudo iniciar mpv")?;
                *self.child.lock() = Some(child);
                *self.ipc_pipe.lock() = Some(pipe);
                let mut state = self.state.lock();
                state.media_id = media_id;
                state.title = Some(title);
                state.playing = true;
                state.error = None;
                state.available = true;
            }
            PlayerCommand::Pause => {
                self.send(&json!({"command":["set_property","pause",true]}))?;
                self.state.lock().playing = false;
            }
            PlayerCommand::TogglePlayback => {
                self.send(&json!({"command":["cycle","pause"]}))?;
                let mut s = self.state.lock();
                s.playing = !s.playing;
            }
            PlayerCommand::Stop => self.stop(),
            PlayerCommand::SeekAbsolute { position_ms } => {
                self.send(&json!({"command":["seek",position_ms as f64/1000.0,"absolute"]}))?;
                self.state.lock().position_ms = position_ms;
            }
            PlayerCommand::SeekRelative { seconds } => {
                self.send(&json!({"command":["seek",seconds,"relative"]}))?;
            }
            PlayerCommand::SetVolume { value } => {
                let value = value.clamp(0.0, 100.0);
                self.send(&json!({"command":["set_property","volume",value]}))?;
                self.state.lock().volume = value;
            }
            PlayerCommand::ToggleMute => {
                self.send(&json!({"command":["cycle","mute"]}))?;
                let mut s = self.state.lock();
                s.muted = !s.muted;
            }
            PlayerCommand::SelectAudioTrack { track_id } => {
                self.send(&json!({"command":["set_property","aid",track_id]}))?;
                self.state.lock().audio_track_id = Some(track_id);
            }
            PlayerCommand::SelectSubtitleTrack { track_id } => {
                self.send(&json!({"command":["set_property","sid",track_id]}))?;
                self.state.lock().subtitle_track_id = Some(track_id);
            }
            PlayerCommand::SetPlaybackSpeed { value } => {
                let value = value.clamp(0.25, 4.0);
                self.send(&json!({"command":["set_property","speed",value]}))?;
                self.state.lock().playback_speed = value;
            }
            PlayerCommand::SetImageProfile { profile } => self.set_image_profile(&profile)?,
            PlayerCommand::SetQuality { quality } => self.state.lock().quality = quality,
            PlayerCommand::SetFullscreen { fullscreen } => {
                self.state.lock().fullscreen = fullscreen
            }
            PlayerCommand::GetPlayerState => {}
        }
        Ok(self.state())
    }

    fn set_image_profile(&self, profile: &ImageProfile) -> Result<()> {
        for (property, value) in [
            ("brightness", profile.brightness),
            ("contrast", profile.contrast),
            ("gamma", profile.gamma),
            ("saturation", profile.saturation),
            ("hue", profile.hue),
        ] {
            self.send(&json!({"command":["set_property",property,value]}))?;
        }
        Ok(())
    }

    fn send(&self, value: &Value) -> Result<()> {
        let pipe = self
            .ipc_pipe
            .lock()
            .clone()
            .context("El reproductor no está activo")?;
        let payload = format!("{}\n", serde_json::to_string(value)?);
        let mut last_error = None;
        for _ in 0..25 {
            match OpenOptions::new().write(true).open(&pipe) {
                Ok(mut stream) => {
                    stream.write_all(payload.as_bytes())?;
                    return Ok(());
                }
                Err(error) => {
                    last_error = Some(error);
                    std::thread::sleep(Duration::from_millis(40));
                }
            }
        }
        Err(last_error
            .map(anyhow::Error::from)
            .unwrap_or_else(|| anyhow::anyhow!("No se pudo controlar mpv")))
    }
}

#[cfg(windows)]
fn open_with_windows_player(path: &std::path::Path) -> Result<()> {
    let player = windows_media_player().context("No se encontró Windows Media Player")?;
    let mut process = Command::new(player);
    process
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    use std::os::windows::process::CommandExt;
    process.creation_flags(0x08000000);
    process
        .spawn()
        .context("No se pudo abrir el reproductor de Windows")?;
    Ok(())
}

#[cfg(windows)]
fn windows_media_player() -> Option<PathBuf> {
    [
        std::env::var_os("ProgramFiles").map(PathBuf::from),
        std::env::var_os("ProgramFiles(x86)").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    .map(|root| root.join(r"Windows Media Player\wmplayer.exe"))
    .find(|path| path.is_file())
}

fn find_mpv_on_path() -> Option<PathBuf> {
    let executable = "mpv.exe";
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(executable))
            .find(|p| p.is_file())
    })
}
