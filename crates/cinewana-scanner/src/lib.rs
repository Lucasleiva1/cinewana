use anyhow::{Context, Result};
use cinewana_core::{MediaTechnical, is_supported_video, parse_media_name};
use cinewana_database::DiscoveredFile;
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Stdio,
    time::UNIX_EPOCH,
};
use tokio::process::Command;
use walkdir::WalkDir;

pub fn discover(root: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let depth = if recursive { usize::MAX } else { 1 };
    let mut videos = WalkDir::new(root)
        .follow_links(false)
        .max_depth(depth)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && is_supported_video(entry.path()))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    videos.sort_by(|a, b| {
        a.to_string_lossy()
            .to_lowercase()
            .cmp(&b.to_string_lossy().to_lowercase())
    });
    Ok(videos)
}

pub async fn inspect(path: &Path, ffprobe: Option<&Path>) -> Result<DiscoveredFile> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("read metadata for {}", path.display()))?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0);
    let technical = match ffprobe {
        Some(executable) => probe(executable, path).await.unwrap_or_default(),
        None => MediaTechnical::default(),
    };
    Ok(DiscoveredFile {
        path: path.to_string_lossy().into_owned(),
        file_name: path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned(),
        file_size: metadata.len() as i64,
        modified_at,
        fingerprint: fingerprint(path, metadata.len(), modified_at)
            .unwrap_or_else(|_| format!("{}:{modified_at}", metadata.len())),
        parsed: parse_media_name(path),
        technical,
        external_subtitles: find_external_subtitles(path),
    })
}

pub async fn probe(executable: &Path, path: &Path) -> Result<MediaTechnical> {
    let output = Command::new(executable)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .context("launch ffprobe")?;
    if !output.status.success() {
        anyhow::bail!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    parse_ffprobe(&serde_json::from_slice(&output.stdout).context("parse ffprobe JSON")?)
}

pub async fn generate_artwork(
    ffmpeg: &Path,
    video: &Path,
    cache_root: &Path,
    fingerprint: &str,
    duration_ms: Option<i64>,
) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let posters = cache_root.join("posters");
    let backdrops = cache_root.join("backdrops");
    let previews = cache_root.join("previews");
    std::fs::create_dir_all(&posters)?;
    std::fs::create_dir_all(&backdrops)?;
    std::fs::create_dir_all(&previews)?;
    let key = fingerprint.chars().take(48).collect::<String>();
    let poster = posters.join(format!("{key}.jpg"));
    let backdrop = backdrops.join(format!("{key}.jpg"));
    let preview = previews.join(format!("{key}.mp4"));
    let seek_points = seek_points(duration_ms);
    if !valid_cache_file(&backdrop, 8_000) {
        generate_frame(
            ffmpeg,
            video,
            &seek_points,
            &[
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-vf",
                "scale=1280:720:force_original_aspect_ratio=increase,crop=1280:720",
                "-q:v",
                "3",
            ],
            &backdrop,
        )
        .await?;
    }
    if !valid_cache_file(&poster, 8_000) {
        generate_frame(
            ffmpeg,
            video,
            &seek_points,
            &[
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-vf",
                "scale=600:900:force_original_aspect_ratio=increase,crop=600:900",
                "-q:v",
                "3",
            ],
            &poster,
        )
        .await?;
    }
    if !valid_cache_file(&preview, 24_000) {
        let seek = format!("{:.3}", seek_points[0]);
        let _ = run_ffmpeg(
            ffmpeg,
            &[
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-y",
                "-ss",
                &seek,
                "-i",
            ],
            video,
            &[
                "-map",
                "0:v:0",
                "-t",
                "8",
                "-an",
                "-vf",
                "scale=640:-2:force_original_aspect_ratio=decrease",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "28",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ],
            &preview,
        )
        .await;
    }
    Ok((
        poster,
        backdrop,
        if valid_cache_file(&preview, 24_000) {
            preview
        } else {
            PathBuf::new()
        },
    ))
}

fn seek_points(duration_ms: Option<i64>) -> Vec<f64> {
    let Some(duration_ms) = duration_ms else {
        return vec![30.0, 60.0, 120.0, 10.0, 2.0];
    };
    let duration = duration_ms as f64 / 1000.0;
    if duration <= 2.0 {
        return vec![0.2, 0.8, 1.2];
    }
    let end_safe = (duration - 1.0).max(1.0);
    [0.12, 0.35, 0.6, 0.82]
        .into_iter()
        .map(|ratio| (duration * ratio).clamp(1.0, end_safe))
        .collect()
}

async fn generate_frame(
    ffmpeg: &Path,
    video: &Path,
    seek_points: &[f64],
    suffix: &[&str],
    output: &Path,
) -> Result<()> {
    let mut last_error = None;
    for seek_seconds in seek_points {
        let seek = format!("{seek_seconds:.3}");
        match run_ffmpeg(
            ffmpeg,
            &[
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-y",
                "-ss",
                &seek,
                "-i",
            ],
            video,
            suffix,
            output,
        )
        .await
        {
            Ok(()) if valid_cache_file(output, 8_000) => return Ok(()),
            Ok(()) => {
                let _ = std::fs::remove_file(output);
                last_error = Some(anyhow::anyhow!("generated frame was too small"));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no usable frame generated")))
}

async fn run_ffmpeg(
    ffmpeg: &Path,
    prefix: &[&str],
    input: &Path,
    suffix: &[&str],
    output: &Path,
) -> Result<()> {
    let mut command = Command::new(ffmpeg);
    command
        .args(prefix)
        .arg(input)
        .args(suffix)
        .arg(output)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let result = command.output().await.context("launch ffmpeg")?;
    if !result.status.success() {
        let _ = std::fs::remove_file(output);
        anyhow::bail!(
            "ffmpeg artwork generation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    Ok(())
}

fn valid_cache_file(path: &Path, minimum_size: u64) -> bool {
    path.metadata()
        .map(|metadata| metadata.len() >= minimum_size)
        .unwrap_or(false)
}

pub fn parse_ffprobe(value: &Value) -> Result<MediaTechnical> {
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let video = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(Value::as_str) == Some("video"));
    let audio = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let format = value.get("format");
    let duration_ms = format
        .and_then(|f| f.get("duration"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v * 1000.0).round() as i64);
    let transfer = video
        .and_then(|v| v.get("color_transfer"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let hdr_type = match transfer {
        "smpte2084" => Some("HDR10".to_string()),
        "arib-std-b67" => Some("HLG".to_string()),
        _ => None,
    };
    Ok(MediaTechnical {
        duration_ms,
        width: video.and_then(|v| v.get("width")).and_then(Value::as_i64),
        height: video.and_then(|v| v.get("height")).and_then(Value::as_i64),
        container: format
            .and_then(|f| f.get("format_name"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        video_codec: video
            .and_then(|v| v.get("codec_name"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        audio_codec: audio
            .and_then(|v| v.get("codec_name"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        hdr_type,
    })
}

fn fingerprint(path: &Path, size: u64, modified: i64) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let sample = 64 * 1024;
    let mut buffer = vec![0u8; sample];
    let first = file.read(&mut buffer)?;
    hasher.update(&buffer[..first]);
    if size > sample as u64 {
        file.seek(SeekFrom::End(-(sample as i64)))?;
        let last = file.read(&mut buffer)?;
        hasher.update(&buffer[..last]);
    }
    // Modification time separates in-place rewrites while the sampled content stays equal.
    hasher.update(&modified.to_le_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

fn find_external_subtitles(video: &Path) -> Vec<String> {
    let Some(parent) = video.parent() else {
        return vec![];
    };
    let stem = video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let mut matches = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|candidate| {
            let extension = candidate
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let candidate_stem = candidate
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_lowercase();
            matches!(extension.as_str(), "srt" | "ass" | "ssa" | "vtt")
                && candidate_stem.starts_with(&stem)
        })
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    matches.sort();
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_probe_json() {
        let parsed = parse_ffprobe(&json!({
          "streams": [{"codec_type":"video","codec_name":"hevc","width":3840,"height":2160,"color_transfer":"smpte2084"},{"codec_type":"audio","codec_name":"aac"}],
          "format": {"format_name":"matroska,webm","duration":"120.500"}
        })).unwrap();
        assert_eq!(parsed.duration_ms, Some(120_500));
        assert_eq!(parsed.height, Some(2160));
        assert_eq!(parsed.hdr_type.as_deref(), Some("HDR10"));
    }
}
