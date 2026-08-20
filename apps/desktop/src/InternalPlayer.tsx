import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  ExternalLink, Maximize, Pause, Play, RotateCcw, SkipBack, SkipForward,
  Settings, SlidersHorizontal, Sparkles, Volume2, VolumeX, X
} from 'lucide-react';
import type { ImageAnalysis, ImageAnalysisProgress, ImageSettings, MediaDetail, MediaSummary, RemoteCommand, RemotePlayerSnapshot } from './types';
import { NEXT_UP_LEAD_SECONDS, nextUpSecondsRemaining, shouldAutoplayNextUp, shouldOfferNextUp } from './playerNextUp';
import {
  PLAYER_HOLD_DELAY_MS,
  PLAYER_MAX_VOLUME,
  playerVolumeRouting,
  resolveVolumeWithDetent,
  seekSecondsForPoint,
  shouldPinPlayerChrome,
  shouldStartImageAnalysis,
} from './playerControls';

export interface InternalPlayerSource {
  detail: MediaDetail;
  url: string;
  resumeMs: number;
}

const defaultImage: ImageSettings = {
  brightness: 0,
  contrast: 0,
  saturation: 0,
  shadows: 0,
  highlights: 0,
  temperature: 0,
};

const CONTROLS_HIDE_DELAY_MS = 2600;
const SINGLE_CLICK_DELAY_MS = 240;
const SHOW_SUGGESTED_IMAGE_ADJUSTMENT = false;

interface PlayerAudioGraph {
  context: AudioContext;
  gain: GainNode;
  limiter: DynamicsCompressorNode;
  source: MediaElementAudioSourceNode;
}

export function InternalPlayer({
  source,
  onClose,
  onOpenExternal,
  onPlayNext,
  onProgressSaved,
  settingsPanel,
}: {
  source: InternalPlayerSource;
  onClose: () => void;
  onOpenExternal: (id: string) => Promise<void>;
  onPlayNext: (id: string) => Promise<void>;
  onProgressSaved: () => void;
  settingsPanel: ReactNode;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const saveTimerRef = useRef<number | null>(null);
  const controlsTimerRef = useRef<number | null>(null);
  const singleClickTimerRef = useRef<number | null>(null);
  const holdTimerRef = useRef<number | null>(null);
  const seekFeedbackTimerRef = useRef<number | null>(null);
  const lastSavedAtRef = useRef(0);
  const lastPointerRef = useRef({ x: 0, y: 0, ready: false });
  const pressStartRef = useRef({ x: 0, y: 0 });
  const holdActiveRef = useRef(false);
  const suppressSurfaceClickRef = useRef(false);
  const playbackRateBeforeHoldRef = useRef(1);
  const audioGraphRef = useRef<PlayerAudioGraph | null>(null);
  const desiredVolumeRef = useRef(0.5);
  const volumeDetentReachedAtRef = useRef<number | null>(null);
  const initialWindowFullscreenRef = useRef<boolean | null>(null);
  const restoredRef = useRef(false);
  const nextStartingRef = useRef(false);
  const imageAnalysisRunningRef = useRef(false);
  const [playing, setPlaying] = useState(true);
  const [current, setCurrent] = useState(0);
  const [duration, setDuration] = useState(() => msToSeconds(source.detail.runtimeMs ?? source.detail.technical.durationMs ?? 0));
  const [volume, setVolume] = useState(0.5);
  const [muted, setMuted] = useState(false);
  const [audioBoostReady, setAudioBoostReady] = useState(false);
  const [fastForward, setFastForward] = useState(false);
  const [seekFeedback, setSeekFeedback] = useState<{ seconds: number; key: number } | null>(null);
  const [fullscreen, setFullscreen] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [showImagePanel, setShowImagePanel] = useState(false);
  const [showSettingsPanel, setShowSettingsPanel] = useState(false);
  const [image, setImage] = useState<ImageSettings>(defaultImage);
  const [analysis, setAnalysis] = useState<ImageAnalysis | null>(null);
  const [analysisProgress, setAnalysisProgress] = useState<ImageAnalysisProgress | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [nextUp, setNextUp] = useState<MediaSummary | null>(null);
  const [nextPromptVisible, setNextPromptVisible] = useState(false);
  const [nextDismissed, setNextDismissed] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const title = displayTitle(source.detail);
  const resumeSeconds = msToSeconds(source.resumeMs);
  const canResume = resumeSeconds > 20 && !source.detail.completed;
  const nextCountdown = nextUpSecondsRemaining(current, duration);
  const nextProgress = clamp((NEXT_UP_LEAD_SECONDS - nextCountdown) / NEXT_UP_LEAD_SECONDS, 0, 1);
  const nextLabel = source.detail.kind === 'episode' ? 'Siguiente episodio' : 'Película recomendada';
  const nextPosition = nextUp?.kind === 'episode'
    ? `T${nextUp.seasonNumber ?? '?'} E${nextUp.episodeNumber ?? '?'}`
    : nextUp?.year?.toString();

  const imageFilter = useMemo(() => {
    const brightness = 1 + image.brightness / 100;
    const contrast = 1 + image.contrast / 100;
    const saturation = 1 + image.saturation / 100;
    return `url(#cw-tonal-curve) brightness(${brightness}) contrast(${contrast}) saturate(${saturation})`;
  }, [image]);

  const tonalCurve = useMemo(() => tonalCurveTable(image.shadows, image.highlights), [image.shadows, image.highlights]);

  const imageOverlay = useMemo(() => ({
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

  const showControlsTemporarily = useCallback(() => {
    clearControlsTimer();
    setControlsVisible(true);
    if (shouldPinPlayerChrome({
      errorVisible: Boolean(error),
      imagePanelOpen: showImagePanel,
      settingsPanelOpen: showSettingsPanel,
    })) return;
    controlsTimerRef.current = window.setTimeout(() => {
      setControlsVisible(false);
      controlsTimerRef.current = null;
    }, CONTROLS_HIDE_DELAY_MS);
  }, [clearControlsTimer, error, showImagePanel, showSettingsPanel]);

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

  const ensureAudioBoost = useCallback(async () => {
    const existing = audioGraphRef.current;
    if (existing) {
      if (existing.context.state === 'suspended') await existing.context.resume();
      return existing;
    }
    const video = videoRef.current;
    if (!video) throw new Error('El reproductor todavía no está disponible.');
    const context = new AudioContext();
    const sourceNode = context.createMediaElementSource(video);
    const gain = context.createGain();
    const limiter = context.createDynamicsCompressor();
    limiter.threshold.value = -3;
    limiter.knee.value = 8;
    limiter.ratio.value = 4;
    limiter.attack.value = 0.003;
    limiter.release.value = 0.2;
    sourceNode.connect(gain);
    gain.connect(limiter);
    limiter.connect(context.destination);
    const graph = { context, gain, limiter, source: sourceNode };
    audioGraphRef.current = graph;
    video.volume = 1;
    video.muted = false;
    gain.gain.value = playerVolumeRouting(desiredVolumeRef.current, muted, true).amplifiedGain;
    setAudioBoostReady(true);
    if (context.state === 'suspended') await context.resume();
    return graph;
  }, [muted]);

  const applyVolume = useCallback((nextVolume: number, nextMuted = nextVolume === 0) => {
    const next = clamp(nextVolume, 0, PLAYER_MAX_VOLUME);
    desiredVolumeRef.current = next;
    setVolume(next);
    setMuted(nextMuted);
    if (next > 0.5) {
      void ensureAudioBoost().catch(() => {
        desiredVolumeRef.current = 0.5;
        setVolume(0.5);
        setMuted(false);
        setError('No se pudo activar el refuerzo de volumen en este equipo.');
      });
    }
  }, [ensureAudioBoost]);

  const stopFastForward = useCallback(() => {
    if (holdTimerRef.current) {
      window.clearTimeout(holdTimerRef.current);
      holdTimerRef.current = null;
    }
    if (!holdActiveRef.current) return;
    const video = videoRef.current;
    if (video) video.playbackRate = playbackRateBeforeHoldRef.current;
    holdActiveRef.current = false;
    suppressSurfaceClickRef.current = true;
    setFastForward(false);
  }, []);

  const showSeekFeedback = useCallback((seconds: number) => {
    if (seekFeedbackTimerRef.current) window.clearTimeout(seekFeedbackTimerRef.current);
    setSeekFeedback({ seconds, key: Date.now() });
    seekFeedbackTimerRef.current = window.setTimeout(() => {
      setSeekFeedback(null);
      seekFeedbackTimerRef.current = null;
    }, 650);
  }, []);

  const handleSurfacePointerDown = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !event.isPrimary) return;
    pressStartRef.current = { x: event.clientX, y: event.clientY };
    suppressSurfaceClickRef.current = false;
    event.currentTarget.setPointerCapture(event.pointerId);
    if (holdTimerRef.current) window.clearTimeout(holdTimerRef.current);
    holdTimerRef.current = window.setTimeout(() => {
      const video = videoRef.current;
      if (!video || video.paused) return;
      playbackRateBeforeHoldRef.current = video.playbackRate;
      video.playbackRate = 2;
      holdActiveRef.current = true;
      setFastForward(true);
      showControlsTemporarily();
    }, PLAYER_HOLD_DELAY_MS);
  }, [showControlsTemporarily]);

  const handleSurfacePointerMove = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (Math.hypot(event.clientX - pressStartRef.current.x, event.clientY - pressStartRef.current.y) < 12) return;
    if (holdTimerRef.current && !holdActiveRef.current) {
      window.clearTimeout(holdTimerRef.current);
      holdTimerRef.current = null;
    }
  }, []);

  const handleSurfacePointerEnd = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    stopFastForward();
  }, [stopFastForward]);

  const handleSurfaceClick = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (suppressSurfaceClickRef.current) {
      suppressSurfaceClickRef.current = false;
      return;
    }
    if (event.detail > 1) return;
    if (singleClickTimerRef.current) window.clearTimeout(singleClickTimerRef.current);
    singleClickTimerRef.current = window.setTimeout(() => {
      togglePlay();
      singleClickTimerRef.current = null;
    }, SINGLE_CLICK_DELAY_MS);
  }, [togglePlay]);

  const handleSurfaceDoubleClick = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    if (singleClickTimerRef.current) {
      window.clearTimeout(singleClickTimerRef.current);
      singleClickTimerRef.current = null;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    const seconds = seekSecondsForPoint(event.clientX, rect.left, rect.width);
    seekBy(seconds);
    showSeekFeedback(seconds);
  }, [seekBy, showSeekFeedback]);

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
        showControlsTemporarily();
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
        showControlsTemporarily();
      } catch {
        setError('No se pudo activar pantalla completa.');
      }
    })();
  }, [showControlsTemporarily]);

  useEffect(() => {
    const height = source.detail.technical.height;
    const quality = height ? (height >= 2160 ? '4K' : height >= 1080 ? '1080p' : height >= 720 ? '720p' : `${height}p`) : undefined;
    const labels: Record<keyof ImageSettings, string> = {
      brightness: 'Brillo', contrast: 'Contraste', saturation: 'Saturación', shadows: 'Sombras', highlights: 'Luces', temperature: 'Temperatura',
    };
    const snapshot: RemotePlayerSnapshot = {
      active: true,
      mediaId: source.detail.id,
      title,
      year: source.detail.year,
      quality,
      positionSeconds: current,
      durationSeconds: duration,
      playing,
      volume: Math.min(volume, 1),
      muted,
      fullscreen,
      imageAnalyzing: analyzing,
      imageAnalysisPercent: analysisProgress?.percent ?? 0,
      nextUp: nextUp && nextPromptVisible ? {
        id: nextUp.id,
        title: nextUp.title,
        label: nextLabel,
        position: nextPosition,
        secondsRemaining: nextCountdown,
      } : null,
      imageSettings: (Object.keys(image) as Array<keyof ImageSettings>).map(id => ({ id, label: labels[id], value: image[id], min: -50, max: 50, step: 1, defaultValue: 0 })),
      audioTracks: [],
      subtitleTracks: [],
    };
    void invoke('remote_update_player_state', { snapshot }).catch(() => {});
  }, [analysisProgress?.percent, analyzing, current, duration, fullscreen, image, muted, nextCountdown, nextLabel, nextPosition, nextPromptVisible, nextUp, playing, source.detail.id, source.detail.technical.height, source.detail.year, title, volume]);

  useEffect(() => () => {
    const snapshot: RemotePlayerSnapshot = {
      active: false, positionSeconds: 0, durationSeconds: 0, playing: false, volume: 0.8,
      muted: false, fullscreen: false, imageAnalyzing: false, imageAnalysisPercent: 0,
      nextUp: null, imageSettings: [], audioTracks: [], subtitleTracks: [],
    };
    void invoke('remote_update_player_state', { snapshot }).catch(() => {});
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
    if (!nextUp || nextDismissed || nextPromptVisible) return;
    setNextPromptVisible(true);
  }, [nextDismissed, nextPromptVisible, nextUp]);

  const cancelNextPrompt = useCallback(() => {
    setNextPromptVisible(false);
    setNextDismissed(true);
    void saveProgress(true);
  }, [saveProgress]);

  const startNextUp = useCallback(async () => {
    if (!nextUp || nextStartingRef.current) return;
    nextStartingRef.current = true;
    setNextPromptVisible(false);
    setNextDismissed(true);
    try {
      await saveProgress(true);
      await onPlayNext(nextUp.id);
    } finally {
      nextStartingRef.current = false;
    }
  }, [nextUp, onPlayNext, saveProgress]);

  const analyzeImage = useCallback(async () => {
    if (!shouldStartImageAnalysis(Boolean(analysis), imageAnalysisRunningRef.current)) return;
    imageAnalysisRunningRef.current = true;
    const video = videoRef.current;
    const resumeAt = Number.isFinite(video?.currentTime) ? video!.currentTime : current;
    const shouldResume = video ? !video.paused : playing;
    setAnalyzing(true);
    setAnalysisProgress({
      mediaId: source.detail.id,
      running: true,
      processedFrames: 0,
      totalFrames: 0,
      sampledFrames: 0,
      percent: 0,
    });
    setError(null);
    if (video) {
      video.pause();
      setPlaying(false);
      setCurrent(resumeAt);
    }
    try {
      const result = await invoke<ImageAnalysis>('analyze_media_image', { mediaId: source.detail.id });
      setAnalysis(result);
    } catch (cause) {
      setError(`No se pudo analizar la imagen: ${String(cause)}`);
    } finally {
      imageAnalysisRunningRef.current = false;
      setAnalyzing(false);
      setAnalysisProgress(null);
      if (video) {
        if (Math.abs(video.currentTime - resumeAt) > 0.25) {
          video.currentTime = resumeAt;
        }
        setCurrent(resumeAt);
        if (shouldResume) {
          await video.play().catch(() => {});
          setPlaying(!video.paused);
        } else {
          setPlaying(false);
        }
      }
    }
  }, [analysis, current, playing, source.detail.id]);

  const toggleImagePanel = useCallback(() => {
    if (showImagePanel) {
      setShowImagePanel(false);
      return;
    }
    setShowSettingsPanel(false);
    setShowImagePanel(true);
    void analyzeImage();
  }, [analyzeImage, showImagePanel]);

  const toggleSettingsPanel = useCallback(() => {
    setShowImagePanel(false);
    setShowSettingsPanel(value => !value);
  }, []);

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void listen<RemoteCommand>('remote-command', event => {
      const command = event.payload;
      const video = videoRef.current;
      if (!video) return;
      if (command.type === 'player_toggle') togglePlay();
      if (command.type === 'player_seek_by') seekBy(command.seconds);
      if (command.type === 'player_seek_to') {
        video.currentTime = clamp(command.seconds, 0, video.duration || duration || 0);
        setCurrent(video.currentTime);
      }
      if (command.type === 'player_set_volume') {
        applyVolume(clamp(command.volume, 0, 1));
      }
      if (command.type === 'player_toggle_mute') {
        setMuted(value => !value);
      }
      if (command.type === 'player_toggle_fullscreen') toggleFullscreen();
      if (command.type === 'player_start_next_up') void startNextUp();
      if (command.type === 'player_cancel_next_up') cancelNextPrompt();
      if (command.type === 'player_analyze_image') void analyzeImage();
      if (command.type === 'player_set_image' && command.setting_id in defaultImage) {
        setImage(previous => ({ ...previous, [command.setting_id]: clamp(command.value, -50, 50) }));
      }
      if (command.type === 'player_reset_image') setImage(defaultImage);
    }).then(unlisten => {
      if (disposed) unlisten(); else cleanup = unlisten;
    });
    return () => { disposed = true; cleanup?.(); };
  }, [analyzeImage, applyVolume, cancelNextPrompt, duration, seekBy, startNextUp, toggleFullscreen, togglePlay]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const graph = audioGraphRef.current;
    const routing = playerVolumeRouting(volume, muted, Boolean(graph));
    if (graph) {
      video.volume = 1;
      video.muted = false;
      graph.gain.gain.setTargetAtTime(routing.amplifiedGain, graph.context.currentTime, 0.018);
    } else {
      video.volume = routing.nativeVolume;
      video.muted = routing.nativeMuted;
    }
  }, [audioBoostReady, muted, volume]);

  useEffect(() => {
    let cancelled = false;
    let cleanup: (() => void) | null = null;
    void listen<ImageAnalysisProgress>('image-analysis-progress', event => {
      const progress = event.payload;
      if (progress.mediaId !== source.detail.id) return;
      setAnalysisProgress(progress.running ? progress : null);
    }).then(unlisten => {
      if (cancelled) {
        unlisten();
      } else {
        cleanup = unlisten;
      }
    });
    return () => {
      cancelled = true;
      if (cleanup) cleanup();
    };
  }, [source.detail.id]);

  useEffect(() => {
    showControlsTemporarily();
    return clearControlsTimer;
  }, [clearControlsTimer, showControlsTemporarily]);

  useEffect(() => {
    const onMove = (event: MouseEvent) => {
      const last = lastPointerRef.current;
      if (last.ready && Math.abs(event.clientX - last.x) < 2 && Math.abs(event.clientY - last.y) < 2) return;
      lastPointerRef.current = { x: event.clientX, y: event.clientY, ready: true };
      showControlsTemporarily();
    };
    const onFullscreen = () => {
      setFullscreen(Boolean(document.fullscreenElement));
      showControlsTemporarily();
    };
    window.addEventListener('mousemove', onMove);
    document.addEventListener('fullscreenchange', onFullscreen);
    return () => {
      window.removeEventListener('mousemove', onMove);
      document.removeEventListener('fullscreenchange', onFullscreen);
    };
  }, [showControlsTemporarily]);

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
        if (showSettingsPanel) {
          setShowSettingsPanel(false);
        } else if (showImagePanel) {
          setShowImagePanel(false);
        } else if (fullscreen || document.fullscreenElement) {
          toggleFullscreen();
        } else {
          closePlayer();
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [closePlayer, fullscreen, seekBy, showImagePanel, showSettingsPanel, toggleFullscreen, togglePlay]);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) window.clearInterval(saveTimerRef.current);
      if (singleClickTimerRef.current) window.clearTimeout(singleClickTimerRef.current);
      if (holdTimerRef.current) window.clearTimeout(holdTimerRef.current);
      if (seekFeedbackTimerRef.current) window.clearTimeout(seekFeedbackTimerRef.current);
      const graph = audioGraphRef.current;
      if (graph) {
        graph.source.disconnect();
        void graph.context.close();
      }
    };
  }, []);

  useEffect(() => {
    restoredRef.current = false;
    lastSavedAtRef.current = 0;
    nextStartingRef.current = false;
    setPlaying(true);
    setCurrent(0);
    setDuration(msToSeconds(source.detail.runtimeMs ?? source.detail.technical.durationMs ?? 0));
    setError(null);
    setNextUp(null);
    setNextPromptVisible(false);
    setNextDismissed(false);
    setAnalysis(null);
    setAnalysisProgress(null);
    setImage(defaultImage);
    stopFastForward();
    setSeekFeedback(null);
  }, [source.detail.id, source.detail.runtimeMs, source.detail.technical.durationMs, stopFastForward]);

  useEffect(() => {
    let cancelled = false;
    invoke<MediaSummary | null>('next_up', { mediaId: source.detail.id })
      .then(next => {
        if (!cancelled) setNextUp(next);
      })
      .catch(() => {
        if (!cancelled) setNextUp(null);
      });
    return () => { cancelled = true; };
  }, [source.detail.id]);

  useEffect(() => {
    if (shouldOfferNextUp(current, duration, Boolean(nextUp), nextDismissed) && !nextPromptVisible) {
      beginNextPrompt();
    } else if (nextUpSecondsRemaining(current, duration) > NEXT_UP_LEAD_SECONDS && nextPromptVisible) {
      setNextPromptVisible(false);
    }
  }, [beginNextPrompt, current, duration, nextDismissed, nextPromptVisible, nextUp]);

  const progress = duration > 0 ? clamp(current / duration, 0, 1) : 0;

  return (
    <div ref={shellRef} className={`cw-player-shell ${controlsVisible ? '' : 'idle'} ${showImagePanel ? 'image-open' : ''} ${showSettingsPanel ? 'settings-open' : ''}`}>
      <svg className="cw-filter-defs" aria-hidden="true" focusable="false">
        <filter id="cw-tonal-curve" colorInterpolationFilters="sRGB">
          <feComponentTransfer>
            <feFuncR type="table" tableValues={tonalCurve} />
            <feFuncG type="table" tableValues={tonalCurve} />
            <feFuncB type="table" tableValues={tonalCurve} />
          </feComponentTransfer>
        </filter>
      </svg>
      <video
        ref={videoRef}
        crossOrigin="anonymous"
        className="cw-player-video"
        src={source.url}
        autoPlay
        playsInline
        controls={false}
        style={{ filter: imageFilter }}
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
        onEnded={() => {
          setPlaying(false);
          if (shouldAutoplayNextUp(Boolean(nextUp), nextDismissed)) {
            void startNextUp();
          } else {
            void saveProgress(true);
          }
        }}
        onError={() => setError('No se pudo reproducir dentro de CINE WANA. Probá Abrir externo para este archivo.')}
      />
      <div className="cw-player-layer" style={imageOverlay}>
        <span className="temperature" />
      </div>

      {seekFeedback && (
        <div key={seekFeedback.key} className={`cw-seek-feedback ${seekFeedback.seconds > 0 ? 'forward' : 'backward'}`}>
          {seekFeedback.seconds > 0 ? <SkipForward size={28}/> : <SkipBack size={28}/>}
          <b>{seekFeedback.seconds > 0 ? '+' : '−'}10 s</b>
        </div>
      )}
      {fastForward && <div className="cw-speed-feedback"><b>2×</b><span>Reproducción rápida</span></div>}

      {error && <div className="cw-player-error"><span>{error}</span><button onClick={() => setError(null)}><X size={16}/></button></div>}

      {nextUp && nextPromptVisible && (
        <aside className="cw-next-up" onClick={event => event.stopPropagation()}>
          <div className="cw-next-poster">
            {nextUp.artworkUrl ? <img src={convertFileSrc(nextUp.artworkUrl)} alt="" /> : <span>{initials(nextUp.title)}</span>}
          </div>
          <div className="cw-next-copy">
            <small>{nextLabel}</small>
            <b>{nextUp.title}</b>
            <span>{nextPosition ? `${nextPosition} · ` : ''}Empieza al terminar · {Math.ceil(nextCountdown)} s</span>
            <div className="cw-next-meter"><i style={{ width: `${nextProgress * 100}%` }} /></div>
          </div>
          <div className="cw-next-actions">
            <button className="primary" onClick={() => void startNextUp()}><Play fill="currentColor" size={13}/>Reproducir ahora</button>
            <button onClick={cancelNextPrompt}>Cancelar</button>
          </div>
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
          <button className={showSettingsPanel ? 'selected' : ''} onClick={toggleSettingsPanel} title="Abrir configuración"><Settings size={18}/></button>
          <button onClick={closePlayer} title="Cerrar reproductor"><X size={20}/></button>
        </div>
      </div>

      <div
        className="cw-player-center"
        onClick={handleSurfaceClick}
        onDoubleClick={handleSurfaceDoubleClick}
        onPointerDown={handleSurfacePointerDown}
        onPointerMove={handleSurfacePointerMove}
        onPointerUp={handleSurfacePointerEnd}
        onPointerCancel={handleSurfacePointerEnd}
        onContextMenu={event => event.preventDefault()}
      >
        {!playing && <button className="cw-big-play"><Play fill="currentColor" size={34}/></button>}
      </div>

      {showImagePanel && (
        <aside className="cw-image-panel" onClick={event => event.stopPropagation()}>
          <div className="cw-panel-head">
            <div><b>Imagen</b><small>Retoca sólo la reproducción, no modifica el archivo.</small></div>
            <button onClick={() => setShowImagePanel(false)}><X size={16}/></button>
          </div>
          <button className="cw-analyze" onClick={analyzeImage} disabled={analyzing}>
            <Sparkles size={15}/>{analyzing ? 'Analizando escenas…' : analysis ? 'Video ya escaneado' : 'Escanear video'}
          </button>
          {analyzing && (
            <div className="cw-analysis-progress">
              <div><span>Escaneando escenas</span><b>{Math.round(analysisProgress?.percent ?? 0)}%</b></div>
              <i><span style={{ width: `${clamp(analysisProgress?.percent ?? 0, 0, 100)}%` }} /></i>
              <small>{analysisProgress?.totalFrames ? `${analysisProgress.processedFrames}/${analysisProgress.totalFrames} muestras` : 'Preparando muestras'} · video en pausa</small>
            </div>
          )}
          {analysis && (
            <div className="cw-analysis">
              <div><span>Luz media</span><b>{analysis.averageLight}%</b></div>
              <div><span>Sombras</span><b>{analysis.shadowsPercent}%</b></div>
              <div><span>Altas luces</span><b>{analysis.highlightsPercent}%</b></div>
              <div><span>Saturacion</span><b>{analysis.averageSaturation}%</b></div>
              <div><span>Dominante</span><b>{signedNumber(analysis.warmth)}</b></div>
              <small>{analysis.sampledFrames} escenas revisadas con FFmpeg.</small>
              {SHOW_SUGGESTED_IMAGE_ADJUSTMENT && (
                <button onClick={() => setImage(analysis.suggested)}>Aplicar sugerido</button>
              )}
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
          <button className="cw-reset-image" onClick={() => setImage(defaultImage)}>Reset imagen</button>
        </aside>
      )}

      {showSettingsPanel && (
        <aside className="cw-settings-panel" onClick={event => event.stopPropagation()}>
          <div className="cw-panel-head cw-settings-panel-head">
            <div><b>Configuración</b><small>Los mismos ajustes de CINE WANA, sin salir del reproductor.</small></div>
            <button onClick={() => setShowSettingsPanel(false)} title="Cerrar configuración"><X size={16}/></button>
          </div>
          <div className="cw-settings-panel-content">{settingsPanel}</div>
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
            <div
              className={`cw-volume-slider ${volume > 0.5 ? 'boosted' : ''}`}
              style={{
                '--normal-fill': `${Math.min(volume, 0.5) / PLAYER_MAX_VOLUME * 100}%`,
                '--boost-fill': `${Math.max(0, volume - 0.5) / PLAYER_MAX_VOLUME * 100}%`,
              } as React.CSSProperties}
            >
              <span className="cw-volume-track" />
              <span className="cw-volume-normal-fill" />
              <span className="cw-volume-boost-fill" />
              <input
                aria-label="Volumen"
                min={0}
                max={PLAYER_MAX_VOLUME}
                step={0.01}
                type="range"
                value={muted ? 0 : volume}
                title={`${Math.round((muted ? 0 : volume) * 100)}%${volume > 0.5 ? ' · volumen reforzado' : ''}`}
                onChange={event => {
                  const resolved = resolveVolumeWithDetent(
                    Number(event.target.value),
                    volumeDetentReachedAtRef.current,
                    performance.now(),
                  );
                  volumeDetentReachedAtRef.current = resolved.detentReachedAt;
                  applyVolume(resolved.volume);
                }}
              />
            </div>
            <span className={`cw-volume-value ${volume > 0.5 ? 'boosted' : ''}`}>{Math.round((muted ? 0 : volume) * 100)}%</span>
            <button className={showImagePanel ? 'selected' : ''} onClick={toggleImagePanel}><SlidersHorizontal size={19}/><span>Imagen</span></button>
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

function tonalCurveTable(shadows: number, highlights: number) {
  const points = 33;
  return Array.from({ length: points }, (_, index) => {
    const input = index / (points - 1);
    return tonalCurveValue(input, shadows, highlights).toFixed(4);
  }).join(' ');
}

function tonalCurveValue(input: number, shadows: number, highlights: number) {
  let value = input;
  if (shadows > 0) {
    const strength = shadows / 50;
    const influence = 1 - smoothstep(0.1, 0.68, input);
    value += 0.28 * strength * influence * (1 - input * 0.18);
  } else if (shadows < 0) {
    const strength = Math.abs(shadows) / 50;
    const influence = 1 - smoothstep(0.04, 0.62, input);
    value -= 0.34 * strength * influence * Math.sqrt(input);
  }
  if (highlights > 0) {
    const strength = highlights / 50;
    const influence = smoothstep(0.45, 0.96, input);
    value += 0.18 * strength * influence * (1 - input * 0.28);
  } else if (highlights < 0) {
    const strength = Math.abs(highlights) / 50;
    const influence = smoothstep(0.46, 1, input);
    value -= 0.24 * strength * influence * input;
  }
  return clamp(value, 0, 1);
}

function smoothstep(edge0: number, edge1: number, value: number) {
  const t = clamp((value - edge0) / (edge1 - edge0), 0, 1);
  return t * t * (3 - 2 * t);
}

function signedNumber(value: number) {
  return value > 0 ? `+${value}` : value.toString();
}

function formatClock(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0:00';
  const whole = Math.floor(seconds);
  const h = Math.floor(whole / 3600);
  const m = Math.floor((whole % 3600) / 60);
  const s = whole % 60;
  return h > 0 ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}` : `${m}:${s.toString().padStart(2, '0')}`;
}
