import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  AudioLines, Captions, Check, Eye, ExternalLink, Info, LoaderCircle, Maximize, Pause, Play, RotateCcw,
  SkipBack, SkipForward, SlidersHorizontal, Volume2, VolumeX, X,
} from 'lucide-react';
import type { MediaSummary, RemoteCommand, RemotePlayerSnapshot } from './types';
import type { InternalPlayerSource } from './InternalPlayer';
import { NEXT_UP_LEAD_SECONDS, nextUpSecondsRemaining, shouldAutoplayNextUp, shouldOfferNextUp } from './playerNextUp';
import {
  PLAYER_HOLD_DELAY_MS,
  PLAYER_MAX_VOLUME,
  resolveVolumeWithDetent,
  seekSecondsForPoint,
  shouldPinPlayerChrome,
} from './playerControls';
import {
  ALL_IMAGE_SLIDERS, ASPECT_CHOICES, DEBAND_SLIDER, LIGHT_SLIDERS, NEUTRAL_IMAGE, ROTATE_CHOICES, SCALING_CHOICES,
  SHADING_SLIDERS, SHARPNESS_SLIDER, SWITCHES, TONE_MAPPING_CHOICES, ZOOM_SLIDER,
  DEFAULT_SUBTITLE_STYLE, SUBTITLE_COLORS, SUBTITLE_FONTS, SUBTITLE_HEIGHT_MAX,
  clampImageValue, formatDelay, isNeutralImage, trackDetail, trackLabel, tracksOf, type SubtitleStyle,
  type EmbeddedState, type ImageChoiceSpec, type ImageSliderSpec, type MpvImage, type MpvImageNumber, type MpvImageSwitch,
} from './embeddedPlayer';

/* Reproductor de CINE WANA con libmpv: mpv dibuja el video en la ventana, por debajo de esta
   interfaz, y acá quedan los mismos controles de siempre más idioma, subtítulos e imagen. */

const CONTROLS_HIDE_DELAY_MS = 2600;
const SINGLE_CLICK_DELAY_MS = 240;
const SEEK_SECONDS = 10;
const STOP_AFTER_CLOSE_MS = 180;

type Panel = 'image' | 'info' | 'tracks' | null;

/* Cuántos reproductores hay montados. React, en desarrollo, monta y desmonta dos veces seguidas:
   mpv solo se frena si después de cerrar no quedó ningún reproductor abierto. */
let mountedPlayers = 0;

function sendCommand(command: Record<string, unknown>) {
  return invoke('embedded_player_command', { command });
}

export function MpvPlayer({
  source,
  onClose,
  onOpenExternal,
  onPlayNext,
  onProgressSaved,
  infoPanel,
}: {
  source: InternalPlayerSource;
  onClose: () => void;
  onOpenExternal: (id: string) => Promise<void>;
  onPlayNext: (id: string) => Promise<void>;
  onProgressSaved: () => void;
  infoPanel: ReactNode;
}) {
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
  const volumeDetentReachedAtRef = useRef<number | null>(null);
  const initialWindowFullscreenRef = useRef<boolean | null>(null);
  const nextStartingRef = useRef(false);
  const minimumSessionRef = useRef(Number.MAX_SAFE_INTEGER);
  const engineRef = useRef<EmbeddedState | null>(null);
  const endedHandledRef = useRef(false);
  const volumeRef = useRef(0.5);
  const mutedRef = useRef(false);
  const imageRef = useRef<MpvImage>(NEUTRAL_IMAGE);
  const [engine, setEngine] = useState<EmbeddedState | null>(null);
  const [scrubSeconds, setScrubSeconds] = useState<number | null>(null);
  const [hover, setHover] = useState<{ x: number; seconds: number } | null>(null);
  const [volume, setVolume] = useState(0.5);
  const [muted, setMuted] = useState(false);
  const [fastForward, setFastForward] = useState(false);
  const [seekFeedback, setSeekFeedback] = useState<{ seconds: number; key: number } | null>(null);
  const [fullscreen, setFullscreen] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [panel, setPanel] = useState<Panel>(null);
  const [image, setImage] = useState<MpvImage>(NEUTRAL_IMAGE);
  const [showOriginal, setShowOriginal] = useState(false);
  const [nextUp, setNextUp] = useState<MediaSummary | null>(null);
  const [nextPromptVisible, setNextPromptVisible] = useState(false);
  const [nextDismissed, setNextDismissed] = useState(false);
  const [error, setError] = useState<string | null>(null);

  volumeRef.current = volume;
  mutedRef.current = muted;
  imageRef.current = image;

  const title = displayTitle(source.detail);
  const knownDuration = msToSeconds(source.detail.runtimeMs ?? source.detail.technical.durationMs ?? 0);
  const duration = engine?.duration && engine.duration > 0 ? engine.duration : knownDuration;
  const current = scrubSeconds ?? engine?.position ?? 0;
  const playing = engine ? !engine.paused && !engine.ended : true;
  const videoReady = Boolean(engine?.videoReady);
  const audioTracks = useMemo(() => tracksOf(engine, 'audio'), [engine]);
  const subtitleTracks = useMemo(() => tracksOf(engine, 'sub'), [engine]);
  const activeSubtitle = subtitleTracks.find(track => track.selected) ?? null;
  const nextCountdown = nextUpSecondsRemaining(current, duration);
  const nextProgress = clamp((NEXT_UP_LEAD_SECONDS - nextCountdown) / NEXT_UP_LEAD_SECONDS, 0, 1);
  const nextLabel = source.detail.kind === 'episode' ? 'Siguiente episodio' : 'Película recomendada';
  const nextPosition = nextUp?.kind === 'episode'
    ? `T${nextUp.seasonNumber ?? '?'} E${nextUp.episodeNumber ?? '?'}`
    : nextUp?.year?.toString();
  const imageChanged = !isNeutralImage(image);

  /* La página se vuelve transparente solo mientras este reproductor está en pantalla (lo resuelve el
     CSS con :has, en el mismo instante en que aparece o desaparece). mpv se frena recién cuando la
     biblioteca ya está dibujada encima: si se frenara antes, por un instante la ventana quedaría
     transparente y se vería lo que hay detrás de CINE WANA. */
  useEffect(() => {
    mountedPlayers += 1;
    return () => {
      mountedPlayers -= 1;
      window.setTimeout(() => {
        if (mountedPlayers > 0) return;
        void invoke('embedded_player_stop').catch(() => {});
      }, STOP_AFTER_CLOSE_MS);
    };
  }, []);

  const saveProgress = useCallback(async (force = false) => {
    const state = engineRef.current;
    const total = state?.duration && state.duration > 0 ? state.duration : knownDuration;
    const position = state?.position ?? 0;
    if (!total || total < 1 || !state?.loaded) return;
    const now = Date.now();
    if (!force && now - lastSavedAtRef.current < 4500) return;
    lastSavedAtRef.current = now;
    await invoke('save_progress', {
      mediaId: source.detail.id,
      positionMs: Math.max(0, Math.round(position * 1000)),
      durationMs: Math.max(0, Math.round(total * 1000)),
    });
    if (force) onProgressSaved();
  }, [knownDuration, onProgressSaved, source.detail.id]);

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
      imagePanelOpen: panel === 'image',
      settingsPanelOpen: panel === 'info' || panel === 'tracks',
    })) return;
    controlsTimerRef.current = window.setTimeout(() => {
      setControlsVisible(false);
      controlsTimerRef.current = null;
    }, CONTROLS_HIDE_DELAY_MS);
  }, [clearControlsTimer, error, panel]);

  const restoreWindowFullscreenMode = useCallback(async () => {
    const initialFullscreen = initialWindowFullscreenRef.current;
    try {
      const appWindow = getCurrentWindow();
      if (initialFullscreen !== null && (await appWindow.isFullscreen()) !== initialFullscreen) {
        await appWindow.setFullscreen(initialFullscreen);
      }
    } catch {
      /* Sin ventana nativa (vista de prueba en el navegador). */
    }
    setFullscreen(Boolean(initialFullscreen));
  }, []);

  /* Cerrar es inmediato: la biblioteca aparece en el acto (opaca) y, por detrás, se guarda el punto
     donde quedaste y la ventana vuelve a su tamaño si estaba en pantalla completa. */
  const closePlayer = useCallback(() => {
    void saveProgress(true).catch(() => {});
    void restoreWindowFullscreenMode();
    onClose();
  }, [onClose, restoreWindowFullscreenMode, saveProgress]);

  const setPaused = useCallback((paused: boolean) => {
    setEngine(previous => previous ? { ...previous, paused } : previous);
    void sendCommand({ type: 'set_pause', paused }).catch(() => {});
  }, []);

  const seekTo = useCallback((seconds: number, fast = false) => {
    const target = clamp(seconds, 0, duration || seconds);
    setEngine(previous => previous ? { ...previous, position: target, ended: false } : previous);
    endedHandledRef.current = false;
    void sendCommand({ type: 'seek_to', seconds: target, fast }).catch(() => {});
  }, [duration]);

  const togglePlay = useCallback(() => {
    const state = engineRef.current;
    if (!state) return;
    if (state.ended) {
      seekTo(0);
      setPaused(false);
      return;
    }
    if (state.paused) {
      setPaused(false);
    } else {
      setPaused(true);
      void saveProgress(true);
    }
  }, [saveProgress, seekTo, setPaused]);

  const seekBy = useCallback((seconds: number) => {
    const state = engineRef.current;
    if (!state) return;
    seekTo(state.position + seconds);
  }, [seekTo]);

  const applyVolume = useCallback((nextVolume: number, nextMuted = nextVolume === 0) => {
    setVolume(clamp(nextVolume, 0, PLAYER_MAX_VOLUME));
    setMuted(nextMuted);
  }, []);

  const stopFastForward = useCallback(() => {
    if (holdTimerRef.current) {
      window.clearTimeout(holdTimerRef.current);
      holdTimerRef.current = null;
    }
    if (!holdActiveRef.current) return;
    void sendCommand({ type: 'set_speed', speed: 1 }).catch(() => {});
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
      const state = engineRef.current;
      if (!state || state.paused || state.ended) return;
      void sendCommand({ type: 'set_speed', speed: 2 }).catch(() => {});
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

  const secondsAtPoint = useCallback((clientX: number, element: HTMLElement) => {
    const rect = element.getBoundingClientRect();
    const percent = clamp((clientX - rect.left) / rect.width, 0, 1);
    return { seconds: percent * (duration || 0), x: clientX - rect.left };
  }, [duration]);

  const toggleFullscreen = useCallback(() => {
    void (async () => {
      try {
        const appWindow = getCurrentWindow();
        const next = !(await appWindow.isFullscreen());
        await appWindow.setFullscreen(next);
        setFullscreen(next);
        showControlsTemporarily();
      } catch {
        setError('No se pudo activar pantalla completa.');
      }
    })();
  }, [showControlsTemporarily]);

  const openExternal = useCallback(async () => {
    setPaused(true);
    await saveProgress(true);
    await restoreWindowFullscreenMode();
    await onOpenExternal(source.detail.id);
  }, [onOpenExternal, restoreWindowFullscreenMode, saveProgress, setPaused, source.detail.id]);

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

  const togglePanel = useCallback((next: Exclude<Panel, null>) => {
    setPanel(currentPanel => currentPanel === next ? null : next);
  }, []);

  const setImageNumber = useCallback((key: MpvImageNumber, value: number) => {
    setImage(previous => ({ ...previous, [key]: clampImageValue(key, value) }));
  }, []);

  const setImageSwitch = useCallback((key: MpvImageSwitch, value: boolean) => {
    setImage(previous => ({ ...previous, [key]: value }));
  }, []);

  const selectAudio = useCallback((id: number) => {
    setEngine(previous => previous ? {
      ...previous,
      tracks: previous.tracks.map(track => track.kind === 'audio' ? { ...track, selected: track.id === id } : track),
    } : previous);
    void sendCommand({ type: 'set_audio_track', id }).catch(cause => setError(`No se pudo cambiar el idioma: ${String(cause)}`));
  }, []);

  const selectSubtitle = useCallback((id: number | null) => {
    setEngine(previous => previous ? {
      ...previous,
      tracks: previous.tracks.map(track => track.kind === 'sub' ? { ...track, selected: track.id === id } : track),
    } : previous);
    void sendCommand({ type: 'set_subtitle_track', id }).catch(cause => setError(`No se pudo cambiar los subtítulos: ${String(cause)}`));
  }, []);

  const setSubtitleScale = useCallback((scale: number) => {
    setEngine(previous => previous ? { ...previous, subtitleScale: scale } : previous);
    void sendCommand({ type: 'set_subtitle_scale', scale }).catch(() => {});
  }, []);

  const subtitleStyle = engine?.subtitleStyle ?? DEFAULT_SUBTITLE_STYLE;
  const setSubtitleStyle = useCallback((change: Partial<SubtitleStyle>) => {
    const current = engineRef.current;
    const next: SubtitleStyle = { ...(current?.subtitleStyle ?? DEFAULT_SUBTITLE_STYLE), ...change };
    if (current) engineRef.current = { ...current, subtitleStyle: next };
    setEngine(previous => previous ? { ...previous, subtitleStyle: next } : previous);
    void sendCommand({ type: 'set_subtitle_style', style: next }).catch(() => {});
  }, []);

  const setSubtitleDelay = useCallback((seconds: number) => {
    const rounded = Math.round(seconds * 10) / 10;
    setEngine(previous => previous ? { ...previous, subtitleDelay: rounded } : previous);
    void sendCommand({ type: 'set_subtitle_delay', seconds: rounded }).catch(() => {});
  }, []);

  /* Abre la película en mpv. Si cambia la película (siguiente episodio) se abre la nueva. */
  useEffect(() => {
    let cancelled = false;
    // Hasta saber el número de la sesión nueva no se acepta ningún aviso (podría ser de la anterior).
    minimumSessionRef.current = Number.MAX_SAFE_INTEGER;
    engineRef.current = null;
    endedHandledRef.current = false;
    nextStartingRef.current = false;
    lastSavedAtRef.current = 0;
    setEngine(null);
    setScrubSeconds(null);
    setError(null);
    setNextUp(null);
    setNextPromptVisible(false);
    setNextDismissed(false);
    setShowOriginal(false);
    setImage(NEUTRAL_IMAGE);
    imageRef.current = NEUTRAL_IMAGE;
    stopFastForward();
    setSeekFeedback(null);
    const resumeSeconds = msToSeconds(source.resumeMs);
    const canResume = resumeSeconds > 20 && !source.detail.completed && (!knownDuration || resumeSeconds < knownDuration * 0.9);
    invoke<number>('embedded_player_open', {
      mediaId: source.detail.id,
      startSeconds: canResume ? resumeSeconds : 0,
      volume: volumeRef.current,
      muted: mutedRef.current,
      image: NEUTRAL_IMAGE,
    }).then(session => {
      if (!cancelled) minimumSessionRef.current = session;
    }).catch(cause => {
      if (!cancelled) setError(`No se pudo abrir la película: ${String(cause)}`);
    });
    return () => { cancelled = true; };
    // Solo cuando cambia la película: el resto de los valores se lee al momento de abrir.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source.detail.id]);

  /* Estado que manda mpv (posición, pausa, pistas…). Se ignoran avisos de una película anterior. */
  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void listen<EmbeddedState>('embedded-player-state', event => {
      const state = event.payload;
      if (state.session < minimumSessionRef.current) return;
      minimumSessionRef.current = state.session;
      engineRef.current = state;
      setEngine(state);
      if (state.error) setError(state.error);
    }).then(unlisten => {
      if (disposed) unlisten(); else cleanup = unlisten;
    });
    return () => { disposed = true; cleanup?.(); };
  }, []);

  useEffect(() => { engineRef.current = engine; }, [engine]);

  useEffect(() => {
    void sendCommand({ type: 'set_volume', volume }).catch(() => {});
  }, [volume]);

  useEffect(() => {
    void sendCommand({ type: 'set_muted', muted }).catch(() => {});
  }, [muted]);

  useEffect(() => {
    void sendCommand({ type: 'set_image', image: showOriginal ? NEUTRAL_IMAGE : image }).catch(() => {});
  }, [image, showOriginal]);

  /* La altura que eligió el usuario; mientras se ven los controles, además, los subtítulos suben para
     que la franja de abajo no los tape. */
  const subtitlePosition = engine?.subtitleStyle?.position ?? DEFAULT_SUBTITLE_STYLE.position;
  useEffect(() => {
    const update = () => {
      const shell = shellRef.current;
      const controls = shell?.querySelector('.cw-player-controls') as HTMLElement | null;
      const chosen = subtitlePosition;
      if (!shell || !controls || !controlsVisible) {
        void sendCommand({ type: 'set_subtitle_position', percent: chosen }).catch(() => {});
        return;
      }
      const covered = controls.getBoundingClientRect().height / Math.max(1, shell.getBoundingClientRect().height);
      const aboveControls = 100 - covered * 100 + 6;
      void sendCommand({ type: 'set_subtitle_position', percent: Math.min(chosen, aboveControls) }).catch(() => {});
    };
    update();
    window.addEventListener('resize', update);
    return () => window.removeEventListener('resize', update);
  }, [controlsVisible, subtitlePosition, videoReady]);

  /* Con un panel abierto, mpv corre el video a la izquierda igual que el reproductor anterior. */
  useEffect(() => {
    const update = () => {
      const width = window.innerWidth || 1;
      const panelWidth = panel ? Math.min(390, width * 0.34) : 0;
      void sendCommand({ type: 'set_video_margins', right: panelWidth / width }).catch(() => {});
    };
    update();
    window.addEventListener('resize', update);
    return () => window.removeEventListener('resize', update);
  }, [panel]);

  useEffect(() => {
    if (saveTimerRef.current) window.clearInterval(saveTimerRef.current);
    saveTimerRef.current = null;
    if (playing && engine?.loaded) {
      saveTimerRef.current = window.setInterval(() => void saveProgress(false), 5000);
    } else if (engine?.loaded) {
      void saveProgress(false);
    }
    return () => {
      if (saveTimerRef.current) window.clearInterval(saveTimerRef.current);
      saveTimerRef.current = null;
    };
  }, [engine?.loaded, playing, saveProgress]);

  useEffect(() => {
    if (!engine?.ended) {
      endedHandledRef.current = false;
      return;
    }
    if (endedHandledRef.current) return;
    endedHandledRef.current = true;
    if (shouldAutoplayNextUp(Boolean(nextUp), nextDismissed)) {
      void startNextUp();
    } else {
      void saveProgress(true);
    }
  }, [engine?.ended, nextDismissed, nextUp, saveProgress, startNextUp]);

  useEffect(() => {
    const height = source.detail.technical.height;
    const quality = height ? (height >= 2160 ? '4K' : height >= 1080 ? '1080p' : height >= 720 ? '720p' : `${height}p`) : undefined;
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
      imageAnalyzing: false,
      imageAnalysisPercent: 0,
      nextUp: nextUp && nextPromptVisible ? {
        id: nextUp.id,
        title: nextUp.title,
        label: nextLabel,
        position: nextPosition,
        secondsRemaining: nextCountdown,
      } : null,
      imageSettings: ALL_IMAGE_SLIDERS.map(spec => ({ id: spec.key, label: spec.label, value: image[spec.key], min: spec.min, max: spec.max, step: 1, defaultValue: 0 })),
      audioTracks: audioTracks.map((track, index) => ({ id: String(track.id), label: trackLabel(track, index + 1), language: track.language ?? null, channels: track.channels ?? null, active: track.selected })),
      subtitleTracks: subtitleTracks.map((track, index) => ({ id: String(track.id), label: trackLabel(track, index + 1), language: track.language ?? null, channels: null, active: track.selected })),
    };
    void invoke('remote_update_player_state', { snapshot }).catch(() => {});
  }, [audioTracks, current, duration, fullscreen, image, muted, nextCountdown, nextLabel, nextPosition, nextPromptVisible, nextUp, playing, source.detail.id, source.detail.technical.height, source.detail.year, subtitleTracks, title, volume]);

  useEffect(() => () => {
    const snapshot: RemotePlayerSnapshot = {
      active: false, positionSeconds: 0, durationSeconds: 0, playing: false, volume: 0.8,
      muted: false, fullscreen: false, imageAnalyzing: false, imageAnalysisPercent: 0,
      nextUp: null, imageSettings: [], audioTracks: [], subtitleTracks: [],
    };
    void invoke('remote_update_player_state', { snapshot }).catch(() => {});
  }, []);

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void listen<RemoteCommand>('remote-command', event => {
      const command = event.payload;
      if (command.type === 'player_toggle') togglePlay();
      if (command.type === 'player_seek_by') seekBy(command.seconds);
      if (command.type === 'player_seek_to') seekTo(command.seconds);
      if (command.type === 'player_set_volume') applyVolume(clamp(command.volume, 0, 1));
      if (command.type === 'player_toggle_mute') setMuted(value => !value);
      if (command.type === 'player_toggle_fullscreen') toggleFullscreen();
      if (command.type === 'player_start_next_up') void startNextUp();
      if (command.type === 'player_cancel_next_up') cancelNextPrompt();
      if (command.type === 'player_set_image') {
        const key = command.setting_id as MpvImageNumber;
        if (ALL_IMAGE_SLIDERS.some(spec => spec.key === key)) {
          setImage(previous => ({ ...previous, [key]: clampImageValue(key, command.value) }));
        }
      }
      if (command.type === 'player_reset_image') setImage(NEUTRAL_IMAGE);
      if (command.type === 'player_set_audio') {
        const id = Number(command.track_id);
        if (Number.isFinite(id)) selectAudio(id);
      }
      if (command.type === 'player_set_subtitle') {
        const id = command.track_id === null ? null : Number(command.track_id);
        if (id === null || Number.isFinite(id)) selectSubtitle(id);
      }
    }).then(unlisten => {
      if (disposed) unlisten(); else cleanup = unlisten;
    });
    return () => { disposed = true; cleanup?.(); };
  }, [applyVolume, cancelNextPrompt, seekBy, seekTo, selectAudio, selectSubtitle, startNextUp, toggleFullscreen, togglePlay]);

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
    window.addEventListener('mousemove', onMove);
    return () => window.removeEventListener('mousemove', onMove);
  }, [showControlsTemporarily]);

  useEffect(() => {
    void (async () => {
      try {
        const isFullscreen = await getCurrentWindow().isFullscreen();
        initialWindowFullscreenRef.current = isFullscreen;
        setFullscreen(isFullscreen);
      } catch {
        initialWindowFullscreenRef.current = false;
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
      // En un campo de texto las teclas escriben. En un deslizador o un interruptor, Escape igual
      // cierra el panel; las flechas siguen moviendo el deslizador.
      const input = event.target instanceof HTMLInputElement ? event.target : null;
      const isControl = input && (input.type === 'range' || input.type === 'checkbox');
      if ((tag === 'INPUT' && !isControl) || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (isControl && event.key !== 'Escape') return;
      if (event.code === 'Space' || event.key === ' ') {
        event.preventDefault();
        // Si quedó marcado un botón (Idioma, Imagen…), el espacio no lo aprieta: solo pausa.
        if (document.activeElement instanceof HTMLButtonElement) document.activeElement.blur();
        if (!event.repeat) togglePlay();
      }
      if (event.key.toLowerCase() === 'j' || event.key === 'ArrowLeft') { event.preventDefault(); seekBy(-SEEK_SECONDS); }
      if (event.key.toLowerCase() === 'l' || event.key === 'ArrowRight') { event.preventDefault(); seekBy(SEEK_SECONDS); }
      if (event.key.toLowerCase() === 'm') { event.preventDefault(); setMuted(value => !value); }
      if (event.key.toLowerCase() === 'f') { event.preventDefault(); toggleFullscreen(); }
      if (event.key === 'Escape') {
        event.preventDefault();
        // La foto ampliada de un actor se cierra sola con su propio Escape.
        if (document.querySelector('.person-viewer')) return;
        if (panel) {
          setPanel(null);
        } else if (fullscreen) {
          toggleFullscreen();
        } else {
          closePlayer();
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [closePlayer, fullscreen, panel, seekBy, toggleFullscreen, togglePlay]);

  useEffect(() => {
    return () => {
      if (singleClickTimerRef.current) window.clearTimeout(singleClickTimerRef.current);
      if (holdTimerRef.current) window.clearTimeout(holdTimerRef.current);
      if (seekFeedbackTimerRef.current) window.clearTimeout(seekFeedbackTimerRef.current);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke<MediaSummary | null>('next_up', { mediaId: source.detail.id })
      .then(next => { if (!cancelled) setNextUp(next); })
      .catch(() => { if (!cancelled) setNextUp(null); });
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
  const sidePanelOpen = panel === 'info' || panel === 'tracks';

  return (
    <div
      ref={shellRef}
      className={`cw-player-shell mpv ${videoReady ? '' : 'mpv-waiting'} ${controlsVisible ? '' : 'idle'} ${panel === 'image' ? 'image-open' : ''} ${sidePanelOpen ? 'settings-open' : ''}`}
    >
      {!videoReady && !error && (
        <div className="cw-mpv-loading"><LoaderCircle className="spin" size={30}/><span>Preparando la película…</span></div>
      )}
      {engine?.buffering && videoReady && (
        <div className="cw-mpv-loading small"><LoaderCircle className="spin" size={22}/><span>Cargando…</span></div>
      )}
      {showOriginal && <div className="cw-original-badge"><Eye size={14}/>Imagen original</div>}

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
          <button className={panel === 'info' ? 'selected' : ''} onClick={() => togglePanel('info')} title={source.detail.kind === 'episode' ? 'Ver datos del episodio' : 'Ver datos de la película'}><Info size={18}/></button>
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
        {engine && !playing && <button className="cw-big-play"><Play fill="currentColor" size={34}/></button>}
      </div>

      {panel === 'image' && (
        <aside className="cw-image-panel" onClick={event => event.stopPropagation()}>
          <div className="cw-panel-head">
            <div><b>Imagen</b><small>Ajustes del reproductor mpv. Retocan solo lo que ves, nunca el archivo.</small></div>
            <button onClick={() => setPanel(null)}><X size={16}/></button>
          </div>
          <div className="cw-image-actions">
            <button
              className={`cw-compare-image ${showOriginal ? 'active' : ''}`}
              disabled={!imageChanged}
              onClick={() => setShowOriginal(value => !value)}
              title="Compará tu ajuste con la imagen original del archivo"
            >
              <Eye size={15}/>{showOriginal ? 'Volver a mis ajustes' : 'Ver original'}
            </button>
            <button className="cw-reset-image" disabled={!imageChanged} onClick={() => { setShowOriginal(false); setImage(NEUTRAL_IMAGE); }}>Reset imagen</button>
          </div>

          <h3 className="cw-panel-section">Luz y color</h3>
          {LIGHT_SLIDERS.map(spec => <ImageSlider key={spec.key} spec={spec} value={image[spec.key]} onChange={setImageNumber}/>)}

          <h3 className="cw-panel-section">Sombreado</h3>
          {SHADING_SLIDERS.map(spec => <ImageSlider key={spec.key} spec={spec} value={image[spec.key]} onChange={setImageNumber}/>)}

          <h3 className="cw-panel-section">Detalle</h3>
          <ImageSlider spec={SHARPNESS_SLIDER} value={image.sharpness} onChange={setImageNumber}/>
          <ImageChoices label="Agrandado de la imagen" choices={SCALING_CHOICES} value={image.scaling} onChange={scaling => setImage(previous => ({ ...previous, scaling }))}/>
          <small className="cw-choice-note">Se nota sobre todo en películas de 720p o menos, agrandadas a tu pantalla. Alta calidad: más detalle, usa más la placa de video. Suave: menos serrucho, ideal para dibujos animados y videos de baja calidad.</small>
          <ImageSwitch spec={SWITCHES.antiring} value={image.antiring} onChange={setImageSwitch}/>

          <h3 className="cw-panel-section">Limpieza</h3>
          <ImageSwitch spec={SWITCHES.deband} value={image.deband} onChange={setImageSwitch}/>
          {image.deband && <ImageSlider spec={DEBAND_SLIDER} value={image.debandStrength} onChange={setImageNumber}/>}
          <ImageSwitch spec={SWITCHES.deinterlace} value={image.deinterlace} onChange={setImageSwitch}/>

          <h3 className="cw-panel-section">Movimiento</h3>
          <ImageSwitch spec={SWITCHES.smoothMotion} value={image.smoothMotion} onChange={setImageSwitch}/>

          <h3 className="cw-panel-section">Encuadre</h3>
          <ImageSwitch spec={SWITCHES.fillScreen} value={image.fillScreen} onChange={setImageSwitch}/>
          <ImageChoices label="Formato" choices={ASPECT_CHOICES} value={image.aspect} onChange={aspect => setImage(previous => ({ ...previous, aspect }))}/>
          <ImageSlider spec={ZOOM_SLIDER} value={image.zoom} onChange={setImageNumber}/>
          <ImageChoices label="Girar" choices={ROTATE_CHOICES} value={image.rotate} onChange={rotate => setImage(previous => ({ ...previous, rotate }))}/>

          {source.detail.technical.hdrType && (
            <>
              <h3 className="cw-panel-section">HDR · {source.detail.technical.hdrType}</h3>
              <ImageChoices label="Pasar a tu pantalla" choices={TONE_MAPPING_CHOICES} value={image.toneMapping} onChange={toneMapping => setImage(previous => ({ ...previous, toneMapping }))}/>
              <ImageSwitch spec={SWITCHES.hdrAutoPeak} value={image.hdrAutoPeak} onChange={setImageSwitch}/>
            </>
          )}
        </aside>
      )}

      {panel === 'tracks' && (
        <aside className="cw-image-panel cw-tracks-panel" onClick={event => event.stopPropagation()}>
          <div className="cw-panel-head">
            <div><b>Idioma y subtítulos</b><small>Lo que trae este archivo. El idioma queda guardado para esta película; el estilo de los subtítulos, para todas.</small></div>
            <button onClick={() => setPanel(null)}><X size={16}/></button>
          </div>
          <h3 className="cw-panel-section"><AudioLines size={14}/>Idioma del audio</h3>
          {audioTracks.length === 0 && <p className="cw-tracks-empty">{engine?.loaded ? 'Este archivo no trae audio.' : 'Leyendo las pistas del archivo…'}</p>}
          {audioTracks.map((track, index) => (
            <button key={`a-${track.id}`} className={`cw-track ${track.selected ? 'selected' : ''}`} onClick={() => selectAudio(track.id)}>
              <span><b>{trackLabel(track, index + 1)}</b><small>{trackDetail(track) || 'Audio'}</small></span>
              {track.selected && <Check size={16}/>}
            </button>
          ))}
          <h3 className="cw-panel-section"><Captions size={14}/>Subtítulos</h3>
          <button className={`cw-track ${activeSubtitle ? '' : 'selected'}`} onClick={() => selectSubtitle(null)}>
            <span><b>Sin subtítulos</b></span>
            {!activeSubtitle && <Check size={16}/>}
          </button>
          {subtitleTracks.map((track, index) => (
            <button key={`s-${track.id}`} className={`cw-track ${track.selected ? 'selected' : ''}`} onClick={() => selectSubtitle(track.id)}>
              <span><b>{trackLabel(track, index + 1)}</b><small>{trackDetail(track) || 'Subtítulo'}</small></span>
              {track.selected && <Check size={16}/>}
            </button>
          ))}
          {engine?.loaded && subtitleTracks.length === 0 && (
            <p className="cw-tracks-empty">Este archivo no trae subtítulos. Si ponés un archivo .srt con el mismo nombre al lado del video, aparece acá.</p>
          )}
          {activeSubtitle && (
            <div className="cw-subtitle-tools">
              <ImageChoices label="Color" choices={SUBTITLE_COLORS} value={subtitleStyle.color} onChange={color => setSubtitleStyle({ color })}/>
              <ImageChoices label="Fuente" choices={SUBTITLE_FONTS} value={subtitleStyle.font} onChange={font => setSubtitleStyle({ font })}/>
              <label className="cw-slider">
                <span>Altura<b>{100 - subtitleStyle.position === 0 ? 'Abajo de todo' : `+${Math.round(100 - subtitleStyle.position)}`}</b></span>
                <input
                  type="range" min={0} max={SUBTITLE_HEIGHT_MAX} step={1} value={Math.round(100 - subtitleStyle.position)}
                  onChange={event => setSubtitleStyle({ position: 100 - Number(event.target.value) })}
                  onDoubleClick={() => setSubtitleStyle({ position: 100 })}
                  title="Doble clic para volver abajo de todo"
                />
              </label>
              <label className="cw-slider">
                <span>Tamaño<b>{Math.round((engine?.subtitleScale ?? 1) * 100)}%</b></span>
                <input type="range" min={0.5} max={2} step={0.05} value={engine?.subtitleScale ?? 1} onChange={event => setSubtitleScale(Number(event.target.value))}/>
              </label>
              <div className="cw-subtitle-sync">
                <span>Sincronización<b>{formatDelay(engine?.subtitleDelay ?? 0)}</b></span>
                <div>
                  <button onClick={() => setSubtitleDelay((engine?.subtitleDelay ?? 0) - 0.5)}>Adelantar 0,5 s</button>
                  <button onClick={() => setSubtitleDelay((engine?.subtitleDelay ?? 0) + 0.5)}>Atrasar 0,5 s</button>
                </div>
                <small>Si el texto aparece antes que la voz, tocá Atrasar. Si aparece después, Adelantar.</small>
                {Math.abs(engine?.subtitleDelay ?? 0) >= 0.05 && <button className="cw-subtitle-reset" onClick={() => setSubtitleDelay(0)}>Volver a 0</button>}
              </div>
            </div>
          )}
        </aside>
      )}

      {panel === 'info' && (
        <aside className="cw-settings-panel" onClick={event => event.stopPropagation()}>
          <div className="cw-panel-head cw-settings-panel-head">
            <div><b>{source.detail.kind === 'episode' ? 'Datos del episodio' : 'Datos de la película'}</b><small>La misma ficha de CINE WANA, sin salir del reproductor.</small></div>
            <button onClick={() => setPanel(null)} title="Cerrar datos"><X size={16}/></button>
          </div>
          <div className="cw-settings-panel-content">{infoPanel}</div>
        </aside>
      )}

      <div className="cw-player-controls">
        <div
          className="cw-progress-hit"
          onPointerMove={event => {
            const track = event.currentTarget.querySelector('.cw-progress-track') as HTMLElement | null;
            if (track) setHover(secondsAtPoint(event.clientX, track));
          }}
          onPointerLeave={() => setHover(null)}
          onPointerDown={event => {
            const track = event.currentTarget.querySelector('.cw-progress-track') as HTMLElement | null;
            if (!track || !duration) return;
            const first = secondsAtPoint(event.clientX, track);
            setScrubSeconds(first.seconds);
            seekTo(first.seconds, true);
            const onMove = (move: PointerEvent) => {
              const point = secondsAtPoint(move.clientX, track);
              setScrubSeconds(point.seconds);
              setHover(point);
              seekTo(point.seconds, true);
            };
            const onUp = (up: PointerEvent) => {
              window.removeEventListener('pointermove', onMove);
              window.removeEventListener('pointerup', onUp);
              const last = secondsAtPoint(up.clientX, track);
              seekTo(last.seconds);
              setScrubSeconds(null);
              void saveProgress(true);
            };
            window.addEventListener('pointermove', onMove);
            window.addEventListener('pointerup', onUp);
          }}
        >
          {hover && duration > 0 && (
            <div className="cw-progress-tooltip" style={{ left: `${hover.x}px` }}>{formatClock(hover.seconds)}</div>
          )}
          <div className="cw-progress-track">
            <i style={{ width: `${progress * 100}%` }} />
            {hover && duration > 0 && <span className="cw-progress-hover" style={{ left: `${hover.x}px` }} />}
          </div>
        </div>

        <div className="cw-controls-row">
          <div className="cw-controls-left">
            <button onClick={() => seekBy(-SEEK_SECONDS)} title="Atrasar 10 segundos"><SkipBack size={20}/></button>
            <button className="cw-play" onClick={togglePlay} title={playing ? 'Pausar' : 'Reproducir'}>{playing ? <Pause fill="currentColor" size={21}/> : <Play fill="currentColor" size={21}/>}</button>
            <button onClick={() => seekBy(SEEK_SECONDS)} title="Adelantar 10 segundos"><SkipForward size={20}/></button>
            <button onClick={() => { seekTo(0); setPaused(false); }} title="Volver al principio"><RotateCcw size={18}/></button>
            <span className="cw-time">{formatClock(current)} / {formatClock(duration)}</span>
          </div>
          <div className="cw-controls-right">
            <button onClick={() => setMuted(value => !value)} title={muted ? 'Activar sonido' : 'Silenciar'}>{muted || volume === 0 ? <VolumeX size={19}/> : <Volume2 size={19}/>}</button>
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
                  const resolved = resolveVolumeWithDetent(Number(event.target.value), volumeDetentReachedAtRef.current, performance.now());
                  volumeDetentReachedAtRef.current = resolved.detentReachedAt;
                  applyVolume(resolved.volume);
                }}
              />
            </div>
            <span className={`cw-volume-value ${volume > 0.5 ? 'boosted' : ''}`}>{Math.round((muted ? 0 : volume) * 100)}%</span>
            <button className={panel === 'tracks' ? 'selected' : ''} onClick={() => togglePanel('tracks')} title="Elegir idioma y subtítulos"><Captions size={19}/><span>Idioma</span></button>
            <button className={panel === 'image' ? 'selected' : ''} onClick={() => togglePanel('image')}><SlidersHorizontal size={19}/><span>Imagen</span></button>
            <button className={fullscreen ? 'selected' : ''} onClick={toggleFullscreen} title="Pantalla completa"><Maximize size={19}/></button>
          </div>
        </div>
      </div>
    </div>
  );
}

function ImageSlider({ spec, value, onChange }: { spec: ImageSliderSpec; value: number; onChange: (key: MpvImageNumber, value: number) => void }) {
  return (
    <label className="cw-slider">
      <span>{spec.label}<b>{value}{spec.unit ?? ''}</b></span>
      <input
        min={spec.min} max={spec.max} step={1} type="range" value={value}
        onChange={event => onChange(spec.key, Number(event.target.value))}
        onDoubleClick={() => onChange(spec.key, 0)}
        title="Doble clic para volver a 0"
      />
    </label>
  );
}

function ImageSwitch({ spec, value, onChange }: { spec: { key: MpvImageSwitch; label: string; detail: string }; value: boolean; onChange: (key: MpvImageSwitch, value: boolean) => void }) {
  return (
    <label className="cw-switch">
      <input type="checkbox" checked={value} onChange={event => onChange(spec.key, event.target.checked)}/>
      <span className="cw-switch-mark" aria-hidden="true"><i/></span>
      <span className="cw-switch-copy"><b>{spec.label}</b><small>{spec.detail}</small></span>
    </label>
  );
}

function ImageChoices<T extends string | number>({ label, choices, value, onChange }: { label: string; choices: ImageChoiceSpec<T>[]; value: T; onChange: (value: T) => void }) {
  return (
    <div className="cw-choice">
      <span>{label}</span>
      <div role="radiogroup" aria-label={label}>
        {choices.map(choice => (
          <button key={String(choice.value)} role="radio" aria-checked={choice.value === value} className={choice.value === value ? 'selected' : ''} onClick={() => onChange(choice.value)}>{choice.label}</button>
        ))}
      </div>
    </div>
  );
}

function displayTitle(item: InternalPlayerSource['detail']) {
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
