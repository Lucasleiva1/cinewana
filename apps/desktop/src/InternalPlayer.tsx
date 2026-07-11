import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  ExternalLink, Maximize, Pause, Play, RotateCcw, SkipBack, SkipForward,
  SlidersHorizontal, Sparkles, Volume2, VolumeX, X
} from 'lucide-react';
import type { MediaDetail } from './types';

export interface InternalPlayerSource {
  detail: MediaDetail;
  path: string;
  url: string;
  resumeMs: number;
}

interface ImageSettings {
  brightness: number;
  contrast: number;
  saturation: number;
  shadows: number;
  highlights: number;
  temperature: number;
}

interface ImageAnalysis {
  averageLight: number;
  shadowsPercent: number;
  highlightsPercent: number;
  sampledFrames: number;
  suggested: ImageSettings;
}

const defaultImage: ImageSettings = {
  brightness: 0,
  contrast: 0,
  saturation: 0,
  shadows: 0,
  highlights: 0,
  temperature: 0,
};

export function InternalPlayer({
  source,
  onClose,
  onOpenExternal,
  onProgressSaved,
}: {
  source: InternalPlayerSource;
  onClose: () => void;
  onOpenExternal: (id: string) => Promise<void>;
  onProgressSaved: () => void;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const saveTimerRef = useRef<number | null>(null);
  const lastSavedAtRef = useRef(0);
  const restoredRef = useRef(false);
  const [playing, setPlaying] = useState(true);
  const [current, setCurrent] = useState(0);
  const [duration, setDuration] = useState(() => msToSeconds(source.detail.runtimeMs ?? source.detail.technical.durationMs ?? 0));
  const [volume, setVolume] = useState(0.82);
  const [muted, setMuted] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [showImagePanel, setShowImagePanel] = useState(false);
  const [image, setImage] = useState<ImageSettings>(defaultImage);
  const [analysis, setAnalysis] = useState<ImageAnalysis | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const title = displayTitle(source.detail);
  const resumeSeconds = msToSeconds(source.resumeMs);
  const canResume = resumeSeconds > 20 && !source.detail.completed;

  const imageFilter = useMemo(() => {
    const brightness = 1 + image.brightness / 100;
    const contrast = 1 + image.contrast / 100;
    const saturation = 1 + image.saturation / 100;
    return `brightness(${brightness}) contrast(${contrast}) saturate(${saturation})`;
  }, [image]);

  const imageOverlay = useMemo(() => ({
    '--shadow-lift': image.shadows > 0 ? Math.min(image.shadows / 120, 0.42) : 0,
    '--shadow-crush': image.shadows < 0 ? Math.min(Math.abs(image.shadows) / 140, 0.36) : 0,
    '--highlight-lift': image.highlights > 0 ? Math.min(image.highlights / 155, 0.34) : 0,
    '--highlight-recover': image.highlights < 0 ? Math.min(Math.abs(image.highlights) / 180, 0.28) : 0,
    '--temperature-opacity': Math.min(Math.abs(image.temperature) / 120, 0.36),
    '--temperature-color': image.temperature >= 0 ? 'rgba(255,170,85,1)' : 'rgba(95,150,255,1)',
  }) as React.CSSProperties, [image]);

  const saveProgress = useCallback(async (force = false) => {
    const video = videoRef.current;
    const videoDuration = Number.isFinite(video?.duration) && video!.duration > 0 ? video!.duration : duration;
    const videoCurrent = Number.isFinite(video?.currentTime) ? video!.currentTime : current;
    if (!videoDuration || videoDuration < 1) return;
    const now = Date.now();
    if (!force && now - lastSavedAtRef.current < 4500) return;
    lastSavedAtRef.current = now;
    await invoke('save_progress', {
      mediaId: source.detail.id,
      positionMs: Math.max(0, Math.round(videoCurrent * 1000)),
      durationMs: Math.max(0, Math.round(videoDuration * 1000)),
    });
    if (force) onProgressSaved();
  }, [current, duration, onProgressSaved, source.detail.id]);

  const closePlayer = useCallback(() => {
    void saveProgress(true).finally(onClose);
  }, [onClose, saveProgress]);

  const togglePlay = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) {
      video.play().then(() => setPlaying(true)).catch(() => setError('No se pudo iniciar la reproducción interna.'));
    } else {
      video.pause();
      setPlaying(false);
      void saveProgress(true);
    }
  }, [saveProgress]);

  const seekBy = useCallback((seconds: number) => {
    const video = videoRef.current;
    if (!video) return;
    video.currentTime = clamp(video.currentTime + seconds, 0, video.duration || duration || 0);
    setCurrent(video.currentTime);
  }, [duration]);

  const seekToPercent = useCallback((clientX: number, element: HTMLElement) => {
    const video = videoRef.current;
    if (!video) return;
    const rect = element.getBoundingClientRect();
    const percent = clamp((clientX - rect.left) / rect.width, 0, 1);
    video.currentTime = percent * (video.duration || duration || 0);
    setCurrent(video.currentTime);
  }, [duration]);

  const toggleFullscreen = useCallback(() => {
    const target = shellRef.current;
    if (!target) return;
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void target.requestFullscreen().catch(() => setError('No se pudo activar pantalla completa.'));
    }
  }, []);

  const openExternal = useCallback(async () => {
    const video = videoRef.current;
    if (video) video.pause();
    setPlaying(false);
    await saveProgress(true);
    await onOpenExternal(source.detail.id);
  }, [onOpenExternal, saveProgress, source.detail.id]);

  const analyzeImage = useCallback(async () => {
    const video = videoRef.current;
    if (!video || analyzing) return;
    const total = video.duration || duration;
    if (!total || total < 1) {
      setError('El video todavía no informó duración para analizar la imagen.');
      return;
    }
    setAnalyzing(true);
    setError(null);
    const wasPlaying = !video.paused;
    const originalTime = video.currentTime;
    video.pause();
    setPlaying(false);
    try {
      const canvas = document.createElement('canvas');
      canvas.width = 160;
      canvas.height = 90;
      const ctx = canvas.getContext('2d', { willReadFrequently: true });
      if (!ctx) throw new Error('No se pudo preparar el análisis.');
      const samples = Math.min(48, Math.max(12, Math.round(total / 180) * 8));
      let lumaTotal = 0;
      let pixelsTotal = 0;
      let shadows = 0;
      let highlights = 0;
      for (let index = 0; index < samples; index += 1) {
        const sampleAt = ((index + 0.5) / samples) * Math.max(0.1, total - 0.2);
        await seekVideo(video, sampleAt);
        ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
        const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
        for (let i = 0; i < data.length; i += 16) {
          const luma = data[i] * 0.2126 + data[i + 1] * 0.7152 + data[i + 2] * 0.0722;
          lumaTotal += luma;
          pixelsTotal += 1;
          if (luma < 45) shadows += 1;
          if (luma > 210) highlights += 1;
        }
      }
      const avg = pixelsTotal ? lumaTotal / pixelsTotal : 128;
      const shadowsPercent = pixelsTotal ? shadows / pixelsTotal * 100 : 0;
      const highlightsPercent = pixelsTotal ? highlights / pixelsTotal * 100 : 0;
      const suggested: ImageSettings = {
        brightness: clamp(Math.round((126 - avg) / 4), -18, 18),
        contrast: clamp(Math.round(10 - Math.abs(avg - 126) / 18), -4, 14),
        saturation: 6,
        shadows: clamp(Math.round(shadowsPercent / 2.4), 0, 26),
        highlights: clamp(Math.round(-highlightsPercent / 2.6), -24, 0),
        temperature: 0,
      };
      setAnalysis({
        averageLight: Math.round(avg / 255 * 100),
        shadowsPercent: Math.round(shadowsPercent),
        highlightsPercent: Math.round(highlightsPercent),
        sampledFrames: samples,
        suggested,
      });
      await seekVideo(video, originalTime);
      if (wasPlaying) {
        await video.play().catch(() => {});
        setPlaying(!video.paused);
      }
    } catch (cause) {
      setError(`No se pudo analizar la imagen: ${String(cause)}`);
      try { await seekVideo(video, originalTime); } catch { /* noop */ }
    } finally {
      setAnalyzing(false);
    }
  }, [analyzing, duration]);

  useEffect(() => {
    const timer = window.setTimeout(() => setControlsVisible(false), 3600);
    return () => window.clearTimeout(timer);
  }, [current, controlsVisible]);

  useEffect(() => {
    const onMove = () => setControlsVisible(true);
    const onFullscreen = () => setControlsVisible(true);
    window.addEventListener('mousemove', onMove);
    document.addEventListener('fullscreenchange', onFullscreen);
    return () => {
      window.removeEventListener('mousemove', onMove);
      document.removeEventListener('fullscreenchange', onFullscreen);
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const tag = (event.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (event.code === 'Space') { event.preventDefault(); togglePlay(); }
      if (event.key.toLowerCase() === 'j' || event.key === 'ArrowLeft') { event.preventDefault(); seekBy(-10); }
      if (event.key.toLowerCase() === 'l' || event.key === 'ArrowRight') { event.preventDefault(); seekBy(10); }
      if (event.key.toLowerCase() === 'm') { event.preventDefault(); setMuted(value => !value); }
      if (event.key.toLowerCase() === 'f') { event.preventDefault(); toggleFullscreen(); }
      if (event.key === 'Escape') { event.preventDefault(); closePlayer(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [closePlayer, seekBy, toggleFullscreen, togglePlay]);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) window.clearInterval(saveTimerRef.current);
    };
  }, []);

  const progress = duration > 0 ? clamp(current / duration, 0, 1) : 0;

  return (
    <div ref={shellRef} className={`cw-player-shell ${controlsVisible ? '' : 'idle'}`} onMouseMove={() => setControlsVisible(true)}>
      <video
        ref={videoRef}
        className="cw-player-video"
        src={source.url}
        autoPlay
        playsInline
        controls={false}
        muted={muted}
        style={{ filter: imageFilter }}
        onClick={togglePlay}
        onDoubleClick={toggleFullscreen}
        onLoadedMetadata={(event) => {
          const video = event.currentTarget;
          const nextDuration = Number.isFinite(video.duration) ? video.duration : duration;
          setDuration(nextDuration);
          if (!restoredRef.current && canResume && resumeSeconds < nextDuration * 0.9) {
            video.currentTime = resumeSeconds;
            setCurrent(resumeSeconds);
          }
          restoredRef.current = true;
        }}
        onPlay={() => {
          setPlaying(true);
          if (saveTimerRef.current) window.clearInterval(saveTimerRef.current);
          saveTimerRef.current = window.setInterval(() => void saveProgress(false), 5000);
        }}
        onPause={() => {
          setPlaying(false);
          void saveProgress(false);
        }}
        onTimeUpdate={(event) => {
          setCurrent(event.currentTarget.currentTime);
          setDuration(Number.isFinite(event.currentTarget.duration) ? event.currentTarget.duration : duration);
        }}
        onVolumeChange={(event) => {
          setVolume(event.currentTarget.volume);
          setMuted(event.currentTarget.muted);
        }}
        onEnded={() => {
          setPlaying(false);
          void saveProgress(true);
        }}
        onError={() => setError('No se pudo reproducir dentro de CINE WANA. Probá Abrir externo para este archivo.')}
      />
      <div className="cw-player-layer" style={imageOverlay}>
        <span className="shadow-lift" />
        <span className="shadow-crush" />
        <span className="highlight-lift" />
        <span className="highlight-recover" />
        <span className="temperature" />
      </div>

      {error && <div className="cw-player-error"><span>{error}</span><button onClick={() => setError(null)}><X size={16}/></button></div>}

      <div className="cw-player-top">
        <div>
          <span className="eyebrow">REPRODUCTOR CINE WANA</span>
          <h1>{title}</h1>
          {source.detail.kind === 'episode' && <p>{source.detail.seriesTitle} · T{source.detail.seasonNumber} E{source.detail.episodeNumber}</p>}
        </div>
        <div className="cw-player-top-actions">
          <button onClick={openExternal} title="Abrir con reproductor externo"><ExternalLink size={17}/>Externo</button>
          <button onClick={closePlayer} title="Cerrar reproductor"><X size={20}/></button>
        </div>
      </div>

      <div className="cw-player-center" onClick={togglePlay}>
        {!playing && <button className="cw-big-play"><Play fill="currentColor" size={34}/></button>}
      </div>

      {showImagePanel && (
        <aside className="cw-image-panel" onClick={event => event.stopPropagation()}>
          <div className="cw-panel-head">
            <div><b>Imagen</b><small>Retoca sólo la reproducción, no modifica el archivo.</small></div>
            <button onClick={() => setShowImagePanel(false)}><X size={16}/></button>
          </div>
          <button className="cw-analyze" onClick={analyzeImage} disabled={analyzing}>
            <Sparkles size={15}/>{analyzing ? 'Analizando escenas…' : 'Escanear video'}
          </button>
          {analysis && (
            <div className="cw-analysis">
              <div><span>Luz media</span><b>{analysis.averageLight}%</b></div>
              <div><span>Sombras</span><b>{analysis.shadowsPercent}%</b></div>
              <div><span>Altas luces</span><b>{analysis.highlightsPercent}%</b></div>
              <small>{analysis.sampledFrames} escenas revisadas.</small>
              <button onClick={() => setImage(analysis.suggested)}>Aplicar sugerido</button>
            </div>
          )}
          {([
            ['Brillo', 'brightness', -50, 50],
            ['Contraste', 'contrast', -50, 50],
            ['Saturación', 'saturation', -50, 50],
            ['Sombras', 'shadows', -50, 50],
            ['Luces', 'highlights', -50, 50],
            ['Temperatura', 'temperature', -50, 50],
          ] as const).map(([label, key, min, max]) => (
            <label className="cw-slider" key={key}>
              <span>{label}<b>{image[key]}</b></span>
              <input min={min} max={max} step={1} type="range" value={image[key]} onChange={event => setImage(prev => ({...prev, [key]: Number(event.target.value)}))}/>
            </label>
          ))}
          <button className="cw-reset-image" onClick={() => { setImage(defaultImage); setAnalysis(null); }}>Reset imagen</button>
        </aside>
      )}

      <div className="cw-player-controls">
        <div className="cw-progress-hit" onPointerDown={(event) => {
          const track = event.currentTarget.querySelector('.cw-progress-track') as HTMLElement | null;
          if (!track) return;
          seekToPercent(event.clientX, track);
          const onMove = (move: PointerEvent) => seekToPercent(move.clientX, track);
          const onUp = () => {
            window.removeEventListener('pointermove', onMove);
            window.removeEventListener('pointerup', onUp);
            void saveProgress(true);
          };
          window.addEventListener('pointermove', onMove);
          window.addEventListener('pointerup', onUp);
        }}>
          <div className="cw-progress-track"><i style={{ width: `${progress * 100}%` }} /></div>
        </div>

        <div className="cw-controls-row">
          <div className="cw-controls-left">
            <button onClick={() => seekBy(-10)}><SkipBack size={20}/></button>
            <button className="cw-play" onClick={togglePlay}>{playing ? <Pause fill="currentColor" size={21}/> : <Play fill="currentColor" size={21}/>}</button>
            <button onClick={() => seekBy(10)}><SkipForward size={20}/></button>
            <button onClick={() => { const video = videoRef.current; if (video) { video.currentTime = 0; video.play().catch(() => {}); } }}><RotateCcw size={18}/></button>
            <span className="cw-time">{formatClock(current)} / {formatClock(duration)}</span>
          </div>
          <div className="cw-controls-right">
            <button onClick={() => setMuted(value => !value)}>{muted || volume === 0 ? <VolumeX size={19}/> : <Volume2 size={19}/>}</button>
            <input className="cw-volume" min={0} max={1} step={0.01} type="range" value={muted ? 0 : volume} onChange={event => {
              const next = Number(event.target.value);
              const video = videoRef.current;
              if (video) {
                video.volume = next;
                video.muted = next === 0;
              }
              setVolume(next);
              setMuted(next === 0);
            }}/>
            <button className={showImagePanel ? 'selected' : ''} onClick={() => setShowImagePanel(value => !value)}><SlidersHorizontal size={19}/><span>Imagen</span></button>
            <button onClick={toggleFullscreen}><Maximize size={19}/></button>
          </div>
        </div>
      </div>
    </div>
  );
}

function displayTitle(item: MediaDetail) {
  return item.kind === 'episode' ? (item.seriesTitle || item.title) : item.title;
}

function msToSeconds(ms: number) {
  return Math.max(0, ms / 1000);
}

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

function formatClock(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0:00';
  const whole = Math.floor(seconds);
  const h = Math.floor(whole / 3600);
  const m = Math.floor((whole % 3600) / 60);
  const s = whole % 60;
  return h > 0 ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}` : `${m}:${s.toString().padStart(2, '0')}`;
}

function seekVideo(video: HTMLVideoElement, seconds: number) {
  return new Promise<void>((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      cleanup();
      reject(new Error('El video tardó demasiado en buscar una escena.'));
    }, 3500);
    const cleanup = () => {
      window.clearTimeout(timeout);
      video.removeEventListener('seeked', onSeeked);
      video.removeEventListener('error', onError);
    };
    const onSeeked = () => {
      cleanup();
      resolve();
    };
    const onError = () => {
      cleanup();
      reject(new Error('No se pudo leer una escena del video.'));
    };
    video.addEventListener('seeked', onSeeked, { once: true });
    video.addEventListener('error', onError, { once: true });
    video.currentTime = seconds;
  });
}
