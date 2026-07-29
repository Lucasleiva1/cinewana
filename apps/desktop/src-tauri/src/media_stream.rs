use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, State as AxumState},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Clone)]
struct MediaHttpState {
    server_token: String,
    paths: Arc<Mutex<HashMap<String, PathBuf>>>,
}

#[derive(Clone)]
pub struct MediaStreamService {
    server_port: u16,
    server_token: String,
    paths: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl MediaStreamService {
    pub fn new() -> anyhow::Result<Self> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let server_port = listener.local_addr()?.port();
        let server_token = Uuid::new_v4().simple().to_string();
        let paths = Arc::new(Mutex::new(HashMap::new()));
        let state = MediaHttpState {
            server_token: server_token.clone(),
            paths: paths.clone(),
        };
        let router = Router::new()
            .route("/media/{server_token}/{media_token}", get(serve_media))
            .with_state(state);
        tauri::async_runtime::spawn(async move {
            if let Ok(listener) = tokio::net::TcpListener::from_std(listener) {
                let _ = axum::serve(listener, router).await;
            }
        });
        Ok(Self {
            server_port,
            server_token,
            paths,
        })
    }

    pub fn register(&self, path: PathBuf) -> Result<String, String> {
        if !path.is_file() {
            return Err("El archivo original no está disponible.".into());
        }
        let media_token = Uuid::new_v4().simple().to_string();
        self.paths.lock().insert(media_token.clone(), path);
        Ok(format!(
            "http://127.0.0.1:{}/media/{}/{}",
            self.server_port, self.server_token, media_token
        ))
    }
}

async fn serve_media(
    AxumPath((server_token, media_token)): AxumPath<(String, String)>,
    AxumState(state): AxumState<MediaHttpState>,
    headers: HeaderMap,
) -> Response {
    if server_token != state.server_token {
        return cors_response(StatusCode::UNAUTHORIZED, Body::empty());
    }
    let Some(path) = state.paths.lock().get(&media_token).cloned() else {
        return cors_response(StatusCode::NOT_FOUND, Body::empty());
    };
    let Ok(metadata) = tokio::fs::metadata(&path).await else {
        return cors_response(StatusCode::NOT_FOUND, Body::empty());
    };
    let total = metadata.len();
    if total == 0 {
        return cors_response(StatusCode::RANGE_NOT_SATISFIABLE, Body::empty());
    }
    let range = match headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
    {
        Some(value) => match parse_range(value, total) {
            Some(range) => Some(range),
            None => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .body(Body::empty())
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
            }
        },
        None => None,
    };
    let (start, end, status) = range
        .map(|(start, end)| (start, end, StatusCode::PARTIAL_CONTENT))
        .unwrap_or((0, total - 1, StatusCode::OK));
    let length = end - start + 1;
    let Ok(mut file) = File::open(&path).await else {
        return cors_response(StatusCode::NOT_FOUND, Body::empty());
    };
    if file.seek(SeekFrom::Start(start)).await.is_err() {
        return cors_response(StatusCode::INTERNAL_SERVER_ERROR, Body::empty());
    }
    let stream = ReaderStream::with_capacity(file.take(length), 64 * 1024);
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_for(&path))
        .header(header::CONTENT_LENGTH, length)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            "accept-ranges, content-length, content-range",
        )
        .header("cross-origin-resource-policy", "cross-origin")
        .header(header::CACHE_CONTROL, "no-store");
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }
    response
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn cors_response(status: StatusCode, body: Body) -> Response {
    Response::builder()
        .status(status)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("cross-origin-resource-policy", "cross-origin")
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let range = value.strip_prefix("bytes=")?;
    if range.contains(',') {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?.min(total);
        if suffix == 0 {
            return None;
        }
        return Some((total - suffix, total - 1));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= total {
        return None;
    }
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().ok()?.min(total - 1)
    };
    (end >= start).then_some((start, end))
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "ts" | "m2ts" => "video/mp2t",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn parses_browser_byte_ranges() {
        assert_eq!(parse_range("bytes=0-", 1_000), Some((0, 999)));
        assert_eq!(parse_range("bytes=100-199", 1_000), Some((100, 199)));
        assert_eq!(parse_range("bytes=-100", 1_000), Some((900, 999)));
    }

    #[test]
    fn rejects_invalid_or_multiple_ranges() {
        assert_eq!(parse_range("bytes=1000-", 1_000), None);
        assert_eq!(parse_range("bytes=300-200", 1_000), None);
        assert_eq!(parse_range("bytes=0-1,4-5", 1_000), None);
    }
}
