import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { NowPlaying } from './App';
import type { PlayerSnapshot } from './types';

const basePlayer: PlayerSnapshot = {
  active: false,
  positionSeconds: 0,
  durationSeconds: 0,
  playing: false,
  volume: 0.8,
  muted: false,
  fullscreen: false,
  imageAnalyzing: false,
  imageAnalysisPercent: 0,
  nextUp: null,
  imageSettings: [],
  audioTracks: [],
  subtitleTracks: [],
};

function renderPlayer(player: PlayerSnapshot) {
  return renderToStaticMarkup(
    <NowPlaying
      player={player}
      send={() => true}
      openSheet={() => undefined}
      openImage={() => undefined}
    />,
  );
}

describe('remote player layout', () => {
  it('keeps the same controls mounted before and after playback starts', () => {
    const inactive = renderPlayer(basePlayer);
    const active = renderPlayer({
      ...basePlayer,
      active: true,
      title: 'Película de prueba',
      durationSeconds: 7200,
      volume: 0.55,
      imageSettings: [{ id: 'brightness', label: 'Brillo', value: 0, min: -1, max: 1, step: 0.1, defaultValue: 0 }],
      audioTracks: [{ id: 'audio-1', label: 'Español', active: true }],
      subtitleTracks: [{ id: 'subtitle-1', label: 'Español', active: true }],
    });

    expect((inactive.match(/type="range"/g) ?? []).length).toBe(2);
    expect((active.match(/type="range"/g) ?? []).length).toBe(2);
    expect((inactive.match(/<button/g) ?? []).length).toBe(8);
    expect((active.match(/<button/g) ?? []).length).toBe(8);
    expect(inactive).toContain('aria-label="Volumen"');
    expect(active).toContain('aria-label="Volumen"');
  });

  it('places changing next-content information below the fixed volume and quick controls', () => {
    const markup = renderPlayer({
      ...basePlayer,
      active: true,
      title: 'Película de prueba',
      nextUp: { id: 'next', title: 'Siguiente', label: 'A continuación', secondsRemaining: 30 },
    });

    expect(markup.indexOf('class="volume"')).toBeLessThan(markup.indexOf('class="quick-controls"'));
    expect(markup.indexOf('class="quick-controls"')).toBeLessThan(markup.indexOf('class="remote-next-up"'));
  });
});
