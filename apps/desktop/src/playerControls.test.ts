import { describe, expect, it } from 'vitest';
import {
  PLAYER_VOLUME_DETENT_MS,
  playerVolumeRouting,
  resolveVolumeWithDetent,
  seekSecondsForPoint,
  shouldPinPlayerChrome,
  shouldStartImageAnalysis,
} from './playerControls';

describe('player chrome auto-hide', () => {
  it('does not pin controls for a next-up recommendation or a paused video', () => {
    expect(shouldPinPlayerChrome({ errorVisible: false, imagePanelOpen: false, settingsPanelOpen: false })).toBe(false);
  });

  it('pins controls only for errors and open interactive panels', () => {
    expect(shouldPinPlayerChrome({ errorVisible: true, imagePanelOpen: false, settingsPanelOpen: false })).toBe(true);
    expect(shouldPinPlayerChrome({ errorVisible: false, imagePanelOpen: true, settingsPanelOpen: false })).toBe(true);
    expect(shouldPinPlayerChrome({ errorVisible: false, imagePanelOpen: false, settingsPanelOpen: true })).toBe(true);
  });
});

describe('player surface gestures', () => {
  it('seeks backward on the left and forward on the right', () => {
    expect(seekSecondsForPoint(120, 100, 800)).toBe(-10);
    expect(seekSecondsForPoint(899, 100, 800)).toBe(10);
  });
});

describe('image analysis session cache', () => {
  it('analyzes once and reuses the result while the same player session remains open', () => {
    expect(shouldStartImageAnalysis(false, false)).toBe(true);
    expect(shouldStartImageAnalysis(false, true)).toBe(false);
    expect(shouldStartImageAnalysis(true, false)).toBe(false);
  });
});

describe('boosted volume detent', () => {
  it('holds at visible 50% before entering the boosted range', () => {
    const first = resolveVolumeWithDetent(0.7, null, 1_000);
    expect(first).toEqual({ volume: 0.5, detentReachedAt: 1_000 });

    const held = resolveVolumeWithDetent(0.7, first.detentReachedAt, 1_000 + PLAYER_VOLUME_DETENT_MS - 1);
    expect(held.volume).toBe(0.5);

    const released = resolveVolumeWithDetent(0.7, first.detentReachedAt, 1_000 + PLAYER_VOLUME_DETENT_MS);
    expect(released.volume).toBe(0.7);
  });

  it('resets the detent after returning below visible 50%', () => {
    expect(resolveVolumeWithDetent(0.4, 500, 2_000)).toEqual({
      volume: 0.4,
      detentReachedAt: null,
    });
  });
});

describe('player volume routing', () => {
  it.each([
    [0.25, 0.5, 0.5],
    [0.5, 1, 1],
    [0.75, 1, 1.5],
    [1, 1, 2],
  ])('routes visible %s as one amplified signal', (volume, nativeVolume, amplifiedGain) => {
    const routing = playerVolumeRouting(volume, false, true);
    expect(routing.nativeVolume).toBe(nativeVolume);
    expect(routing.nativeMuted).toBe(false);
    expect(routing.amplifiedGain).toBeCloseTo(amplifiedGain);
  });

  it('mutes both native audio and the supplemental boost', () => {
    expect(playerVolumeRouting(1, true, true)).toEqual({
      nativeVolume: 1,
      nativeMuted: true,
      amplifiedGain: 0,
    });
  });

  it('keeps native audio connected if supplemental boost is unavailable', () => {
    expect(playerVolumeRouting(1, false, false)).toEqual({
      nativeVolume: 1,
      nativeMuted: false,
      amplifiedGain: 0,
    });
  });
});
