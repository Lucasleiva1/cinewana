//! Reproductor incrustado con libmpv.
//!
//! libmpv (el motor de mpv) dibuja el video dentro de la ventana de CINE WANA, debajo de la
//! interfaz web, y CINE WANA le da órdenes con su API oficial: comandos, propiedades y eventos. La
//! biblioteca viaja adentro del instalador; este módulo solo recibe la ruta del archivo y nunca
//! busca un mpv instalado en la computadora.

use anyhow::{Context, Result, anyhow, bail};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_double, c_int, c_ulong, c_void},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const SHADER_SOURCE: &str = include_str!("cinewana-imagen.glsl");
const SHADER_FILE: &str = "cinewana-imagen.glsl";
const CHOICES_FILE: &str = "pistas-elegidas.json";
const SUBTITLE_STYLE_FILE: &str = "estilo-subtitulos.json";
const LOG_FILE: &str = "mpv-ultimo.log";

/// Volumen de la interfaz: 0.5 es el 100 % normal y 1.0 el refuerzo máximo (doble de señal).
pub const UI_NORMAL_VOLUME: f64 = 0.5;
const MPV_VOLUME_MAX: f64 = 130.0;
/// Cada cuánto se avisa la posición mientras el video corre. Los demás cambios salen al instante.
const POSITION_EMIT_INTERVAL: Duration = Duration::from_millis(200);

// ---------------------------------------------------------------------------------------------
// API de C de libmpv (client.h, versión 2.x)
// ---------------------------------------------------------------------------------------------

type RawHandle = *mut c_void;

#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct MpvEventProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct MpvEventEndFile {
    reason: c_int,
    error: c_int,
    playlist_entry_id: i64,
    playlist_insert_id: i64,
    playlist_insert_num_entries: c_int,
}

const FORMAT_NONE: c_int = 0;
const FORMAT_STRING: c_int = 1;
const FORMAT_FLAG: c_int = 3;
const FORMAT_DOUBLE: c_int = 5;

const EVENT_NONE: c_int = 0;
const EVENT_SHUTDOWN: c_int = 1;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const EVENT_VIDEO_RECONFIG: c_int = 17;
const EVENT_PROPERTY_CHANGE: c_int = 22;
const END_FILE_REASON_ERROR: c_int = 4;

struct Api {
    create: unsafe extern "C" fn() -> RawHandle,
    initialize: unsafe extern "C" fn(RawHandle) -> c_int,
    set_option_string: unsafe extern "C" fn(RawHandle, *const c_char, *const c_char) -> c_int,
    set_property_string: unsafe extern "C" fn(RawHandle, *const c_char, *const c_char) -> c_int,
    get_property_string: unsafe extern "C" fn(RawHandle, *const c_char) -> *mut c_char,
    command: unsafe extern "C" fn(RawHandle, *mut *const c_char) -> c_int,
    observe_property: unsafe extern "C" fn(RawHandle, u64, *const c_char, c_int) -> c_int,
    wait_event: unsafe extern "C" fn(RawHandle, c_double) -> *mut MpvEvent,
    wakeup: unsafe extern "C" fn(RawHandle),
    free: unsafe extern "C" fn(*mut c_void),
    terminate_destroy: unsafe extern "C" fn(RawHandle),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    client_api_version: unsafe extern "C" fn() -> c_ulong,
}

/// libmpv-2.dll cargada en memoria. Se carga una sola vez y queda viva mientras dure la app.
pub struct LibMpv {
    api: Api,
    path: PathBuf,
}

impl LibMpv {
    pub fn load(path: &Path) -> Result<Arc<Self>> {
        let api = load_api(path)?;
        let version = unsafe { (api.client_api_version)() };
        if (version >> 16) != 2 {
            bail!("La versión de libmpv no es compatible ({version:#x})");
        }
        Ok(Arc::new(Self {
            api,
            path: path.to_path_buf(),
        }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(windows)]
fn load_api(path: &Path) -> Result<Api> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // LOAD_WITH_ALTERED_SEARCH_PATH: lo que libmpv necesite se busca primero en su propia carpeta.
    let module = unsafe { win::LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), 0x0000_0008) };
    if module.is_null() {
        bail!("No se pudo cargar libmpv desde {}", path.display());
    }
    let symbol = |name: &str| -> Result<*mut c_void> {
        let c_name = CString::new(name).expect("nombre de función sin ceros");
        let pointer = unsafe { win::GetProcAddress(module, c_name.as_ptr().cast()) };
        if pointer.is_null() {
            Err(anyhow!("libmpv no trae la función {name}"))
        } else {
            Ok(pointer)
        }
    };
    macro_rules! function {
        ($name:literal) => {
            unsafe { std::mem::transmute(symbol($name)?) }
        };
    }
    Ok(Api {
        create: function!("mpv_create"),
        initialize: function!("mpv_initialize"),
        set_option_string: function!("mpv_set_option_string"),
        set_property_string: function!("mpv_set_property_string"),
        get_property_string: function!("mpv_get_property_string"),
        command: function!("mpv_command"),
        observe_property: function!("mpv_observe_property"),
        wait_event: function!("mpv_wait_event"),
        wakeup: function!("mpv_wakeup"),
        free: function!("mpv_free"),
        terminate_destroy: function!("mpv_terminate_destroy"),
        error_string: function!("mpv_error_string"),
        client_api_version: function!("mpv_client_api_version"),
    })
}

#[cfg(not(windows))]
fn load_api(_path: &Path) -> Result<Api> {
    bail!("El reproductor incrustado solo funciona en Windows")
}

/// Carga libmpv, la inicializa sin imagen ni sonido y la cierra. Se ejecuta en un proceso aparte
/// antes de usarla dentro de CINE WANA: si la compilación no fuera compatible con el procesador,
/// el que se cae es ese proceso de prueba y no la aplicación.
pub fn probe_library(path: &Path) -> Result<String> {
    let lib = LibMpv::load(path)?;
    let handle = unsafe { (lib.api.create)() };
    if handle.is_null() {
        bail!("libmpv no pudo crear el reproductor");
    }
    let core = Core {
        lib,
        handle: handle as usize,
    };
    let _ = core.set_option("vo", "null");
    let _ = core.set_option("ao", "null");
    let code = unsafe { (core.lib.api.initialize)(core.raw()) };
    let version = core.get_string("mpv-version");
    unsafe { (core.lib.api.terminate_destroy)(core.raw()) };
    if code < 0 {
        bail!("libmpv no pudo arrancar: {}", core.error_text(code));
    }
    Ok(version.unwrap_or_default())
}

#[derive(Clone)]
struct Core {
    lib: Arc<LibMpv>,
    /// El puntero de mpv guardado como número: la API de libmpv se puede usar desde cualquier hilo.
    handle: usize,
}

impl Core {
    fn raw(&self) -> RawHandle {
        self.handle as RawHandle
    }

    fn error_text(&self, code: c_int) -> String {
        let text = unsafe { (self.lib.api.error_string)(code) };
        if text.is_null() {
            format!("error {code}")
        } else {
            unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
        }
    }

    fn check(&self, code: c_int, what: &str) -> Result<()> {
        if code < 0 {
            bail!("mpv rechazó {what}: {}", self.error_text(code));
        }
        Ok(())
    }

    fn set_option(&self, name: &str, value: &str) -> Result<()> {
        let (name_c, value_c) = (CString::new(name)?, CString::new(value)?);
        let code =
            unsafe { (self.lib.api.set_option_string)(self.raw(), name_c.as_ptr(), value_c.as_ptr()) };
        self.check(code, name)
    }

    fn set_property(&self, name: &str, value: impl ToString) -> Result<()> {
        let (name_c, value_c) = (CString::new(name)?, CString::new(value.to_string())?);
        let code = unsafe {
            (self.lib.api.set_property_string)(self.raw(), name_c.as_ptr(), value_c.as_ptr())
        };
        self.check(code, name)
    }

    fn get_string(&self, name: &str) -> Option<String> {
        let name_c = CString::new(name).ok()?;
        let pointer = unsafe { (self.lib.api.get_property_string)(self.raw(), name_c.as_ptr()) };
        if pointer.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned();
        unsafe { (self.lib.api.free)(pointer.cast()) };
        Some(text)
    }

    fn command(&self, args: &[&str]) -> Result<()> {
        let owned = args
            .iter()
            .map(|arg| CString::new(*arg))
            .collect::<Result<Vec<_>, _>>()?;
        let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(std::ptr::null());
        let code = unsafe { (self.lib.api.command)(self.raw(), pointers.as_mut_ptr()) };
        self.check(code, args.first().copied().unwrap_or("un comando"))
    }

    fn observe(&self, id: u64, name: &str, format: c_int) {
        if let Ok(name_c) = CString::new(name) {
            unsafe { (self.lib.api.observe_property)(self.raw(), id, name_c.as_ptr(), format) };
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Lo que ve la interfaz
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedState {
    /// Cada película abierta tiene su número: la interfaz ignora avisos de una sesión anterior.
    pub session: u64,
    pub loaded: bool,
    /// mpv ya está dibujando imagen; antes de esto la interfaz mantiene el fondo negro.
    pub video_ready: bool,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub ended: bool,
    pub buffering: bool,
    /// Mismo volumen que la interfaz (0.5 = 100 %).
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub audio_id: Option<i64>,
    pub subtitle_id: Option<i64>,
    pub subtitle_scale: f64,
    pub subtitle_delay: f64,
    pub subtitle_style: SubtitleStyle,
    /// Decodificador por placa de video en uso ("d3d11va", "nvdec"...) o None si decodifica el procesador.
    pub hardware_decoder: Option<String>,
    pub tracks: Vec<EmbeddedTrack>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedTrack {
    pub id: i64,
    /// "video", "audio" o "sub".
    pub kind: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub codec: Option<String>,
    pub channels: Option<i64>,
    pub external: bool,
    pub selected: bool,
    pub default_track: bool,
    pub forced: bool,
    pub hearing_impaired: bool,
}

/// Cómo se ven los subtítulos de texto. Vale para todas las películas y queda guardado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SubtitleStyle {
    /// "blanco" o "amarillo".
    pub color: String,
    /// "clasica", "montserrat", "segoe" o "verdana".
    pub font: String,
    /// Altura elegida: 100 es abajo de todo, 60 bastante más arriba (en % de la pantalla).
    pub position: f64,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            color: "blanco".into(),
            font: "clasica".into(),
            position: 100.0,
        }
    }
}

/// Ajustes de imagen en la escala de la interfaz (-50 a 50; nitidez, zoom e intensidades de 0 a 100).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct EmbeddedImage {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub gamma: f64,
    pub hue: f64,
    pub shadows: f64,
    pub highlights: f64,
    pub temperature: f64,
    pub sharpness: f64,
    /// "normal" (el de mpv), "alta" (ewa_lanczossharp) o "suave" (mitchell, para animación y fuentes chicas).
    pub scaling: String,
    /// Reduce los halos claros que deja el agrandado alrededor de los bordes.
    pub antiring: bool,
    pub deband: bool,
    /// 0 a 100: cuántas pasadas hace el quitado de bandas.
    pub deband_strength: f64,
    pub deinterlace: bool,
    /// Interpolación de cuadros sincronizada con la pantalla.
    pub smooth_motion: bool,
    pub fill_screen: bool,
    /// "original", "16:9", "4:3", "2.35:1" o "1.85:1".
    pub aspect: String,
    /// 0 a 100 % de acercamiento.
    pub zoom: f64,
    /// 0, 90, 180 o 270 grados.
    pub rotate: i64,
    /// Para películas HDR: "auto", "bt.2390", "hable", "mobius" o "reinhard".
    pub tone_mapping: String,
    /// Para películas HDR: mide el brillo de cada escena para adaptarlo a la pantalla.
    pub hdr_auto_peak: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EmbeddedCommand {
    SetPause {
        paused: bool,
    },
    /// `fast` salta al cuadro clave más cercano: sirve mientras se arrastra la barra.
    SeekTo {
        seconds: f64,
        #[serde(default)]
        fast: bool,
    },
    SeekBy {
        seconds: f64,
    },
    SetVolume {
        volume: f64,
    },
    SetMuted {
        muted: bool,
    },
    SetSpeed {
        speed: f64,
    },
    SetAudioTrack {
        id: i64,
    },
    /// `None` apaga los subtítulos.
    SetSubtitleTrack {
        id: Option<i64>,
    },
    SetSubtitleScale {
        scale: f64,
    },
    SetSubtitleDelay {
        seconds: f64,
    },
    /// Altura de los subtítulos en % de la pantalla (100 = abajo de todo). Suben mientras se ven
    /// los controles para que la barra no los tape.
    SetSubtitlePosition {
        percent: f64,
    },
    /// Color, fuente y altura elegidos; se guardan para todas las películas.
    SetSubtitleStyle {
        style: SubtitleStyle,
    },
    SetImage {
        image: EmbeddedImage,
    },
    /// Parte de la ventana (0 a 0.6) que tapa un panel lateral: el video se corre a la izquierda.
    SetVideoMargins {
        right: f64,
    },
}

pub struct OpenRequest {
    pub window_handle: isize,
    pub media_id: String,
    pub path: PathBuf,
    pub start_seconds: f64,
    pub volume: f64,
    pub muted: bool,
    pub image: EmbeddedImage,
}

pub type StateListener = Arc<dyn Fn(EmbeddedState) + Send + Sync>;

// ---------------------------------------------------------------------------------------------
// El reproductor
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackRef {
    id: i64,
    language: Option<String>,
    title: Option<String>,
}

/// Lo que eligió el usuario en una película, para volver a ponerlo la próxima vez.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackChoice {
    audio: Option<TrackRef>,
    subtitle: Option<TrackRef>,
    subtitles_off: bool,
}

#[derive(Default)]
struct Shared {
    state: EmbeddedState,
    media_id: Option<String>,
    choices: HashMap<String, TrackChoice>,
}

#[derive(Clone, Default)]
struct ScaleDefaults {
    scale: String,
    cscale: String,
    dscale: String,
}

struct Instance {
    core: Core,
    window_handle: isize,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    scale_defaults: ScaleDefaults,
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        unsafe { (self.core.lib.api.wakeup)(self.core.raw()) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        unsafe { (self.core.lib.api.terminate_destroy)(self.core.raw()) };
    }
}

pub struct EmbeddedPlayer {
    lib: Arc<LibMpv>,
    data_dir: PathBuf,
    listener: StateListener,
    shared: Arc<Mutex<Shared>>,
    instance: Mutex<Option<Instance>>,
    session: AtomicU64,
    /// Último ajuste de imagen que se le mandó a mpv.
    last_image: Mutex<Option<EmbeddedImage>>,
    /// Fuentes que trae CINE WANA para los subtítulos (Montserrat).
    fonts_dir: Option<PathBuf>,
}

impl EmbeddedPlayer {
    pub fn new(
        lib: Arc<LibMpv>,
        data_dir: PathBuf,
        fonts_dir: Option<PathBuf>,
        listener: StateListener,
    ) -> Result<Self> {
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("No se pudo crear {}", data_dir.display()))?;
        std::fs::write(data_dir.join(SHADER_FILE), SHADER_SOURCE)
            .context("No se pudo preparar el filtro de imagen de mpv")?;
        let choices = std::fs::read_to_string(data_dir.join(CHOICES_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let subtitle_style: SubtitleStyle = std::fs::read_to_string(data_dir.join(SUBTITLE_STYLE_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Ok(Self {
            lib,
            data_dir,
            listener,
            shared: Arc::new(Mutex::new(Shared {
                choices,
                state: EmbeddedState {
                    subtitle_style,
                    ..EmbeddedState::default()
                },
                ..Shared::default()
            })),
            instance: Mutex::new(None),
            session: AtomicU64::new(0),
            last_image: Mutex::new(None),
            fonts_dir,
        })
    }

    pub fn library_path(&self) -> &Path {
        self.lib.path()
    }

    pub fn state(&self) -> EmbeddedState {
        self.shared.lock().state.clone()
    }

    pub fn open(&self, request: OpenRequest) -> Result<u64> {
        let (core, scale_defaults) = self.ensure_instance(request.window_handle)?;
        let session = self.session.fetch_add(1, Ordering::AcqRel) + 1;
        let start = request.start_seconds.max(0.0);
        let subtitle_style = {
            let mut shared = self.shared.lock();
            shared.media_id = Some(request.media_id.clone());
            let subtitle_style = shared.state.subtitle_style.clone();
            shared.state = EmbeddedState {
                session,
                position: start,
                volume: request.volume.clamp(0.0, 1.0),
                muted: request.muted,
                speed: 1.0,
                subtitle_scale: 1.0,
                subtitle_style: subtitle_style.clone(),
                ..EmbeddedState::default()
            };
            subtitle_style
        };
        apply_subtitle_style(&core, &subtitle_style);
        for (name, value) in [
            ("pause", "no".to_string()),
            ("speed", "1".to_string()),
            ("sub-delay", "0".to_string()),
            ("sub-scale", "1".to_string()),
            ("aid", "auto".to_string()),
            ("sid", "auto".to_string()),
            ("video-margin-ratio-right", "0".to_string()),
            ("volume", format_number(ui_volume_to_mpv(request.volume))),
            ("mute", yes_no(request.muted).to_string()),
        ] {
            let _ = core.set_property(name, value);
        }
        apply_image(&core, &request.image, None, &scale_defaults);
        *self.last_image.lock() = Some(request.image.clone());
        let path = request.path.to_string_lossy().into_owned();
        if start > 0.5 {
            let options = format!("start={}", format_number(start));
            core.command(&["loadfile", &path, "replace", "-1", &options])?;
        } else {
            core.command(&["loadfile", &path, "replace"])?;
        }
        (self.listener)(self.state());
        Ok(session)
    }

    pub fn command(&self, command: EmbeddedCommand) -> Result<()> {
        let core = self.current_core()?;
        match command {
            EmbeddedCommand::SetPause { paused } => core.set_property("pause", yes_no(paused)),
            EmbeddedCommand::SeekTo { seconds, fast } => {
                let mode = if fast { "absolute+keyframes" } else { "absolute+exact" };
                core.command(&["seek", &format_number(seconds.max(0.0)), mode])
            }
            EmbeddedCommand::SeekBy { seconds } => {
                core.command(&["seek", &format_number(seconds), "relative+exact"])
            }
            EmbeddedCommand::SetVolume { volume } => {
                core.set_property("volume", format_number(ui_volume_to_mpv(volume)))
            }
            EmbeddedCommand::SetMuted { muted } => core.set_property("mute", yes_no(muted)),
            EmbeddedCommand::SetSpeed { speed } => {
                core.set_property("speed", format_number(speed.clamp(0.25, 4.0)))
            }
            EmbeddedCommand::SetAudioTrack { id } => {
                core.set_property("aid", id)?;
                self.remember_choice(|choice, tracks| {
                    choice.audio = track_ref(tracks, "audio", id);
                });
                Ok(())
            }
            EmbeddedCommand::SetSubtitleTrack { id } => {
                match id {
                    Some(id) => core.set_property("sid", id)?,
                    None => core.set_property("sid", "no")?,
                }
                self.remember_choice(|choice, tracks| {
                    choice.subtitles_off = id.is_none();
                    choice.subtitle = id.and_then(|id| track_ref(tracks, "sub", id));
                });
                Ok(())
            }
            EmbeddedCommand::SetSubtitleScale { scale } => {
                core.set_property("sub-scale", format_number(scale.clamp(0.4, 3.0)))
            }
            EmbeddedCommand::SetSubtitleDelay { seconds } => {
                core.set_property("sub-delay", format_number(seconds.clamp(-60.0, 60.0)))
            }
            EmbeddedCommand::SetSubtitlePosition { percent } => {
                core.set_property("sub-pos", format_number(percent.clamp(40.0, 100.0).round()))
            }
            EmbeddedCommand::SetSubtitleStyle { style } => {
                let style = SubtitleStyle {
                    position: style.position.clamp(40.0, 100.0),
                    ..style
                };
                apply_subtitle_style(&core, &style);
                let state = {
                    let mut shared = self.shared.lock();
                    shared.state.subtitle_style = style.clone();
                    shared.state.clone()
                };
                if let Ok(text) = serde_json::to_string_pretty(&style) {
                    let _ = std::fs::write(self.data_dir.join(SUBTITLE_STYLE_FILE), text);
                }
                (self.listener)(state);
                Ok(())
            }
            EmbeddedCommand::SetImage { image } => {
                let defaults = self
                    .instance
                    .lock()
                    .as_ref()
                    .map(|instance| instance.scale_defaults.clone())
                    .unwrap_or_default();
                let mut last = self.last_image.lock();
                apply_image(&core, &image, last.as_ref(), &defaults);
                *last = Some(image);
                Ok(())
            }
            EmbeddedCommand::SetVideoMargins { right } => core.set_property(
                "video-margin-ratio-right",
                format_number(right.clamp(0.0, 0.6)),
            ),
        }
    }

    /// Descarga la película: mpv cierra su imagen y la ventana queda solo con la interfaz.
    pub fn stop(&self) -> Result<()> {
        if let Ok(core) = self.current_core() {
            core.command(&["stop"])?;
        }
        let state = {
            let mut shared = self.shared.lock();
            shared.media_id = None;
            shared.state.loaded = false;
            shared.state.video_ready = false;
            shared.state.paused = true;
            shared.state.clone()
        };
        (self.listener)(state);
        Ok(())
    }

    fn current_core(&self) -> Result<Core> {
        self.instance
            .lock()
            .as_ref()
            .map(|instance| instance.core.clone())
            .context("El reproductor todavía no abrió ningún archivo")
    }

    fn remember_choice(&self, update: impl FnOnce(&mut TrackChoice, &[EmbeddedTrack])) {
        let snapshot = {
            let mut shared = self.shared.lock();
            let Some(media_id) = shared.media_id.clone() else {
                return;
            };
            let tracks = shared.state.tracks.clone();
            let choice = shared.choices.entry(media_id).or_default();
            update(choice, &tracks);
            shared.choices.clone()
        };
        if let Ok(text) = serde_json::to_string_pretty(&snapshot) {
            let _ = std::fs::write(self.data_dir.join(CHOICES_FILE), text);
        }
    }

    fn ensure_instance(&self, window_handle: isize) -> Result<(Core, ScaleDefaults)> {
        let mut guard = self.instance.lock();
        if let Some(instance) = guard.as_ref() {
            if instance.window_handle == window_handle {
                return Ok((instance.core.clone(), instance.scale_defaults.clone()));
            }
        }
        // La ventana cambió (no debería pasar): se cierra el reproductor viejo antes de crear otro.
        *guard = None;
        let instance = self.create_instance(window_handle)?;
        let result = (instance.core.clone(), instance.scale_defaults.clone());
        *guard = Some(instance);
        Ok(result)
    }

    fn create_instance(&self, window_handle: isize) -> Result<Instance> {
        let handle = unsafe { (self.lib.api.create)() };
        if handle.is_null() {
            bail!("libmpv no pudo crear el reproductor");
        }
        let core = Core {
            lib: self.lib.clone(),
            handle: handle as usize,
        };
        let shader = self.data_dir.join(SHADER_FILE);
        let log = self.data_dir.join(LOG_FILE);
        let mut options: Vec<(&str, String)> = vec![
            ("wid", window_handle.to_string()),
            ("config", "no".into()),
            ("terminal", "no".into()),
            ("vo", "gpu-next".into()),
            ("hwdec", "auto-safe".into()),
            ("keep-open", "yes".into()),
            ("idle", "yes".into()),
            ("force-window", "no".into()),
            ("input-default-bindings", "no".into()),
            ("input-vo-keyboard", "no".into()),
            ("input-cursor", "no".into()),
            ("cursor-autohide", "no".into()),
            ("osc", "no".into()),
            ("osd-level", "0".into()),
            ("load-scripts", "no".into()),
            ("ytdl", "no".into()),
            ("load-stats-overlay", "no".into()),
            ("load-console", "no".into()),
            ("load-select", "no".into()),
            ("load-positioning", "no".into()),
            ("load-commands", "no".into()),
            ("load-context-menu", "no".into()),
            ("load-auto-profiles", "no".into()),
            ("audio-client-name", "CINE WANA".into()),
            ("volume-max", format_number(MPV_VOLUME_MAX)),
            ("sub-auto", "fuzzy".into()),
            ("sub-file-paths", "Subs;subs;Subtitles;subtitles;Subtitulos;subtitulos".into()),
            ("audio-file-auto", "no".into()),
            ("alang", "lat,es-419,es-la,spa,es,esp".into()),
            ("slang", "lat,es-419,es-la,spa,es,esp".into()),
            ("subs-with-matching-audio", "forced".into()),
            ("glsl-shaders", shader.to_string_lossy().into_owned()),
            ("log-file", log.to_string_lossy().into_owned()),
        ];
        if let Some(fonts_dir) = &self.fonts_dir {
            options.push(("sub-fonts-dir", fonts_dir.to_string_lossy().into_owned()));
        }
        for (name, value) in &options {
            // Una opción que esta versión no conozca no frena al reproductor: queda en el registro.
            let _ = core.set_option(name, value);
        }
        let code = unsafe { (core.lib.api.initialize)(core.raw()) };
        if code < 0 {
            let text = core.error_text(code);
            unsafe { (core.lib.api.terminate_destroy)(core.raw()) };
            bail!("libmpv no pudo arrancar: {text}");
        }
        for (id, name, format) in OBSERVED {
            core.observe(*id, name, *format);
        }
        let scale_defaults = ScaleDefaults {
            scale: core.get_string("scale").unwrap_or_else(|| "lanczos".into()),
            cscale: core.get_string("cscale").unwrap_or_else(|| "lanczos".into()),
            dscale: core.get_string("dscale").unwrap_or_else(|| "hermite".into()),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let core = core.clone();
            let shared = self.shared.clone();
            let listener = self.listener.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("cinewana-mpv-eventos".into())
                .spawn(move || event_loop(core, shared, listener, stop, window_handle))
                .context("No se pudo iniciar el hilo de eventos de mpv")?
        };
        Ok(Instance {
            core,
            window_handle,
            stop,
            thread: Some(thread),
            scale_defaults,
        })
    }
}

const OBSERVED: &[(u64, &str, c_int)] = &[
    (1, "time-pos", FORMAT_DOUBLE),
    (2, "duration", FORMAT_DOUBLE),
    (3, "pause", FORMAT_FLAG),
    (4, "eof-reached", FORMAT_FLAG),
    (5, "paused-for-cache", FORMAT_FLAG),
    (6, "volume", FORMAT_DOUBLE),
    (7, "mute", FORMAT_FLAG),
    (8, "track-list", FORMAT_NONE),
    (9, "aid", FORMAT_STRING),
    (10, "sid", FORMAT_STRING),
    (11, "vo-configured", FORMAT_FLAG),
    (12, "speed", FORMAT_DOUBLE),
    (13, "sub-scale", FORMAT_DOUBLE),
    (14, "sub-delay", FORMAT_DOUBLE),
    (15, "hwdec-current", FORMAT_STRING),
];

fn event_loop(
    core: Core,
    shared: Arc<Mutex<Shared>>,
    listener: StateListener,
    stop: Arc<AtomicBool>,
    window_handle: isize,
) {
    let mut last_emit = Instant::now();
    let mut position_pending = false;
    while !stop.load(Ordering::Acquire) {
        let event = unsafe { (core.lib.api.wait_event)(core.raw(), 0.1) };
        if event.is_null() {
            continue;
        }
        let event = unsafe { &*event };
        let mut urgent = false;
        match event.event_id {
            EVENT_NONE => {}
            EVENT_SHUTDOWN => break,
            EVENT_PROPERTY_CHANGE if !event.data.is_null() => {
                let property = unsafe { &*(event.data as *const MpvEventProperty) };
                let refresh_tracks = matches!(event.reply_userdata, 8 | 9 | 10);
                apply_property(&shared, event.reply_userdata, property);
                if refresh_tracks {
                    update_tracks(&core, &shared);
                }
                if event.reply_userdata == 1 {
                    position_pending = true;
                } else {
                    urgent = true;
                }
            }
            EVENT_FILE_LOADED => {
                {
                    let mut shared = shared.lock();
                    shared.state.loaded = true;
                    shared.state.ended = false;
                    shared.state.error = None;
                }
                update_tracks(&core, &shared);
                restore_choice(&core, &shared);
                urgent = true;
            }
            EVENT_END_FILE if !event.data.is_null() => {
                let end = unsafe { &*(event.data as *const MpvEventEndFile) };
                if end.reason == END_FILE_REASON_ERROR {
                    let detail = core.error_text(end.error);
                    shared.lock().state.error = Some(format!(
                        "mpv no pudo reproducir este archivo ({detail}). Probá Abrir externo."
                    ));
                    urgent = true;
                }
            }
            EVENT_VIDEO_RECONFIG => keep_video_below_interface(window_handle),
            _ => {}
        }
        if urgent || (position_pending && last_emit.elapsed() >= POSITION_EMIT_INTERVAL) {
            let state = shared.lock().state.clone();
            listener(state);
            last_emit = Instant::now();
            position_pending = false;
        }
    }
}

fn apply_property(shared: &Mutex<Shared>, id: u64, property: &MpvEventProperty) {
    let double = || {
        (property.format == FORMAT_DOUBLE && !property.data.is_null())
            .then(|| unsafe { *(property.data as *const f64) })
    };
    let flag = || {
        (property.format == FORMAT_FLAG && !property.data.is_null())
            .then(|| unsafe { *(property.data as *const c_int) } != 0)
    };
    let string = || {
        if property.format != FORMAT_STRING || property.data.is_null() {
            return None;
        }
        let pointer = unsafe { *(property.data as *const *const c_char) };
        (!pointer.is_null())
            .then(|| unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned())
    };
    let mut shared = shared.lock();
    let state = &mut shared.state;
    match id {
        1 => state.position = double().unwrap_or(state.position).max(0.0),
        2 => state.duration = double().unwrap_or(0.0).max(0.0),
        3 => state.paused = flag().unwrap_or(false),
        4 => state.ended = flag().unwrap_or(false),
        5 => state.buffering = flag().unwrap_or(false),
        6 => {
            if let Some(volume) = double() {
                state.volume = mpv_volume_to_ui(volume);
            }
        }
        7 => state.muted = flag().unwrap_or(false),
        9 => state.audio_id = string().and_then(|value| value.parse().ok()),
        10 => state.subtitle_id = string().and_then(|value| value.parse().ok()),
        11 => state.video_ready = flag().unwrap_or(false),
        12 => state.speed = double().unwrap_or(1.0),
        13 => state.subtitle_scale = double().unwrap_or(1.0),
        14 => state.subtitle_delay = double().unwrap_or(0.0),
        15 => {
            state.hardware_decoder = string().filter(|value| !value.is_empty() && value != "no")
        }
        _ => {}
    }
}

fn update_tracks(core: &Core, shared: &Mutex<Shared>) {
    let tracks = core
        .get_string("track-list")
        .map(|json| parse_tracks(&json))
        .unwrap_or_default();
    shared.lock().state.tracks = tracks;
}

/// Convierte la lista de pistas que da mpv (JSON) en la lista que muestra la interfaz.
pub fn parse_tracks(json: &str) -> Vec<EmbeddedTrack> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let text = |key: &str| {
                item.get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            };
            let flag = |key: &str| item.get(key).and_then(Value::as_bool).unwrap_or(false);
            Some(EmbeddedTrack {
                id: item.get("id")?.as_i64()?,
                kind: text("type")?,
                language: text("lang"),
                title: text("title"),
                codec: text("codec"),
                channels: item.get("demux-channel-count").and_then(Value::as_i64),
                external: flag("external"),
                selected: flag("selected"),
                default_track: flag("default"),
                forced: flag("forced"),
                hearing_impaired: flag("hearing-impaired"),
            })
        })
        .collect()
}

fn track_ref(tracks: &[EmbeddedTrack], kind: &str, id: i64) -> Option<TrackRef> {
    tracks
        .iter()
        .find(|track| track.kind == kind && track.id == id)
        .map(|track| TrackRef {
            id,
            language: track.language.clone(),
            title: track.title.clone(),
        })
}

/// Busca la pista elegida antes: primero por idioma y nombre (no cambian aunque cambie el orden),
/// después por número.
fn find_remembered(tracks: &[EmbeddedTrack], kind: &str, wanted: &TrackRef) -> Option<i64> {
    let same_kind = || tracks.iter().filter(move |track| track.kind == kind);
    same_kind()
        .find(|track| track.language == wanted.language && track.title == wanted.title)
        .or_else(|| same_kind().find(|track| track.id == wanted.id))
        .map(|track| track.id)
}

fn restore_choice(core: &Core, shared: &Mutex<Shared>) {
    let (choice, tracks) = {
        let shared = shared.lock();
        let Some(choice) = shared
            .media_id
            .as_ref()
            .and_then(|media_id| shared.choices.get(media_id))
            .cloned()
        else {
            return;
        };
        (choice, shared.state.tracks.clone())
    };
    if let Some(id) = choice
        .audio
        .as_ref()
        .and_then(|wanted| find_remembered(&tracks, "audio", wanted))
    {
        let _ = core.set_property("aid", id);
    }
    if choice.subtitles_off {
        let _ = core.set_property("sid", "no");
    } else if let Some(id) = choice
        .subtitle
        .as_ref()
        .and_then(|wanted| find_remembered(&tracks, "sub", wanted))
    {
        let _ = core.set_property("sid", id);
    }
}

/// Color y fuente de los subtítulos de texto. Si el usuario eligió algo distinto de lo de fábrica,
/// se fuerza también sobre los subtítulos .ass, que traen su propio estilo.
fn apply_subtitle_style(core: &Core, style: &SubtitleStyle) {
    let color = if style.color == "amarillo" { "#FFE14D" } else { "#FFFFFF" };
    let font = subtitle_font(&style.font);
    let custom = style.color == "amarillo" || style.font != "clasica";
    let _ = core.set_property("sub-color", color);
    let _ = core.set_property("sub-font", font);
    let _ = core.set_property("sub-border-color", "#000000");
    let _ = core.set_property("sub-ass-override", if custom { "force" } else { "yes" });
}

fn subtitle_font(font: &str) -> &'static str {
    match font {
        "montserrat" => "Montserrat SemiBold",
        "segoe" => "Segoe UI Semibold",
        "verdana" => "Verdana",
        _ => "sans-serif",
    }
}

/// Lista de propiedades de mpv que corresponden a un ajuste de imagen, siempre en el mismo orden.
fn image_properties(image: &EmbeddedImage, defaults: &ScaleDefaults) -> Vec<(&'static str, String)> {
    let level = |value: f64| format_number(value.clamp(-50.0, 50.0).round());
    let (scale, cscale, dscale) = match image.scaling.as_str() {
        "alta" => ("ewa_lanczossharp", "ewa_lanczossharp", "mitchell"),
        "suave" => ("mitchell", "mitchell", "mitchell"),
        _ => (
            defaults.scale.as_str(),
            defaults.cscale.as_str(),
            defaults.dscale.as_str(),
        ),
    };
    vec![
        ("contrast", level(image.contrast)),
        ("saturation", level(image.saturation)),
        ("gamma", level(image.gamma)),
        ("hue", format_number((image.hue.clamp(-50.0, 50.0).round() * 0.4).round())),
        ("glsl-shader-opts", shader_options(image)),
        ("scale", scale.to_string()),
        ("cscale", cscale.to_string()),
        ("dscale", dscale.to_string()),
        ("scale-antiring", if image.antiring { "0.7" } else { "0" }.to_string()),
        ("deband", yes_no(image.deband).to_string()),
        ("deband-iterations", deband_iterations(image.deband_strength).to_string()),
        ("deinterlace", yes_no(image.deinterlace).to_string()),
        // La interpolación necesita que el video se sincronice con la pantalla.
        ("video-sync", if image.smooth_motion { "display-resample" } else { "audio" }.to_string()),
        ("interpolation", yes_no(image.smooth_motion).to_string()),
        ("panscan", if image.fill_screen { "1" } else { "0" }.to_string()),
        ("video-aspect-override", aspect_value(&image.aspect).to_string()),
        ("video-zoom", format_number(zoom_value(image.zoom))),
        ("video-rotate", rotate_value(image.rotate).to_string()),
        ("tone-mapping", tone_mapping_value(&image.tone_mapping).to_string()),
        ("hdr-compute-peak", if image.hdr_auto_peak { "auto" } else { "no" }.to_string()),
    ]
}

/// Manda a mpv solo lo que cambió respecto del ajuste anterior (o todo, si no hay anterior), así
/// mover un control no reinicia otros de rebote.
fn apply_image(
    core: &Core,
    image: &EmbeddedImage,
    previous: Option<&EmbeddedImage>,
    defaults: &ScaleDefaults,
) {
    let next = image_properties(image, defaults);
    let before = previous.map(|previous| image_properties(previous, defaults));
    for (index, (name, value)) in next.iter().enumerate() {
        let unchanged = before
            .as_ref()
            .and_then(|before| before.get(index))
            .is_some_and(|(_, old)| old == value);
        if !unchanged {
            let _ = core.set_property(name, value);
        }
    }
}

fn deband_iterations(strength: f64) -> i64 {
    1 + (strength.clamp(0.0, 100.0) / 100.0 * 3.0).round() as i64
}

fn aspect_value(aspect: &str) -> &'static str {
    match aspect {
        "16:9" => "16:9",
        "4:3" => "4:3",
        "2.35:1" => "2.35:1",
        "1.85:1" => "1.85:1",
        _ => "no",
    }
}

/// mpv mide el zoom en potencias de 2 (0 = tamaño normal, 1 = el doble).
fn zoom_value(percent: f64) -> f64 {
    (1.0 + percent.clamp(0.0, 100.0) / 100.0).log2()
}

fn rotate_value(degrees: i64) -> &'static str {
    match degrees.rem_euclid(360) {
        90 => "90",
        180 => "180",
        270 => "270",
        _ => "0",
    }
}

fn tone_mapping_value(mode: &str) -> &'static str {
    match mode {
        "bt.2390" => "bt.2390",
        "hable" => "hable",
        "mobius" => "mobius",
        "reinhard" => "reinhard",
        _ => "auto",
    }
}

/// Valores del filtro propio de CINE WANA (cinewana-imagen.glsl), de -1 a 1 (nitidez de 0 a 1).
pub fn shader_options(image: &EmbeddedImage) -> String {
    let unit = |value: f64| value.clamp(-50.0, 50.0) / 50.0;
    format!(
        "cw_brightness={:.4},cw_shadows={:.4},cw_highlights={:.4},cw_temperature={:.4},cw_sharpness={:.4}",
        unit(image.brightness),
        unit(image.shadows),
        unit(image.highlights),
        unit(image.temperature),
        image.sharpness.clamp(0.0, 100.0) / 100.0,
    )
}

/// mpv sube el volumen con una curva cúbica; así el control suena igual que el reproductor anterior.
pub fn ui_volume_to_mpv(volume: f64) -> f64 {
    let gain = volume.clamp(0.0, 1.0) / UI_NORMAL_VOLUME;
    (100.0 * gain.cbrt()).min(MPV_VOLUME_MAX)
}

pub fn mpv_volume_to_ui(volume: f64) -> f64 {
    let gain = (volume.max(0.0) / 100.0).powi(3);
    (gain * UI_NORMAL_VOLUME).clamp(0.0, 1.0)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn format_number(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" { "0".into() } else { text.to_string() }
}

/// mpv crea su ventana de video adentro de la de CINE WANA; se manda al fondo para que la
/// interfaz (botones, paneles) quede siempre encima del video.
fn keep_video_below_interface(window_handle: isize) {
    #[cfg(windows)]
    unsafe {
        win::EnumChildWindows(window_handle, Some(win::push_mpv_window_to_bottom), 0);
    }
    #[cfg(not(windows))]
    let _ = window_handle;
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        pub fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn EnumChildWindows(
            parent: isize,
            callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
            lparam: isize,
        ) -> i32;
        fn GetClassNameW(window: isize, buffer: *mut u16, max: i32) -> i32;
        fn SetWindowPos(
            window: isize,
            insert_after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    const HWND_BOTTOM: isize = 1;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;

    pub unsafe extern "system" fn push_mpv_window_to_bottom(window: isize, _lparam: isize) -> i32 {
        let mut buffer = [0u16; 64];
        let length = unsafe { GetClassNameW(window, buffer.as_mut_ptr(), buffer.len() as i32) };
        let class = String::from_utf16_lossy(&buffer[..length.max(0) as usize]);
        if class == "mpv" {
            unsafe {
                SetWindowPos(
                    window,
                    HWND_BOTTOM,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                );
            }
        }
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_keeps_the_previous_player_scale() {
        assert!((ui_volume_to_mpv(0.5) - 100.0).abs() < 1e-9);
        assert_eq!(ui_volume_to_mpv(0.0), 0.0);
        assert!(ui_volume_to_mpv(1.0) <= MPV_VOLUME_MAX);
        for volume in [0.1, 0.25, 0.5, 0.75, 1.0] {
            assert!((mpv_volume_to_ui(ui_volume_to_mpv(volume)) - volume).abs() < 1e-6);
        }
    }

    #[test]
    fn neutral_image_sends_neutral_shader_values() {
        assert_eq!(
            shader_options(&EmbeddedImage::default()),
            "cw_brightness=0.0000,cw_shadows=0.0000,cw_highlights=0.0000,cw_temperature=0.0000,cw_sharpness=0.0000"
        );
        let image = EmbeddedImage {
            brightness: 25.0,
            temperature: -80.0,
            sharpness: 40.0,
            ..EmbeddedImage::default()
        };
        let options = shader_options(&image);
        assert!(options.contains("cw_brightness=0.5000"));
        assert!(options.contains("cw_temperature=-1.0000"));
        assert!(options.contains("cw_sharpness=0.4000"));
    }

    #[test]
    fn tracks_are_read_from_mpv_json() {
        let json = r#"[
            {"id":1,"type":"video","codec":"h264","selected":true},
            {"id":1,"type":"audio","lang":"spa","title":"Español latino","codec":"ac3","demux-channel-count":6,"default":true,"selected":true},
            {"id":2,"type":"audio","lang":"eng","title":"English","codec":"ac3","demux-channel-count":2},
            {"id":1,"type":"sub","lang":"spa","codec":"subrip","external":true,"forced":true}
        ]"#;
        let tracks = parse_tracks(json);
        assert_eq!(tracks.len(), 4);
        assert_eq!(tracks[1].language.as_deref(), Some("spa"));
        assert_eq!(tracks[1].channels, Some(6));
        assert!(tracks[1].selected && tracks[1].default_track);
        assert!(tracks[3].external && tracks[3].forced);
        assert!(parse_tracks("no es json").is_empty());
    }

    #[test]
    fn remembered_track_is_found_by_language_before_number() {
        let tracks = parse_tracks(
            r#"[{"id":1,"type":"audio","lang":"eng","title":"English"},{"id":2,"type":"audio","lang":"spa","title":"Latino"}]"#,
        );
        let wanted = TrackRef {
            id: 1,
            language: Some("spa".into()),
            title: Some("Latino".into()),
        };
        assert_eq!(find_remembered(&tracks, "audio", &wanted), Some(2));
        let missing = TrackRef {
            id: 1,
            language: Some("por".into()),
            title: None,
        };
        assert_eq!(find_remembered(&tracks, "audio", &missing), Some(1));
    }

    #[test]
    fn framing_and_cleanup_values_match_mpv() {
        assert_eq!(deband_iterations(0.0), 1);
        assert_eq!(deband_iterations(100.0), 4);
        assert_eq!(aspect_value("4:3"), "4:3");
        assert_eq!(aspect_value("original"), "no");
        assert_eq!(aspect_value("cualquier cosa"), "no");
        assert_eq!(zoom_value(0.0), 0.0);
        assert!((zoom_value(100.0) - 1.0).abs() < 1e-9);
        assert_eq!(rotate_value(-90), "270");
        assert_eq!(rotate_value(45), "0");
        assert_eq!(tone_mapping_value("hable"), "hable");
        assert_eq!(tone_mapping_value(""), "auto");
    }

    #[test]
    fn subtitle_fonts_map_to_real_families() {
        assert_eq!(subtitle_font("montserrat"), "Montserrat SemiBold");
        assert_eq!(subtitle_font("clasica"), "sans-serif");
        assert_eq!(subtitle_font("otra"), "sans-serif");
        assert_eq!(SubtitleStyle::default().position, 100.0);
    }

    #[test]
    fn numbers_are_written_the_way_mpv_reads_them() {
        assert_eq!(format_number(12.0), "12");
        assert_eq!(format_number(12.5), "12.5");
        assert_eq!(format_number(-0.25), "-0.25");
        assert_eq!(format_number(0.0), "0");
    }
}
