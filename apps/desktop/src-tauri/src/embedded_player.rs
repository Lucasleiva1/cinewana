//! Puente entre la interfaz y el reproductor incrustado (libmpv dentro de la ventana).
//!
//! libmpv viaja adentro de CINE WANA: en el instalador va en la carpeta de recursos `mpv/`. Antes
//! de cargarla en la aplicación se prueba en un proceso aparte, así una compilación incompatible
//! con el procesador nunca puede tirar abajo CINE WANA: en ese caso se usa el reproductor anterior.

use crate::{AppServices, error_string};
use cinewana_player::embedded::{
    EmbeddedCommand, EmbeddedImage, EmbeddedPlayer, EmbeddedState, LibMpv, OpenRequest,
    probe_library,
};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, State, WebviewWindow};

pub const PROBE_FLAG: &str = "--cinewana-probar-libmpv";
const STATE_EVENT: &str = "embedded-player-state";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedStatusDto {
    pub available: bool,
    pub message: Option<String>,
}

enum HostState {
    Unknown,
    Ready(Arc<EmbeddedPlayer>),
    Unavailable(String),
}

pub struct EmbeddedPlayerHost {
    app: AppHandle,
    library: Option<PathBuf>,
    data_dir: PathBuf,
    state: Mutex<HostState>,
}

impl EmbeddedPlayerHost {
    pub fn new(app: AppHandle, resource_dir: &Path, data_dir: PathBuf) -> Self {
        Self {
            app,
            library: locate_library(resource_dir),
            data_dir,
            state: Mutex::new(HostState::Unknown),
        }
    }

    fn player(&self) -> Result<Arc<EmbeddedPlayer>, String> {
        let mut state = self.state.lock();
        match &*state {
            HostState::Ready(player) => return Ok(player.clone()),
            HostState::Unavailable(message) => return Err(message.clone()),
            HostState::Unknown => {}
        }
        let result = self.prepare();
        *state = match &result {
            Ok(player) => HostState::Ready(player.clone()),
            Err(message) => HostState::Unavailable(message.clone()),
        };
        result
    }

    fn prepare(&self) -> Result<Arc<EmbeddedPlayer>, String> {
        let library = self
            .library
            .clone()
            .ok_or_else(|| "Falta libmpv-2.dll en la carpeta de CINE WANA".to_string())?;
        probe_in_child_process(&library)?;
        let lib = LibMpv::load(&library).map_err(error_string)?;
        let app = self.app.clone();
        let listener = Arc::new(move |state: EmbeddedState| {
            let _ = app.emit(STATE_EVENT, state);
        });
        // Las fuentes de subtítulos (Montserrat) viajan al lado de libmpv, en su carpeta "fonts".
        let fonts_dir = library
            .parent()
            .map(|folder| folder.join("fonts"))
            .filter(|folder| folder.is_dir());
        EmbeddedPlayer::new(lib, self.data_dir.clone(), fonts_dir, listener)
            .map(Arc::new)
            .map_err(error_string)
    }

    pub fn status(&self) -> EmbeddedStatusDto {
        match self.player() {
            Ok(_) => EmbeddedStatusDto {
                available: true,
                message: None,
            },
            Err(message) => EmbeddedStatusDto {
                available: false,
                message: Some(message),
            },
        }
    }
}

/// Solo se usa la copia que trae CINE WANA. En desarrollo, además, la de la carpeta del proyecto.
fn locate_library(resource_dir: &Path) -> Option<PathBuf> {
    let bundled = resource_dir.join("mpv").join("libmpv-2.dll");
    if bundled.is_file() {
        return Some(bundled);
    }
    #[cfg(debug_assertions)]
    {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("lib")
            .join("libmpv-2.dll");
        if project.is_file() {
            return Some(project);
        }
    }
    None
}

fn probe_in_child_process(library: &Path) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(error_string)?;
    let mut command = Command::new(executable);
    command
        .arg(PROBE_FLAG)
        .arg(library)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("No se pudo probar libmpv: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "libmpv no funciona en esta computadora (código {})",
                    status.code().unwrap_or(-1)
                ));
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(40)),
            Ok(None) => {
                let _ = child.kill();
                return Err("La prueba de libmpv tardó demasiado".into());
            }
            Err(error) => return Err(format!("No se pudo probar libmpv: {error}")),
        }
    }
}

/// Lo ejecuta el proceso de prueba (ver `probe_in_child_process`). Devuelve el código de salida.
pub fn run_probe(arguments: &[std::ffi::OsString]) -> Option<i32> {
    let mut arguments = arguments.iter();
    if arguments.next()? != PROBE_FLAG {
        return None;
    }
    let library = PathBuf::from(arguments.next()?);
    Some(match probe_library(&library) {
        Ok(_) => 0,
        Err(_) => 2,
    })
}

#[tauri::command]
pub async fn embedded_player_status(
    state: State<'_, AppServices>,
) -> Result<EmbeddedStatusDto, String> {
    let host = state.embedded.clone();
    tauri::async_runtime::spawn_blocking(move || host.status())
        .await
        .map_err(error_string)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn embedded_player_open(
    media_id: String,
    start_seconds: f64,
    volume: f64,
    muted: bool,
    image: EmbeddedImage,
    window: WebviewWindow,
    state: State<'_, AppServices>,
) -> Result<u64, String> {
    let path = state
        .db
        .media_path(&media_id)
        .map_err(error_string)?
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .ok_or_else(|| "El archivo ya no está disponible".to_string())?;
    let window_handle = window.hwnd().map_err(error_string)?.0 as isize;
    let host = state.embedded.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let player = host.player()?;
        player
            .open(OpenRequest {
                window_handle,
                media_id,
                path,
                start_seconds,
                volume,
                muted,
                image,
            })
            .map_err(error_string)
    })
    .await
    .map_err(error_string)?
}

#[tauri::command]
pub async fn embedded_player_command(
    command: EmbeddedCommand,
    state: State<'_, AppServices>,
) -> Result<(), String> {
    let host = state.embedded.clone();
    tauri::async_runtime::spawn_blocking(move || {
        host.player()?.command(command).map_err(error_string)
    })
    .await
    .map_err(error_string)?
}

#[tauri::command]
pub async fn embedded_player_stop(state: State<'_, AppServices>) -> Result<(), String> {
    let host = state.embedded.clone();
    tauri::async_runtime::spawn_blocking(move || host.player()?.stop().map_err(error_string))
        .await
        .map_err(error_string)?
}
