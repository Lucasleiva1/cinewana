use cinewana_core::{
    AccountDto, BootstrapDto, CatalogQuery, DEFAULT_LIBRARY_ROOT, MediaDetail,
    MediaMetadataCandidate, MediaMetadataUpdate, MediaSummary, PlayerCommand, PlayerState,
    ScanProgress,
};
use cinewana_database::{Database, MetadataImportTarget};
use cinewana_metadata::{MetadataSearchOutcome, WikipediaMetadataClient};
use cinewana_player::PlayerService;
use parking_lot::Mutex;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[derive(Clone)]
struct AppServices {
    db: Arc<Database>,
    progress: Arc<Mutex<ScanProgress>>,
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    cache_dir: PathBuf,
    metadata: Arc<WikipediaMetadataClient>,
    player: Arc<PlayerService>,
}

#[tauri::command]
fn bootstrap(state: State<'_, AppServices>) -> Result<BootstrapDto, String> {
    let active_account = state.db.active_account().map_err(error_string)?;
    let home = if let Some(account) = active_account.as_ref() {
        state
            .db
            .home(Some(account.id.as_str()))
            .map_err(error_string)?
    } else {
        Default::default()
    };
    Ok(BootstrapDto {
        roots: state.db.roots(true).map_err(error_string)?,
        scan: state.progress.lock().clone(),
        home,
        accounts: state.db.accounts().map_err(error_string)?,
        active_account,
        ffprobe_available: state.ffprobe.is_some(),
        player_available: state.player.available(),
    })
}

#[tauri::command]
fn create_account(
    name: String,
    password: String,
    state: State<'_, AppServices>,
) -> Result<AccountDto, String> {
    state
        .db
        .create_account(&name, &password)
        .map_err(error_string)
}

#[tauri::command]
fn login_account(
    name: String,
    password: String,
    state: State<'_, AppServices>,
) -> Result<AccountDto, String> {
    state
        .db
        .login_account(&name, &password)
        .map_err(error_string)
}

#[tauri::command]
fn logout_account(state: State<'_, AppServices>) -> Result<(), String> {
    state.db.logout_account().map_err(error_string)
}

#[tauri::command]
fn catalog(
    query: Option<CatalogQuery>,
    state: State<'_, AppServices>,
) -> Result<Vec<MediaSummary>, String> {
    let account_id = state.db.require_active_account_id().map_err(error_string)?;
    state
        .db
        .catalog(Some(&account_id), &query.unwrap_or_default())
        .map_err(error_string)
}

#[tauri::command]
fn media_detail(id: String, state: State<'_, AppServices>) -> Result<Option<MediaDetail>, String> {
    let account_id = state.db.require_active_account_id().map_err(error_string)?;
    state
        .db
        .media_detail(Some(&account_id), &id)
        .map_err(error_string)
}

#[tauri::command]
fn update_media_metadata(
    media_id: String,
    metadata: MediaMetadataUpdate,
    state: State<'_, AppServices>,
) -> Result<(), String> {
    state
        .db
        .update_media_metadata(&media_id, &metadata)
        .map_err(error_string)
}

#[tauri::command]
async fn refresh_media_metadata(
    media_id: String,
    state: State<'_, AppServices>,
) -> Result<(), String> {
    let target = state
        .db
        .metadata_target(&media_id)
        .map_err(error_string)?
        .ok_or_else(|| "No se encontró el archivo para buscar información".to_string())?;
    import_metadata_for_target(state.inner(), &target)
        .await
        .map_err(error_string)
}

#[tauri::command]
async fn apply_metadata_candidate(
    media_id: String,
    candidate: MediaMetadataCandidate,
    state: State<'_, AppServices>,
) -> Result<(), String> {
    let target = state
        .db
        .metadata_target(&media_id)
        .map_err(error_string)?
        .ok_or_else(|| "No se encontró el archivo para guardar información".to_string())?;
    let metadata = state
        .metadata
        .import_candidate(&candidate)
        .await
        .map_err(error_string)?
        .ok_or_else(|| "No se pudo leer esa página de Wikipedia".to_string())?;
    let json_path =
        cinewana_metadata::write_metadata_json(&state.cache_dir, &target.fingerprint, &metadata)
            .map_err(error_string)?;
    state
        .db
        .apply_imported_metadata(&media_id, &metadata, Some(&json_path.to_string_lossy()))
        .map_err(error_string)
}

#[tauri::command]
fn set_media_flag(
    media_id: String,
    flag: String,
    value: bool,
    state: State<'_, AppServices>,
) -> Result<(), String> {
    let account_id = state.db.require_active_account_id().map_err(error_string)?;
    state
        .db
        .set_flag(&account_id, &media_id, &flag, value)
        .map_err(error_string)
}

#[tauri::command]
fn save_progress(
    media_id: String,
    position_ms: i64,
    duration_ms: i64,
    state: State<'_, AppServices>,
) -> Result<(), String> {
    let account_id = state.db.require_active_account_id().map_err(error_string)?;
    state
        .db
        .save_progress(&account_id, &media_id, position_ms, duration_ms)
        .map_err(error_string)
}

#[tauri::command]
fn scan_status(state: State<'_, AppServices>) -> ScanProgress {
    state.progress.lock().clone()
}

#[tauri::command]
fn start_scan(
    app: AppHandle,
    reason: Option<String>,
    state: State<'_, AppServices>,
) -> ScanProgress {
    let services = state.inner().clone();
    spawn_scan(app, services, reason.unwrap_or_else(|| "manual".into()));
    state.progress.lock().clone()
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppServices>) -> ScanProgress {
    state.cancel.store(true, Ordering::SeqCst);
    let mut progress = state.progress.lock();
    progress.cancel_requested = true;
    progress.message = Some("Cancelando después del archivo actual…".into());
    progress.clone()
}

#[tauri::command]
fn replace_library_root(
    path: String,
    app: AppHandle,
    state: State<'_, AppServices>,
) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return Err("La carpeta seleccionada no existe o no está disponible".into());
    }
    let id = state
        .db
        .replace_root(&path.to_string_lossy())
        .map_err(error_string)?;
    spawn_scan(app, state.inner().clone(), "root_changed".into());
    Ok(id)
}

#[tauri::command]
fn technical_path(
    media_id: String,
    state: State<'_, AppServices>,
) -> Result<Option<String>, String> {
    state.db.media_path(&media_id).map_err(error_string)
}

#[tauri::command]
fn player_state(state: State<'_, AppServices>) -> PlayerState {
    state.player.state()
}

#[tauri::command]
fn player_command(
    command: PlayerCommand,
    app: AppHandle,
    state: State<'_, AppServices>,
) -> Result<PlayerState, String> {
    let is_play = matches!(&command, PlayerCommand::Play { .. });
    let is_stop = matches!(&command, PlayerCommand::Stop);
    let media_id = match &command {
        PlayerCommand::Play { media_id } => media_id.clone(),
        _ => None,
    };
    let (title, path) = if let Some(id) = media_id {
        let account_id = state.db.require_active_account_id().map_err(error_string)?;
        let detail = state
            .db
            .media_detail(Some(&account_id), &id)
            .map_err(error_string)?;
        let title = detail.as_ref().map(|d| d.summary.title.clone());
        let path = state
            .db
            .media_path(&id)
            .map_err(error_string)?
            .map(PathBuf::from);
        (title, path)
    } else {
        (None, None)
    };
    let parent_hwnd = if is_play && std::env::var_os("CINE_WANA_EMBED_MPV").is_some() {
        let window = if let Some(window) = app.get_window("player") {
            window
        } else {
            tauri::WindowBuilder::new(&app, "player")
                .title("CINE WANA — Reproductor")
                .decorations(false)
                .fullscreen(true)
                .build()
                .map_err(error_string)?
        };
        window.show().map_err(error_string)?;
        window.set_focus().map_err(error_string)?;
        Some(window.hwnd().map_err(error_string)?.0 as isize)
    } else {
        None
    };
    let result = state
        .player
        .execute(command, title, path, parent_hwnd)
        .map_err(error_string)?;
    if is_stop {
        if let Some(window) = app.get_window("player") {
            let _ = window.close();
        }
    } else if is_play {
        let handle = app.clone();
        let player = state.player.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                if !player.is_running() {
                    if let Some(window) = handle.get_window("player") {
                        let _ = window.close();
                    }
                    break;
                }
            }
        });
    }
    Ok(result)
}

fn spawn_scan(app: AppHandle, services: AppServices, reason: String) {
    if services
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    services.cancel.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_scan(&app, &services, &reason).await {
            let mut progress = services.progress.lock();
            progress.running = false;
            progress.errors += 1;
            progress.message = Some(format!("No se pudo completar el escaneo: {error}"));
            let _ = app.emit("scan-progress", progress.clone());
        }
        services.running.store(false, Ordering::SeqCst);
    });
}

async fn import_metadata_for_target(
    services: &AppServices,
    target: &MetadataImportTarget,
) -> anyhow::Result<()> {
    let outcome = services
        .metadata
        .search_movie(&target.title, target.year)
        .await?;
    match outcome {
        MetadataSearchOutcome::Imported(metadata) => {
            let json_path = cinewana_metadata::write_metadata_json(
                &services.cache_dir,
                &target.fingerprint,
                &metadata,
            )?;
            services.db.apply_imported_metadata(
                &target.media_id,
                &metadata,
                Some(&json_path.to_string_lossy()),
            )?;
        }
        MetadataSearchOutcome::Ambiguous(candidates) => {
            services
                .db
                .store_metadata_candidates(&target.media_id, &candidates)?;
        }
        MetadataSearchOutcome::NotFound => {
            services
                .db
                .store_metadata_candidates(&target.media_id, &[])?;
        }
    }
    Ok(())
}

async fn run_scan(app: &AppHandle, services: &AppServices, reason: &str) -> anyhow::Result<()> {
    let roots = services.db.enabled_roots_with_paths()?;
    let job_id = Uuid::new_v4().to_string();
    {
        let mut p = services.progress.lock();
        *p = ScanProgress {
            job_id: Some(job_id.clone()),
            running: true,
            message: Some("Buscando películas y series…".into()),
            ..ScanProgress::default()
        };
        app.emit("scan-progress", p.clone())?;
    }
    let mut grand_found = 0u64;
    let mut grand_processed = 0u64;
    let mut grand_skipped = 0u64;
    let mut grand_errors = 0u64;
    let root_count = roots.len();
    for (root_index, (root_id, root_path)) in roots.into_iter().enumerate() {
        let root = PathBuf::from(&root_path);
        if !root.is_dir() {
            services.db.set_root_status(&root_id, "disconnected")?;
            continue;
        }
        let scan_id = if root_count == 1 {
            job_id.clone()
        } else {
            Uuid::new_v4().to_string()
        };
        services.db.start_scan(&root_id, &scan_id, reason)?;
        let discover_root = root.clone();
        let files = tauri::async_runtime::spawn_blocking(move || {
            cinewana_scanner::discover(&discover_root, true)
        })
        .await??;
        grand_found += files.len() as u64;
        {
            let mut p = services.progress.lock();
            p.found = grand_found;
            p.message = Some(format!("Biblioteca {} de {}", root_index + 1, root_count));
            app.emit("scan-progress", p.clone())?;
        }
        let mut local_processed = 0u64;
        let mut local_skipped = 0u64;
        let mut local_errors = 0u64;
        for path in files {
            if services.cancel.load(Ordering::SeqCst) {
                break;
            }
            let display = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Archivo")
                .to_owned();
            {
                let mut p = services.progress.lock();
                p.current_file = Some(display);
                p.processed = grand_processed + local_processed;
                p.skipped = grand_skipped + local_skipped;
                p.errors = grand_errors + local_errors;
                p.percent = if grand_found == 0 {
                    0.0
                } else {
                    ((p.processed + p.skipped + p.errors) as f64 / grand_found as f64 * 100.0)
                        .min(100.0)
                };
                app.emit("scan-progress", p.clone())?;
            }
            match cinewana_scanner::inspect(&path, services.ffprobe.as_deref()).await {
                Ok(file) => match services.db.upsert_file(&root_id, &scan_id, &file) {
                    Ok(outcome) => {
                        if outcome.skipped {
                            local_skipped += 1;
                        } else {
                            local_processed += 1;
                        }
                        if let Some(ffmpeg) = services.ffmpeg.as_deref() {
                            {
                                let mut p = services.progress.lock();
                                p.message = Some("Generando portada y vista previa…".into());
                                let _ = app.emit("scan-progress", p.clone());
                            }
                            match cinewana_scanner::generate_artwork(
                                ffmpeg,
                                &path,
                                &services.cache_dir,
                                &file.fingerprint,
                                file.technical.duration_ms,
                            )
                            .await
                            {
                                Ok((poster, backdrop, preview)) => {
                                    if services
                                        .db
                                        .set_artwork(
                                            &outcome.media_id,
                                            &poster.to_string_lossy(),
                                            &backdrop.to_string_lossy(),
                                            &preview.to_string_lossy(),
                                        )
                                        .is_err()
                                    {
                                        local_errors += 1;
                                    }
                                }
                                Err(_) => local_errors += 1,
                            }
                        }
                        if services
                            .db
                            .should_auto_import_metadata(&outcome.media_id)
                            .unwrap_or(false)
                        {
                            if let Ok(Some(target)) = services.db.metadata_target(&outcome.media_id)
                            {
                                {
                                    let mut p = services.progress.lock();
                                    p.message =
                                        Some(format!("Buscando información: {}", target.file_name));
                                    let _ = app.emit("scan-progress", p.clone());
                                }
                                let _ = import_metadata_for_target(services, &target).await;
                                tokio::time::sleep(std::time::Duration::from_millis(350)).await;
                            }
                        }
                    }
                    Err(_) => local_errors += 1,
                },
                Err(_) => local_errors += 1,
            }
        }
        let cancelled = services.cancel.load(Ordering::SeqCst);
        services.db.finish_scan(
            &root_id,
            &scan_id,
            if cancelled { "cancelled" } else { "completed" },
            files_len(grand_found, root_count),
            local_processed,
            local_skipped,
            local_errors,
        )?;
        grand_processed += local_processed;
        grand_skipped += local_skipped;
        grand_errors += local_errors;
        if cancelled {
            break;
        }
    }
    let cancelled = services.cancel.load(Ordering::SeqCst);
    {
        let mut p = services.progress.lock();
        p.running = false;
        p.cancel_requested = false;
        p.processed = grand_processed;
        p.skipped = grand_skipped;
        p.errors = grand_errors;
        p.percent = if cancelled { p.percent } else { 100.0 };
        p.current_file = None;
        p.message = Some(if cancelled {
            "Escaneo cancelado".into()
        } else {
            format!(
                "Biblioteca actualizada: {} procesados, {} sin cambios",
                grand_processed, grand_skipped
            )
        });
        app.emit("scan-progress", p.clone())?;
    }
    app.emit("library-changed", ())?;
    Ok(())
}

fn files_len(found: u64, _roots: usize) -> u64 {
    found
}

fn find_command(name: &str) -> Option<PathBuf> {
    let executable = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let local = PathBuf::from(".tools")
        .join("ffmpeg")
        .join("bin")
        .join(&executable);
    if local.is_file() {
        return Some(local);
    }
    if let Some(found) = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(&executable))
            .find(|p| p.is_file())
    }) {
        return Some(found);
    }
    if cfg!(windows) && matches!(name, "ffmpeg" | "ffprobe") {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let packages = PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            return find_file_recursive(&packages, &executable, 5);
        }
    }
    None
}

fn find_file_recursive(root: &std::path::Path, name: &str, depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    for entry in std::fs::read_dir(root).ok()?.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(name))
                .unwrap_or(false)
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let db = Arc::new(Database::open(app_data.join("cine-wana.db"))?);
            db.seed_root(DEFAULT_LIBRARY_ROOT)?;
            let ffmpeg = find_command("ffmpeg");
            let services = AppServices {
                db,
                progress: Arc::new(Mutex::new(ScanProgress::default())),
                running: Arc::new(AtomicBool::new(false)),
                cancel: Arc::new(AtomicBool::new(false)),
                ffmpeg,
                ffprobe: find_command("ffprobe"),
                cache_dir: app_data.join("cache"),
                metadata: Arc::new(WikipediaMetadataClient::new()?),
                player: Arc::new(PlayerService::discover()),
            };
            app.manage(services.clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(450)).await;
                spawn_scan(handle, services, "startup".into());
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            create_account,
            login_account,
            logout_account,
            catalog,
            media_detail,
            update_media_metadata,
            refresh_media_metadata,
            apply_metadata_candidate,
            set_media_flag,
            save_progress,
            scan_status,
            start_scan,
            cancel_scan,
            replace_library_root,
            technical_path,
            player_state,
            player_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running CINE WANA");
}
