use anyhow::{Context, Result};
use cinewana_core::{
    ImageAdjustment, ImageAnalysis, MediaTechnical, is_supported_video, parse_media_name,
};
use cinewana_database::DiscoveredFile;
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, UNIX_EPOCH},
};
use tokio::process::Command;
use walkdir::WalkDir;

const ANALYSIS_WIDTH: usize = 160;
const ANALYSIS_HEIGHT: usize = 90;
const ANALYSIS_FRAME_BYTES: usize = ANALYSIS_WIDTH * ANALYSIS_HEIGHT * 3;

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
    let mut command = Command::new(executable);
    hide_console_window(&mut command);
    let output = command
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

pub async fn analyze_image(
    ffmpeg: &Path,
    video: &Path,
    duration_ms: Option<i64>,
) -> Result<ImageAnalysis> {
    analyze_image_with_progress(ffmpeg, video, duration_ms, |_, _, _| {}).await
}

pub async fn analyze_image_with_progress<F>(
    ffmpeg: &Path,
    video: &Path,
    duration_ms: Option<i64>,
    mut progress: F,
) -> Result<ImageAnalysis>
where
    F: FnMut(u32, u32, u32),
{
    let seek_points = analysis_seek_points(duration_ms);
    let total = seek_points.len() as u32;
    let mut frames = Vec::new();
    let mut last_error = None;
    progress(0, total, 0);
    for (index, seek_seconds) in seek_points.into_iter().enumerate() {
        match sample_analysis_frame(ffmpeg, video, seek_seconds).await {
            Ok(frame) if frame.len() >= ANALYSIS_FRAME_BYTES => frames.push(frame),
            Ok(frame) => {
                last_error = Some(anyhow::anyhow!(
                    "ffmpeg returned {} bytes for an image sample",
                    frame.len()
                ));
            }
            Err(error) => last_error = Some(error),
        }
        progress((index + 1) as u32, total, frames.len() as u32);
    }
    if frames.is_empty() {
        if let Some(error) = last_error {
            return Err(error).context("No se pudo leer ninguna escena del video");
        }
        anyhow::bail!("No se pudo leer ninguna escena del video");
    }
    Ok(analyze_rgb_samples(
        &frames,
        ANALYSIS_WIDTH,
        ANALYSIS_HEIGHT,
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

fn analysis_seek_points(duration_ms: Option<i64>) -> Vec<f64> {
    let Some(duration_ms) = duration_ms else {
        return vec![
            2.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1_200.0, 1_800.0, 2_400.0, 3_000.0,
            3_600.0, 4_500.0, 5_400.0, 6_300.0,
        ];
    };
    let duration = duration_ms as f64 / 1000.0;
    if duration <= 6.0 {
        let end_safe = (duration - 0.2).max(0.1);
        return [0.18, 0.5, 0.82]
            .into_iter()
            .map(|ratio| (duration * ratio).clamp(0.05, end_safe))
            .collect();
    }
    let samples = ((duration / 240.0).round() as usize).clamp(12, 32);
    let end_safe = (duration - 1.0).max(1.0);
    (0..samples)
        .map(|index| ((index as f64 + 0.5) / samples as f64) * end_safe)
        .map(|seconds| seconds.clamp(0.75, end_safe))
        .collect()
}

async fn sample_analysis_frame(ffmpeg: &Path, video: &Path, seek_seconds: f64) -> Result<Vec<u8>> {
    let seek = format!("{seek_seconds:.3}");
    let filter =
        format!("scale={ANALYSIS_WIDTH}:{ANALYSIS_HEIGHT}:flags=fast_bilinear,format=rgb24");
    let mut command = Command::new(ffmpeg);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-ss",
            &seek,
            "-i",
        ])
        .arg(video)
        .args([
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            "-an",
            "-sn",
            "-vf",
            &filter,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    hide_console_window(&mut command);
    let output = tokio::time::timeout(Duration::from_secs(15), command.output())
        .await
        .context("ffmpeg image analysis timed out")?
        .context("launch ffmpeg")?;
    if !output.status.success() {
        anyhow::bail!(
            "ffmpeg image analysis failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

fn analyze_rgb_samples(samples: &[Vec<u8>], width: usize, height: usize) -> ImageAnalysis {
    let expected = width * height * 3;
    let mut luma_total = 0.0;
    let mut saturation_total = 0.0;
    let mut red_total = 0.0;
    let mut blue_total = 0.0;
    let mut pixels_total = 0u64;
    let mut shadows = 0u64;
    let mut highlights = 0u64;
    let mut histogram = [0u64; 256];

    for frame in samples {
        if frame.len() < expected {
            continue;
        }
        for pixel in frame[..expected].chunks_exact(3) {
            let r = pixel[0] as f64;
            let g = pixel[1] as f64;
            let b = pixel[2] as f64;
            let luma = r * 0.2126 + g * 0.7152 + b * 0.0722;
            let max_channel = r.max(g).max(b);
            let min_channel = r.min(g).min(b);
            let saturation = if max_channel > 0.0 {
                (max_channel - min_channel) / max_channel
            } else {
                0.0
            };
            let bucket = luma.round().clamp(0.0, 255.0) as usize;
            histogram[bucket] += 1;
            luma_total += luma;
            saturation_total += saturation;
            red_total += r;
            blue_total += b;
            pixels_total += 1;
            if luma < 45.0 {
                shadows += 1;
            }
            if luma > 210.0 {
                highlights += 1;
            }
        }
    }

    if pixels_total == 0 {
        return ImageAnalysis::default();
    }

    let average_luma = luma_total / pixels_total as f64;
    let average_saturation = saturation_total / pixels_total as f64 * 100.0;
    let warmth = ((red_total - blue_total) / pixels_total as f64 / 255.0 * 100.0).round() as i32;
    let shadows_percent = percent(shadows, pixels_total);
    let highlights_percent = percent(highlights, pixels_total);
    let p10 = histogram_percentile(&histogram, pixels_total, 0.10) as f64;
    let p90 = histogram_percentile(&histogram, pixels_total, 0.90) as f64;

    ImageAnalysis {
        average_light: ((average_luma / 255.0) * 100.0).round() as u32,
        shadows_percent,
        highlights_percent,
        average_saturation: average_saturation.round() as u32,
        warmth,
        sampled_frames: samples.len() as u32,
        suggested: suggest_image_adjustment(
            average_luma,
            shadows_percent as f64,
            highlights_percent as f64,
            average_saturation,
            warmth as f64,
            p90 - p10,
            p10,
            p90,
        ),
    }
}

fn suggest_image_adjustment(
    average_luma: f64,
    shadows_percent: f64,
    highlights_percent: f64,
    average_saturation: f64,
    warmth: f64,
    dynamic_range: f64,
    p10: f64,
    p90: f64,
) -> ImageAdjustment {
    let average_light = average_luma / 255.0 * 100.0;
    let brightness = clamp_i32(((50.0 - average_light) / 2.6).round() as i32, -18, 18);
    let mut contrast = clamp_i32(((145.0 - dynamic_range) / 7.0).round() as i32, -8, 16);
    if dynamic_range < 90.0 {
        contrast = clamp_i32(contrast + 4, -8, 16);
    }
    let shadows = if p10 > 42.0 && shadows_percent < 8.0 {
        -6
    } else {
        clamp_i32(((shadows_percent - 18.0) / 2.3).round() as i32, 0, 28)
    };
    let highlights = if highlights_percent > 5.0 {
        -clamp_i32(((highlights_percent - 3.0) / 2.0).round() as i32, 1, 24)
    } else if p90 < 175.0 && average_light < 48.0 {
        4
    } else {
        0
    };
    let saturation = clamp_i32(
        ((34.0 - average_saturation) / 2.4).round() as i32 + 4,
        -8,
        16,
    );
    let temperature = clamp_i32((-warmth / 2.2).round() as i32, -14, 14);
    ImageAdjustment {
        brightness,
        contrast,
        saturation,
        shadows,
        highlights,
        temperature,
    }
}

fn percent(part: u64, total: u64) -> u32 {
    if total == 0 {
        0
    } else {
        (part as f64 / total as f64 * 100.0).round() as u32
    }
}

fn histogram_percentile(histogram: &[u64; 256], total: u64, percentile: f64) -> u8 {
    let target = ((total as f64 * percentile).ceil() as u64).max(1);
    let mut seen = 0u64;
    for (value, count) in histogram.iter().enumerate() {
        seen += *count;
        if seen >= target {
            return value as u8;
        }
    }
    255
}

fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
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
    hide_console_window(&mut command);
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

#[cfg(windows)]
fn hide_console_window(command: &mut Command) {
    command.creation_flags(0x08000000);
}

#[cfg(not(windows))]
fn hide_console_window(_command: &mut Command) {}

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

    fn solid_frame(red: u8, green: u8, blue: u8) -> Vec<u8> {
        let mut frame = Vec::with_capacity(ANALYSIS_FRAME_BYTES);
        for _ in 0..(ANALYSIS_WIDTH * ANALYSIS_HEIGHT) {
            frame.extend_from_slice(&[red, green, blue]);
        }
        frame
    }

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

    #[test]
    fn analysis_suggests_lifting_dark_flat_video() {
        let frames = vec![solid_frame(28, 30, 32)];
        let analysis = analyze_rgb_samples(&frames, ANALYSIS_WIDTH, ANALYSIS_HEIGHT);

        assert!(analysis.average_light < 15);
        assert!(analysis.shadows_percent > 90);
        assert!(analysis.suggested.brightness > 0);
        assert!(analysis.suggested.shadows > 0);
        assert!(analysis.suggested.saturation > 0);
    }

    #[test]
    fn analysis_cools_strong_warm_cast_gently() {
        let frames = vec![solid_frame(180, 120, 70)];
        let analysis = analyze_rgb_samples(&frames, ANALYSIS_WIDTH, ANALYSIS_HEIGHT);

        assert!(analysis.warmth > 0);
        assert!(analysis.suggested.temperature < 0);
    }
}
