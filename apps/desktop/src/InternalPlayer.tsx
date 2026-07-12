import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  ExternalLink, Maximize, Pause, Play, RotateCcw, SkipBack, SkipForward,
  SlidersHorizontal, Sparkles, Volume2, VolumeX, X
} from 'lucide-react';
import type { MediaDetail, MediaSummary } from './types';

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

const NEXT_CREDITS_LEAD_SECONDS = 30;
const NEXT_COUNTDOWN_SECONDS = 8;
const CONTROLS_HIDE_DELAY_MS = 2600;

export function InternalPlayer({
  source,
  onClose,
  onOpenExternal,
  onPlayNext,
  onProgressSaved,
}: {
  source: InternalPlayerSource;
  onClose: () => void;
  onOpenExternal: (id: string) => Promise<void>;
  onPlayNext: (id: string) => Promise<void>;
  onProgressSaved: () => void;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const saveTimerRef = useRef<number | null>(null);
  const controlsTimerRef = useRef<number | null>(null);
  const lastSavedAtRef = useRef(0);
  const lastPointerRef = useRef({ x: 0, y: 0, ready: false });
  const initialWindowFullscreenRef = useRef<boolean | null>(null);
  const restoredRef = useRef(false);
  const nextStartedAtRef = useRef<number | null>(null);
  const [playing, setPlaying] = useState(true);
  const [current, setCurrent] = useState(0);
  const [duration, setDuration] = useState(() => msToSeconds(source.detail.runtimeMs ?? source.detail.technical.durationMs ?? 0));
  const [volume, setVolume] = useState(0.82);
  const [muted, setMuted] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [showImagePanel, setShowImagePanel] = useState(false);
  const [image, setImage] = useState<ImageSettings>(defaultImage);
  const [analysis, setAnalysis] = useState<ImageAnalysis | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [nextMovie, setNextMovie] = useState<MediaSummary | null>(null);
  const [nextPromptVisible, setNextPromptVisible] = useState(false);
  const [nextDismissed, setNextDismissed] = useState(false);
  const [nextCountdown, setNextCountdown] = useState(NEXT_COUNTDOWN_SECONDS);
  const [error, setError] = useState<string | null>(null);

  const title = displayTitle(source.detail);
  const resumeSeconds = msToSeconds(source.resumeMs);
  const canResume = resumeSeconds > 20 && !source.detail.completed;
  const nextProgress = clamp((NEXT_COUNTDOWN_SECONDS - nextCountdown) / NEXT_COUNTDOWN_SECONDS, 0, 1);

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

  const clearControlsTimer = useCallback(() => {
    if (controlsTimerRef.current) {
      window.clearTimeout(controlsTimerRef.current);
      controlsTimerRef.current = null;
    }
  }, []);

  const restoreWindowFullscreenMode = useCallback(async () => {
    const initialFullscreen = initialWindowFullscreenRef.current;
    try {
      const appWindow = getCurrentWindow();
      if (initialFullscreen !== null && (await appWindow.isFullscreen()) !== initialFullscreen) {
        await appWindow.setFullscreen(initialFullscreen);
      }
    } catch {
      /* Fall back to browser fullscreen below. */
    }
    if (document.fullscreenElement) {
      await document.exitFullscreen().catch(() => {});
    }
    setFullscreen(Boolean(initialFullscreen));
  }, []);

  const closePlayer = useCallback(() => {
    void (async () => {
      await saveProgress(true).catch(() => {});
      await restoreWindowFullscreenMode();
      onClose();
    })();
  }, [onClose, restoreWindowFullscreenMode, saveProgress]);

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
    void (async () => {
      const target = shellRef.current;
      try {
        const appWindow = getCurrentWindow();
        const active = (await appWindow.isFullscreen()) || Boolean(document.fullscreenElement);
        const next = !active;
        await appWindow.setFullscreen(next);
        if (!next && document.fullscreenElement) {
          await document.exitFullscreen().catch(() => {});
        }
        setFullscreen(next);
        setControlsVisible(true);
        return;
      } catch {
        /* Browser fullscreen is the fallback when the native call is unavailable. */
      }
      if (!target) return;
      try {
        if (document.fullscreenElement) {
          await document.exitFullscreen();
          setFullscreen(false);
        } else {
          await target.requestFullscreen();
          setFullscreen(true);
        }
        setControlsVisible(true);
      } catch {
        setError('No se pudo activar pantalla completa.');
      }
    })();
  }, []);

  const openExternal = useCallback(async () => {
    const video = videoRef.current;
    if (video) video.pause();
    setPlaying(false);
    await saveProgress(true);
    await restoreWindowFullscreenMode();
    await onOpenExternal(source.detail.id);
  }, [onOpenExternal, restoreWindowFullscreenMode, saveProgress, source.detail.id]);

  const beginNextPrompt = useCallback(() => {
    if (!nextMovie || nextDismissed || nextPromptVisible) return;
    nextStartedAtRef.current = Date.now();
    setNextCountdown(NEXT_COUNTDOWN_SECONDS);
    setNextPromptVisible(true);
    setControlsVisible(true);
  }, [nextDismissed, nextMovie, nextPromptVisible]);

  const cancelNextPrompt = useCallback(() => {
    nextStartedAtRef.current = null;
    setNextPromptVisible(false);
    setNextDismissed(true);
    setNextCountdown(NEXT_COUNTDOWN_SECONDS);
    void saveProgress(true);
  }, [saveProgress]);

  const startNextMovie = useCallback(async () => {
    if (!nextMovie) return;
    nextStartedAtRef.current = null;
    setNextPromptVisible(false);
    setNextDismissed(true);
    await saveProgress(true);
    await onPlayNext(nextMovie.id);
  }, [nextMovie, onPlayNext, saveProgress]);

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
    clearControlsTimer();
    if (!playing || showImagePanel || nextPromptVisible || error) {
      setControlsVisible(true);
      return;
    }
    if (controlsVisible) {
      controlsTimerRef.current = window.setTimeout(() => {
        setControlsVisible(false);
        controlsTimerRef.current = null;
      }, CONTROLS_HIDE_DELAY_MS);
    }
    return clearControlsTimer;
  }, [clearControlsTimer, controlsVisible, error, nextPromptVisible, playing, showImagePanel]);

  useEffect(() => {
    const revealControls = () => {
      setControlsVisible(true);
    };
    const onMove = (event: MouseEvent) => {
      const last = lastPointerRef.current;
      if (last.ready && Math.abs(event.clientX - last.x) < 2 && Math.abs(event.clientY - last.y) < 2) return;
      lastPointerRef.current = { x: event.clientX, y: event.clientY, ready: true };
      revealControls();
    };
    const onFullscreen = () => {
      setFullscreen(Boolean(document.fullscreenElement));
      revealControls();
    };
    window.addEventListener('mousemove', onMove);
    document.addEventListener('fullscreenchange', onFullscreen);
    return () => {
      window.removeEventListener('mousemove', onMove);
      document.removeEventListener('fullscreenchange', onFullscreen);
    };
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const appWindow = getCurrentWindow();
        const isFullscreen = await appWindow.isFullscreen();
        initialWindowFullscreenRef.current = isFullscreen;
        setFullscreen(isFullscreen || Boolean(document.fullscreenElement));
      } catch {
        initialWindowFullscreenRef.current = Boolean(document.fullscreenElement);
        setFullscreen(Boolean(document.fullscreenElement));
      }
    })();
    return () => {
      clearControlsTimer();
      void restoreWindowFullscreenMode();
    };
  }, [clearControlsTimer, restoreWindowFullscreenMode]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const tag = (event.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (event.code === 'Space') { event.preventDefault(); togglePlay(); }
      if (event.key.toLowerCase() === 'j' || event.key === 'ArrowLeft') { event.preventDefault(); seekBy(-10); }
      if (event.key.toLowerCase() === 'l' || event.key === 'ArrowRight') { event.preventDefault(); seekBy(10); }
      if (event.key.toLowerCase() === 'm') { event.preventDefault(); setMuted(value => !value); }
      if (event.key.toLowerCase() === 'f') { event.preventDefault(); toggleFullscreen(); }
      if (event.key === 'Escape') {
        event.preventDefault();
        if (fullscreen || document.fullscreenElement) {
          toggleFullscreen();
        } else {
          closePlayer();
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [closePlayer, fullscreen, seekBy, toggleFullscreen, togglePlay]);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) window.clearInterval(saveTimerRef.current);
    };
  }, []);

  useEffect(() => {
    restoredRef.current = false;
    lastSavedAtRef.current = 0;
    nextStartedAtRef.current = null;
    setPlaying(true);
    setCurrent(0);
    setDuration(msToSeconds(source.detail.runtimeMs ?? source.detail.technical.durationMs ?? 0));
    setError(null);
    setNextMovie(null);
    setNextPromptVisible(false);
    setNextDismissed(false);
    setNextCountdown(NEXT_COUNTDOWN_SECONDS);
    setAnalysis(null);
    setImage(defaultImage);
  }, [source.detail.id, source.detail.runtimeMs, source.detail.technical.durationMs]);

  useEffect(() => {
    let cancelled = false;
    if (source.detail.kind !== 'movie') {
      setNextMovie(null);
      return () => { cancelled = true; };
    }
    invoke<MediaSummary | null>('next_movie', { mediaId: source.detail.id })
      .then(next => {
        if (!cancelled) setNextMovie(next);
      })
      .catch(() => {
        if (!cancelled) setNextMovie(null);
      });
    return () => { cancelled = true; };
  }, [source.detail.id, source.detail.kind]);

  useEffect(() => {
    if (!nextMovie || nextDismissed || nextPromptVisible || duration <= 0) return;
    if (duration < NEXT_CREDITS_LEAD_SECONDS + NEXT_COUNTDOWN_SECONDS + 8) return;
    if (duration - current <= NEXT_CREDITS_LEAD_SECONDS) beginNextPrompt();
  }, [beginNextPrompt, current, duration, nextDismissed, nextMovie, nextPromptVisible]);

  useEffect(() => {
    if (!nextPromptVisible || !nextMovie) return;
    const timer = window.setInterval(() => {
      const startedAt = nextStartedAtRef.current ?? Date.now();
      const remaining = Math.max(0, NEXT_COUNTDOWN_SECONDS - (Date.now() - startedAt) / 1000);
      setNextCountdown(remaining);
      if (remaining <= 0) {
        window.clearInterval(timer);
        void startNextMovie();
      }
    }, 120);
    return () => window.clearInterval(timer);
  }, [nextMovie, nextPromptVisible, startNextMovie]);

  const progress = duration > 0 ? clamp(current / duration, 0, 1) : 0;

  return (
    <div ref={shellRef} className={`cw-player-shell ${controlsVisible ? '' : 'idle'}`}>
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
          if (nextMovie && !nextDismissed) {
            beginNextPrompt();
          } else {
            void saveProgress(true);
          }
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

      {nextMovie && nextPromptVisible && (
        <aside className="cw-next-up" onClick={event => event.stopPropagation()}>
          <div className="cw-next-poster">
            {nextMovie.artworkUrl ? <img src={convertFileSrc(nextMovie.artworkUrl)} alt="" /> : <span>{initials(nextMovie.title)}</span>}
          </div>
          <div className="cw-next-copy">
            <small>Siguiente parte</small>
            <b>{nextMovie.title}</b>
            <span>Empieza en {Math.ceil(nextCountdown)} s</span>
            <div className="cw-next-meter"><i style={{ width: `${nextProgress * 100}%` }} /></div>
          </div>
          <button onClick={cancelNextPrompt}>Cancelar</button>
        </aside>
      )}

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
            <button className={fullscreen ? 'selected' : ''} onClick={toggleFullscreen} title="Pantalla completa"><Maximize size={19}/></button>
          </div>
        </div>
      </div>
    </div>
  );
}

function displayTitle(item: MediaDetail) {
  return item.kind === 'episode' ? (item.seriesTitle || item.title) : item.title;
}

function initials(value: string) {
  return value.split(/\s+/).filter(Boolean).slice(0, 3).map(part => part[0]).join('').toUpperCase();
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
