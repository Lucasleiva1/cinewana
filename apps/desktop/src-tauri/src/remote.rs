use axum::{
    Json, Router,
    body::Body,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{Duration, Utc};
use cinewana_core::{
    CatalogQuery, MediaDetail, MediaKind, MediaSummary, MediaTrack, SeriesSeasonSummary,
    SeriesSummary,
};
use cinewana_database::Database;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use qrcode::{QrCode, render::svg};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, UdpSocket},
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};
use tauri::{AppHandle, Emitter};
use tokio::{
    net::TcpListener,
    sync::{broadcast, oneshot},
};
use uuid::Uuid;

const DEFAULT_PORT: u16 = 47_821;
const MAX_MESSAGE_BYTES: usize = 16 * 1024;
const PAIRING_MINUTES: i64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemotePlayerSnapshot {
    pub active: bool,
    pub media_id: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub quality: Option<String>,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub playing: bool,
    pub volume: f64,
    pub muted: bool,
    pub fullscreen: bool,
    pub image_analyzing: bool,
    pub image_analysis_percent: f64,
    pub next_up: Option<RemoteNextUp>,
    pub image_settings: Vec<RemoteImageSetting>,
    pub audio_tracks: Vec<RemoteTrack>,
    pub subtitle_tracks: Vec<RemoteTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteImageSetting {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub default_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteNextUp {
    pub id: String,
    pub title: String,
    pub label: String,
    pub position: Option<String>,
    pub seconds_remaining: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTrack {
    pub id: String,
    pub label: String,
    pub language: Option<String>,
    pub channels: Option<i32>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteCommand {
    PlayerToggle,
    PlayerSeekBy {
        seconds: f64,
    },
    PlayerSeekTo {
        seconds: f64,
    },
    PlayerSetVolume {
        volume: f64,
    },
    PlayerToggleMute,
    PlayerToggleFullscreen,
    PlayerStartNextUp,
    PlayerCancelNextUp,
    PlayerAnalyzeImage,
    PlayerSetImage {
        setting_id: String,
        value: f64,
    },
    PlayerResetImage,
    PlayerSetAudio {
        track_id: String,
    },
    PlayerSetSubtitle {
        track_id: Option<String>,
    },
    LibraryPlayMedia {
        media_id: String,
    },
    LibrarySetFlag {
        media_id: String,
        flag: String,
        value: bool,
    },
    LibraryRefresh,
    Navigate {
        direction: String,
    },
    NavigateBack,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeviceDto {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPairingDto {
    pub id: String,
    pub device_name: String,
    pub requested_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingDto {
    pub url: String,
    pub code: String,
    pub expires_at: String,
    pub qr_data_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatusDto {
    pub enabled: bool,
    pub computer_name: String,
    pub address: String,
    pub port: u16,
    pub url: Option<String>,
    pub secure_context: bool,
    pub pairing: Option<PairingDto>,
    pub devices: Vec<RemoteDeviceDto>,
    pub pending: Vec<PendingPairingDto>,
    pub last_connected_at: Option<String>,
    pub asset_root_ready: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDevice {
    id: String,
    name: String,
    token_hash: String,
    created_at: String,
    last_seen_at: Option<String>,
}

#[derive(Debug, Clone)]
struct PairingSession {
    token: String,
    code: String,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    id: String,
    pair_token: String,
    device_name: String,
    requested_at: String,
}

#[derive(Debug, Clone)]
struct ApprovedPairing {
    pair_token: String,
    device_id: String,
    device_token: String,
    expires_at: chrono::DateTime<Utc>,
}

struct RuntimeState {
    enabled: bool,
    address: String,
    port: u16,
    pairing: Option<PairingSession>,
    devices: Vec<StoredDevice>,
    pending: Vec<PendingPairing>,
    approved: Vec<ApprovedPairing>,
    last_connected_at: Option<String>,
    error: Option<String>,
    shutdown: Option<oneshot::Sender<()>>,
}

pub struct RemoteService {
    db: Arc<Database>,
    app: AppHandle,
    data_path: PathBuf,
    asset_root: PathBuf,
    runtime: Mutex<RuntimeState>,
    player: Mutex<RemotePlayerSnapshot>,
    player_tx: broadcast::Sender<RemotePlayerSnapshot>,
}

#[derive(Clone)]
struct HttpState {
    remote: Arc<RemoteService>,
}

impl RemoteService {
    pub fn new(
        db: Arc<Database>,
        app: AppHandle,
        data_dir: &FsPath,
        resource_dir: &FsPath,
    ) -> Arc<Self> {
        let bundled_assets = resource_dir.join("remote");
        let development_assets =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../remote/dist");
        let asset_root = std::env::var_os("CINE_WANA_REMOTE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if bundled_assets.join("index.html").is_file() {
                    bundled_assets
                } else {
                    development_assets
                }
            });
        let data_path = data_dir.join("remote-devices.json");
        let devices = std::fs::read(&data_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<StoredDevice>>(&bytes).ok())
            .unwrap_or_default();
        let (player_tx, _) = broadcast::channel(64);
        Arc::new(Self {
            db,
            app,
            data_path,
            asset_root,
            runtime: Mutex::new(RuntimeState {
                enabled: false,
                address: lan_ip().to_string(),
                port: configured_port(),
                pairing: None,
                devices,
                pending: Vec::new(),
                approved: Vec::new(),
                last_connected_at: None,
                error: None,
                shutdown: None,
            }),
            player: Mutex::new(RemotePlayerSnapshot::default()),
            player_tx,
        })
    }

    pub fn status(&self) -> RemoteStatusDto {
        self.cleanup_expired();
        let state = self.runtime.lock();
        let base_url = format!("http://{}:{}", state.address, state.port);
        RemoteStatusDto {
            enabled: state.enabled,
            computer_name: computer_name(),
            address: state.address.clone(),
            port: state.port,
            url: state.enabled.then_some(base_url.clone()),
            secure_context: false,
            pairing: state
                .pairing
                .as_ref()
                .map(|pairing| pairing_dto(pairing, &base_url)),
            devices: state.devices.iter().map(device_dto).collect(),
            pending: state
                .pending
                .iter()
                .map(|pending| PendingPairingDto {
                    id: pending.id.clone(),
                    device_name: pending.device_name.clone(),
                    requested_at: pending.requested_at.clone(),
                })
                .collect(),
            last_connected_at: state.last_connected_at.clone(),
            asset_root_ready: self.asset_root.join("index.html").is_file(),
            error: state.error.clone(),
        }
    }

    pub async fn start(self: &Arc<Self>) -> Result<RemoteStatusDto, String> {
        {
            let state = self.runtime.lock();
            if state.enabled {
                return Ok(self.status());
            }
            if !self.asset_root.join("index.html").is_file() {
                return Err("Primero compilá la interfaz remota con `pnpm --filter @cine-wana/remote build`.".into());
            }
        }
        let port = self.runtime.lock().port;
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
            .await
            .map_err(|error| format!("No se pudo abrir el puerto {port}: {error}"))?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        {
            let mut state = self.runtime.lock();
            state.enabled = true;
            state.address = lan_ip().to_string();
            state.error = None;
            state.shutdown = Some(shutdown_tx);
        }
        let http_state = HttpState {
            remote: self.clone(),
        };
        let router = Router::new()
            .route("/health", get(health))
            .route("/ws", get(websocket))
            .route("/api/media/{id}", get(media_detail))
            .route("/api/artwork/{id}", get(artwork))
            .route("/api/backdrop/{id}", get(backdrop))
            .route("/", get(static_index))
            .route("/{*path}", get(static_asset))
            .with_state(http_state);
        let service = self.clone();
        tokio::spawn(async move {
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
            let mut state = service.runtime.lock();
            state.enabled = false;
            state.shutdown = None;
            if let Err(error) = result {
                state.error = Some(error.to_string());
            }
            drop(state);
            service.emit_status();
        });
        self.emit_status();
        Ok(self.status())
    }

    pub fn stop(&self) -> RemoteStatusDto {
        let mut state = self.runtime.lock();
        if let Some(shutdown) = state.shutdown.take() {
            let _ = shutdown.send(());
        }
        state.enabled = false;
        state.pairing = None;
        state.pending.clear();
        drop(state);
        self.emit_status();
        self.status()
    }

    pub fn create_pairing(&self) -> Result<RemoteStatusDto, String> {
        if !self.runtime.lock().enabled {
            return Err("Activá primero el servidor remoto.".into());
        }
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let code = format!("{:06}", Uuid::new_v4().as_u128() % 1_000_000);
        let mut state = self.runtime.lock();
        state.pending.clear();
        state.approved.clear();
        state.pairing = Some(PairingSession {
            token,
            code,
            expires_at: Utc::now() + Duration::minutes(PAIRING_MINUTES),
        });
        drop(state);
        self.emit_status();
        Ok(self.status())
    }

    pub fn approve(&self, request_id: &str) -> Result<RemoteStatusDto, String> {
        let mut state = self.runtime.lock();
        let index = state
            .pending
            .iter()
            .position(|item| item.id == request_id)
            .ok_or_else(|| "La solicitud ya no está disponible.".to_string())?;
        let pending = state.pending.remove(index);
        let device_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let device = StoredDevice {
            id: Uuid::new_v4().to_string(),
            name: pending.device_name,
            token_hash: hash_token(&device_token),
            created_at: Utc::now().to_rfc3339(),
            last_seen_at: None,
        };
        state.approved.push(ApprovedPairing {
            pair_token: pending.pair_token,
            device_id: device.id.clone(),
            device_token,
            expires_at: Utc::now() + Duration::minutes(2),
        });
        state.devices.push(device);
        self.persist_devices(&state.devices)?;
        drop(state);
        self.emit_status();
        Ok(self.status())
    }

    pub fn reject(&self, request_id: &str) -> RemoteStatusDto {
        self.runtime
            .lock()
            .pending
            .retain(|item| item.id != request_id);
        self.emit_status();
        self.status()
    }

    pub fn revoke(&self, device_id: &str) -> Result<RemoteStatusDto, String> {
        let mut state = self.runtime.lock();
        state.devices.retain(|device| device.id != device_id);
        self.persist_devices(&state.devices)?;
        drop(state);
        self.emit_status();
        Ok(self.status())
    }

    pub fn update_player(&self, snapshot: RemotePlayerSnapshot) {
        *self.player.lock() = snapshot.clone();
        let _ = self.player_tx.send(snapshot);
    }

    fn cleanup_expired(&self) {
        let now = Utc::now();
        let mut state = self.runtime.lock();
        if state
            .pairing
            .as_ref()
            .is_some_and(|pairing| pairing.expires_at <= now)
        {
            state.pairing = None;
            state.pending.clear();
        }
        state.approved.retain(|approved| approved.expires_at > now);
    }

    fn emit_status(&self) {
        let _ = self.app.emit("remote-status-changed", self.status());
    }

    fn persist_devices(&self, devices: &[StoredDevice]) -> Result<(), String> {
        let parent = self
            .data_path
            .parent()
            .ok_or_else(|| "Ruta de datos inválida".to_string())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let bytes = serde_json::to_vec_pretty(devices).map_err(|error| error.to_string())?;
        std::fs::write(&self.data_path, bytes).map_err(|error| error.to_string())
    }

    fn verify_token(&self, token: &str) -> bool {
        let candidate = hash_token(token);
        self.runtime
            .lock()
            .devices
            .iter()
            .any(|device| device.token_hash == candidate)
    }

    fn authenticate(&self, device_id: &str, token: &str) -> bool {
        let candidate = hash_token(token);
        let now = Utc::now().to_rfc3339();
        let mut state = self.runtime.lock();
        let Some(device) = state
            .devices
            .iter_mut()
            .find(|device| device.id == device_id && device.token_hash == candidate)
        else {
            return false;
        };
        device.last_seen_at = Some(now.clone());
        state.last_connected_at = Some(now);
        let devices = state.devices.clone();
        drop(state);
        let _ = self.persist_devices(&devices);
        self.emit_status();
        true
    }

    fn try_pair(&self, pair_token: &str, device_name: &str) -> PairResult {
        self.cleanup_expired();
        let mut state = self.runtime.lock();
        if let Some(index) = state
            .approved
            .iter()
            .position(|approved| approved.pair_token == pair_token)
        {
            let approved = state.approved.remove(index);
            state.pairing = None;
            return PairResult::Authorized {
                device_id: approved.device_id,
                device_token: approved.device_token,
            };
        }
        let valid = state
            .pairing
            .as_ref()
            .is_some_and(|pairing| pairing.token == pair_token && pairing.expires_at > Utc::now());
        if !valid {
            return PairResult::Invalid;
        }
        if let Some(pending) = state
            .pending
            .iter()
            .find(|pending| pending.pair_token == pair_token)
        {
            return PairResult::Pending {
                request_id: pending.id.clone(),
            };
        }
        let pending = PendingPairing {
            id: Uuid::new_v4().to_string(),
            pair_token: pair_token.to_string(),
            device_name: clean_device_name(device_name),
            requested_at: Utc::now().to_rfc3339(),
        };
        let request_id = pending.id.clone();
        state.pending.push(pending);
        drop(state);
        self.emit_status();
        PairResult::Pending { request_id }
    }

    fn library_snapshot(&self) -> Result<Value, String> {
        let account_id = self
            .db
            .require_active_account_id()
            .map_err(|error| error.to_string())?;
        let catalog = self
            .db
            .catalog(
                Some(&account_id),
                &CatalogQuery {
                    limit: Some(1000),
                    ..Default::default()
                },
            )
            .map_err(|error| error.to_string())?;
        let home = self
            .db
            .home(Some(&account_id))
            .map_err(|error| error.to_string())?;
        let items = catalog.iter().map(remote_media).collect::<Vec<_>>();
        let movies = home.movies.iter().map(remote_media).collect::<Vec<_>>();
        let recently_added = home
            .recently_added
            .iter()
            .map(remote_media)
            .collect::<Vec<_>>();
        let continue_watching = home
            .continue_watching
            .iter()
            .map(remote_media)
            .collect::<Vec<_>>();
        let series = home.series.iter().map(remote_series).collect::<Vec<_>>();
        Ok(json!({
            "type": "library",
            "items": items,
            "movies": movies,
            "recentlyAdded": recently_added,
            "continueWatching": continue_watching,
            "series": series,
        }))
    }

    fn detail(&self, id: &str) -> Result<Option<RemoteMediaDetail>, String> {
        validate_id(id)?;
        let account_id = self
            .db
            .require_active_account_id()
            .map_err(|error| error.to_string())?;
        self.db
            .media_detail(Some(&account_id), id)
            .map_err(|error| error.to_string())
            .map(|detail| detail.map(remote_detail))
    }

    fn set_flag(&self, media_id: &str, flag: &str, value: bool) -> Result<(), String> {
        validate_id(media_id)?;
        if flag != "favorite" && flag != "watchlist" {
            return Err("Bandera no permitida".into());
        }
        let account_id = self
            .db
            .require_active_account_id()
            .map_err(|error| error.to_string())?;
        self.db
            .set_flag(&account_id, media_id, flag, value)
            .map_err(|error| error.to_string())
    }
}

impl Drop for RemoteService {
    fn drop(&mut self) {
        if let Some(shutdown) = self.runtime.get_mut().shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

enum PairResult {
    Authorized {
        device_id: String,
        device_token: String,
    },
    Pending {
        request_id: String,
    },
    Invalid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteMedia {
    id: String,
    kind: MediaKind,
    title: String,
    year: Option<i32>,
    series_title: Option<String>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    progress_percent: f64,
    favorite: bool,
    in_watchlist: bool,
    completed: bool,
    artwork_available: bool,
    backdrop_available: bool,
    overview: Option<String>,
    duration_ms: Option<i64>,
    quality: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSeries {
    episode_id: String,
    title: String,
    seasons: u32,
    episodes: u32,
    artwork_available: bool,
    season_items: Vec<RemoteSeason>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSeason {
    season_number: i32,
    title: String,
    episodes: Vec<RemoteMedia>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteMediaDetail {
    #[serde(flatten)]
    summary: RemoteMedia,
    genres: Vec<String>,
    runtime_ms: Option<i64>,
    tracks: Vec<RemoteTrack>,
}

async fn health(State(state): State<HttpState>) -> Json<Value> {
    Json(
        json!({ "product": "CINE WANA", "ready": true, "pairedDevices": state.remote.runtime.lock().devices.len() }),
    )
}

async fn websocket(
    ws: WebSocketUpgrade,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Response {
    if !origin_allowed(&state.remote, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, state.remote))
        .into_response()
}

async fn handle_socket(mut socket: WebSocket, remote: Arc<RemoteService>) {
    let Some(Ok(Message::Text(first))) = socket.next().await else {
        return;
    };
    if first.len() > MAX_MESSAGE_BYTES {
        let _ = socket.close().await;
        return;
    }
    let Ok(hello) = serde_json::from_str::<Value>(&first) else {
        let _ = send_json(
            &mut socket,
            json!({"type":"error","message":"Mensaje inválido"}),
        )
        .await;
        return;
    };
    let session_token = match hello.get("type").and_then(Value::as_str) {
        Some("auth") => {
            let device_id = hello
                .get("deviceId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let token = hello
                .get("token")
                .and_then(Value::as_str)
                .unwrap_or_default();
            remote
                .authenticate(device_id, token)
                .then(|| token.to_string())
        }
        Some("pair") => {
            let token = hello
                .get("pairToken")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let name = hello
                .get("deviceName")
                .and_then(Value::as_str)
                .unwrap_or("Teléfono");
            match remote.try_pair(token, name) {
                PairResult::Authorized {
                    device_id,
                    device_token,
                } => {
                    let _ = send_json(
                        &mut socket,
                        json!({"type":"paired","deviceId":device_id,"token":device_token}),
                    )
                    .await;
                    Some(device_token)
                }
                PairResult::Pending { request_id } => {
                    let _ = send_json(
                        &mut socket,
                        json!({"type":"pair_pending","requestId":request_id}),
                    )
                    .await;
                    None
                }
                PairResult::Invalid => {
                    let _ = send_json(&mut socket, json!({"type":"pair_invalid"})).await;
                    None
                }
            }
        }
        _ => None,
    };
    let Some(session_token) = session_token else {
        let _ = socket.close().await;
        return;
    };
    let _ = send_json(&mut socket, json!({"type":"authenticated"})).await;
    let _ = send_json(
        &mut socket,
        json!({"type":"player","player":remote.player.lock().clone()}),
    )
    .await;
    if let Ok(library) = remote.library_snapshot() {
        let _ = send_json(&mut socket, library).await;
    }
    let mut player_rx = remote.player_tx.subscribe();
    let mut rate = VecDeque::<Instant>::new();
    loop {
        tokio::select! {
            incoming = socket.next() => {
                let Some(Ok(message)) = incoming else { break; };
                let Message::Text(text) = message else { continue; };
                if text.len() > MAX_MESSAGE_BYTES { break; }
                if !remote.verify_token(&session_token) { let _ = send_json(&mut socket, json!({"type":"session_revoked"})).await; break; }
                let now = Instant::now();
                while rate.front().is_some_and(|stamp| now.duration_since(*stamp) > StdDuration::from_secs(1)) { rate.pop_front(); }
                if rate.len() >= 30 { let _ = send_json(&mut socket, json!({"type":"error","message":"Demasiados comandos"})).await; continue; }
                rate.push_back(now);
                let Ok(command) = serde_json::from_str::<RemoteCommand>(&text) else { let _ = send_json(&mut socket, json!({"type":"error","message":"Comando no permitido"})).await; continue; };
                match validate_command(&command) {
                    Err(error) => { let _ = send_json(&mut socket, json!({"type":"error","message":error})).await; }
                    Ok(()) => match &command {
                        RemoteCommand::LibrarySetFlag { media_id, flag, value } => {
                            match remote.set_flag(media_id, flag, *value) {
                                Ok(()) => if let Ok(library) = remote.library_snapshot() { let _ = send_json(&mut socket, library).await; },
                                Err(error) => { let _ = send_json(&mut socket, json!({"type":"error","message":error})).await; }
                            }
                        }
                        RemoteCommand::LibraryRefresh => {
                            if let Ok(library) = remote.library_snapshot() { let _ = send_json(&mut socket, library).await; }
                        }
                        _ => { let _ = remote.app.emit("remote-command", command); }
                    }
                }
            }
            snapshot = player_rx.recv() => {
                if let Ok(player) = snapshot {
                    if send_json(&mut socket, json!({"type":"player","player":player})).await.is_err() { break; }
                }
            }
        }
    }
}

async fn media_detail(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&state.remote, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.remote.detail(&id) {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
    }
}

async fn artwork(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&state.remote, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(detail)) = state.remote.detail(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let account_id = match state.remote.db.require_active_account_id() {
        Ok(id) => id,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let Ok(Some(source)) = state
        .remote
        .db
        .media_detail(Some(&account_id), &detail.summary.id)
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(path) = source.summary.artwork_url else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = mime_for(FsPath::new(&path));
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn backdrop(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&state.remote, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(detail)) = state.remote.detail(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let account_id = match state.remote.db.require_active_account_id() {
        Ok(id) => id,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let Ok(Some(source)) = state
        .remote
        .db
        .media_detail(Some(&account_id), &detail.summary.id)
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(path) = source.summary.backdrop_url else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = mime_for(FsPath::new(&path));
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn static_index(State(state): State<HttpState>) -> Response {
    serve_file(&state.remote.asset_root, "index.html").await
}

async fn static_asset(Path(path): Path<String>, State(state): State<HttpState>) -> Response {
    if path.starts_with("api/") || path == "ws" || path.contains("..") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let response = serve_file(&state.remote.asset_root, &path).await;
    if response.status() == StatusCode::NOT_FOUND {
        serve_file(&state.remote.asset_root, "index.html").await
    } else {
        response
    }
}

async fn serve_file(root: &FsPath, relative: &str) -> Response {
    let path = root.join(relative.trim_start_matches('/'));
    if !path.starts_with(root) || !path.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(&path).await {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, mime_for(&path))
            .header(
                header::CACHE_CONTROL,
                if relative == "index.html" {
                    "no-cache"
                } else {
                    "public, max-age=86400"
                },
            )
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn send_json(socket: &mut WebSocket, value: Value) -> Result<(), axum::Error> {
    socket.send(Message::Text(value.to_string().into())).await
}

fn authorized(remote: &RemoteService, headers: &HeaderMap) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| remote.verify_token(token))
}

fn origin_allowed(remote: &RemoteService, headers: &HeaderMap) -> bool {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let state = remote.runtime.lock();
    origin == format!("http://{}:{}", state.address, state.port)
        || origin == format!("http://127.0.0.1:{}", state.port)
        || origin == format!("http://localhost:{}", state.port)
}

fn validate_command(command: &RemoteCommand) -> Result<(), String> {
    match command {
        RemoteCommand::PlayerSeekBy { seconds }
            if !seconds.is_finite() || seconds.abs() > 600.0 =>
        {
            Err("Salto inválido".into())
        }
        RemoteCommand::PlayerSeekTo { seconds } if !seconds.is_finite() || *seconds < 0.0 => {
            Err("Posición inválida".into())
        }
        RemoteCommand::PlayerSetVolume { volume }
            if !volume.is_finite() || !(0.0..=1.0).contains(volume) =>
        {
            Err("Volumen inválido".into())
        }
        RemoteCommand::PlayerSetImage { setting_id, value }
            if !matches!(
                setting_id.as_str(),
                "brightness" | "contrast" | "saturation" | "shadows" | "highlights" | "temperature"
            ) || !value.is_finite()
                || !(-100.0..=100.0).contains(value) =>
        {
            Err("Ajuste de imagen inválido".into())
        }
        RemoteCommand::LibraryPlayMedia { media_id } => validate_id(media_id),
        RemoteCommand::Navigate { direction }
            if !matches!(
                direction.as_str(),
                "up" | "down" | "left" | "right" | "confirm"
            ) =>
        {
            Err("Dirección inválida".into())
        }
        _ => Ok(()),
    }
}

fn validate_id(value: &str) -> Result<(), String> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| "Identificador inválido".into())
}
fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn clean_device_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|char| !char.is_control())
        .take(48)
        .collect::<String>()
        .trim()
        .to_string()
        .chars()
        .collect::<String>()
        .pipe(|name| {
            if name.is_empty() {
                "Teléfono".into()
            } else {
                name
            }
        })
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

fn configured_port() -> u16 {
    std::env::var("REMOTE_CONTROL_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|port| *port >= 1024)
        .unwrap_or(DEFAULT_PORT)
}
fn lan_ip() -> IpAddr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(192, 0, 2, 1), 80))?;
            socket.local_addr().map(|address| address.ip())
        })
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}
fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "CINE WANA".into())
}
fn device_dto(device: &StoredDevice) -> RemoteDeviceDto {
    RemoteDeviceDto {
        id: device.id.clone(),
        name: device.name.clone(),
        created_at: device.created_at.clone(),
        last_seen_at: device.last_seen_at.clone(),
    }
}
fn pairing_dto(pairing: &PairingSession, base_url: &str) -> PairingDto {
    let url = format!("{base_url}/?pair={}", pairing.token);
    let svg = QrCode::new(url.as_bytes())
        .map(|code| {
            code.render::<svg::Color>()
                .min_dimensions(320, 320)
                .dark_color(svg::Color("#111111"))
                .light_color(svg::Color("#ffffff"))
                .build()
        })
        .unwrap_or_default();
    PairingDto {
        url,
        code: pairing.code.clone(),
        expires_at: pairing.expires_at.to_rfc3339(),
        qr_data_url: format!(
            "data:image/svg+xml;base64,{}",
            STANDARD.encode(svg.as_bytes())
        ),
    }
}

fn remote_media(item: &MediaSummary) -> RemoteMedia {
    RemoteMedia {
        id: item.id.clone(),
        kind: item.kind.clone(),
        title: item.title.clone(),
        year: item.year,
        series_title: item.series_title.clone(),
        season_number: item.season_number,
        episode_number: item.episode_number,
        progress_percent: item.progress_percent,
        favorite: item.favorite,
        in_watchlist: item.in_watchlist,
        completed: item.completed,
        artwork_available: item.artwork_url.is_some(),
        backdrop_available: item.backdrop_url.is_some(),
        overview: item.overview.clone(),
        duration_ms: item.technical.duration_ms,
        quality: item.technical.height.map(|height| {
            if height >= 2160 {
                "4K".into()
            } else if height >= 1080 {
                "1080p".into()
            } else if height >= 720 {
                "720p".into()
            } else {
                format!("{height}p")
            }
        }),
    }
}

fn remote_series(series: &SeriesSummary) -> RemoteSeries {
    RemoteSeries {
        episode_id: series.episode_id.clone(),
        title: series.title.clone(),
        seasons: series.seasons,
        episodes: series.episodes,
        artwork_available: series.artwork_url.is_some(),
        season_items: series.season_items.iter().map(remote_season).collect(),
    }
}

fn remote_season(season: &SeriesSeasonSummary) -> RemoteSeason {
    RemoteSeason {
        season_number: season.season_number,
        title: season.title.clone(),
        episodes: season.episodes.iter().map(remote_media).collect(),
    }
}

fn remote_detail(detail: MediaDetail) -> RemoteMediaDetail {
    let tracks = detail.tracks.iter().map(remote_track).collect();
    RemoteMediaDetail {
        summary: remote_media(&detail.summary),
        genres: detail.genres,
        runtime_ms: detail.runtime_ms,
        tracks,
    }
}

fn remote_track(track: &MediaTrack) -> RemoteTrack {
    RemoteTrack {
        id: track.id.clone(),
        label: track
            .title
            .clone()
            .or_else(|| track.language.clone())
            .unwrap_or_else(|| track.codec.clone().unwrap_or_else(|| "Pista".into())),
        language: track.language.clone(),
        channels: track.channels,
        active: track.default_track,
    }
}

fn mime_for(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}
