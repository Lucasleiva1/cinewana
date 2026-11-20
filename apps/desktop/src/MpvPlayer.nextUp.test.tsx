// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MpvPlayer } from './MpvPlayer';
import type { MediaDetail, MediaSummary } from './types';
import { DEFAULT_SUBTITLE_STYLE, type EmbeddedState } from './embeddedPlayer';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listeners: new Map<string, (event: { payload: EmbeddedState }) => void>(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke, convertFileSrc: (s: string) => s }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: EmbeddedState }) => void) => {
    mocks.listeners.set(name, callback);
    return () => { mocks.listeners.delete(name); };
  }),
}));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ isFullscreen: async () => false, setFullscreen: async () => {} }),
}));

const movie: MediaDetail = {
  id: 'movie-1', kind: 'movie', title: 'Película actual', technical: {},
  progressPercent: 0, favorite: false, inWatchlist: false, completed: false,
  offline: false, addedAt: '', categories: [], incomplete: false,
  genres: ['Acción'], cast: [], tracks: [], fileName: '', manualMetadata: false,
  metadataStatus: '', metadataCandidates: [], recommendations: [], people: [],
};
const next: MediaSummary = { ...movie, id: 'movie-2', title: 'Otra de acción' };
const engine: EmbeddedState = {
  session: 1, loaded: true, videoReady: true, position: 0, duration: 600,
  paused: false, ended: false, buffering: false, volume: 0.5, muted: false,
  speed: 1, subtitleScale: 1, subtitleDelay: 0, subtitleStyle: DEFAULT_SUBTITLE_STYLE, tracks: [],
};

async function mount() {
  const onPlayNext = vi.fn(async () => {});
  render(<MpvPlayer source={{ detail: movie, url: '', resumeMs: 0 }}
    onClose={vi.fn()} onOpenExternal={vi.fn()} onPlayNext={onPlayNext}
    onProgressSaved={vi.fn()} infoPanel={null} />);
  await act(async () => {});
  return onPlayNext;
}

async function emit(position: number, ended = false) {
  await act(async () => {
    mocks.listeners.get('embedded-player-state')?.({ payload: { ...engine, position, ended } });
  });
}

beforeEach(() => {
  mocks.listeners.clear();
  mocks.invoke.mockReset();
  mocks.invoke.mockImplementation(async (command: string) => {
    if (command === 'embedded_player_open') return 1;
    if (command === 'next_up') return next;
  });
});
afterEach(cleanup);

describe('native player next-up flow', () => {
  it('shows the recommendation at two minutes, counts down and advances only at EOF once', async () => {
    const playNext = await mount();
    await emit(479);
    expect(screen.queryByText('Otra de acción')).toBeNull();
    await emit(480);
    expect(screen.getByText('Otra de acción')).toBeTruthy();
    expect(screen.getByText(/Empieza al terminar · 120 s/)).toBeTruthy();
    await emit(599);
    expect(screen.getByText(/Empieza al terminar · 1 s/)).toBeTruthy();
    expect(playNext).not.toHaveBeenCalled();
    await emit(600, true);
    await waitFor(() => expect(playNext).toHaveBeenCalledExactlyOnceWith('movie-2'));
    await emit(600, true);
    expect(playNext).toHaveBeenCalledTimes(1);
  });

  it('hides the offer on rewind and cancels autoplay for the current film', async () => {
    const playNext = await mount();
    await emit(490);
    await emit(470);
    expect(screen.queryByText('Otra de acción')).toBeNull();
    await emit(490);
    fireEvent.click(screen.getByRole('button', { name: 'Cancelar' }));
    expect(screen.queryByText('Otra de acción')).toBeNull();
    await emit(600, true);
    expect(playNext).not.toHaveBeenCalled();
  });

  it('advances when a pending recommendation resolves after EOF', async () => {
    let resolveNext!: (value: MediaSummary) => void;
    const pendingNext = new Promise<MediaSummary>(resolve => { resolveNext = resolve; });
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'embedded_player_open') return 1;
      if (command === 'next_up') return pendingNext;
    });
    const playNext = await mount();
    await emit(600, true);
    expect(playNext).not.toHaveBeenCalled();
    await act(async () => { resolveNext(next); });
    await waitFor(() => expect(playNext).toHaveBeenCalledExactlyOnceWith('movie-2'));
  });
});
