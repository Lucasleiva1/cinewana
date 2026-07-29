import { describe, expect, it } from 'vitest';
import {
  PLAYER_VOLUME_DETENT_MS,
  resolveVolumeWithDetent,
  seekSecondsForPoint,
} from './playerControls';

describe('player surface gestures', () => {
  it('seeks backward on the left and forward on the right', () => {
    expect(seekSecondsForPoint(120, 100, 800)).toBe(-10);
    expect(seekSecondsForPoint(899, 100, 800)).toBe(10);
  });
});

describe('boosted volume detent', () => {
  it('holds at 100% before entering the boosted range', () => {
    const first = resolveVolumeWithDetent(1.2, null, 1_000);
    expect(first).toEqual({ volume: 1, detentReachedAt: 1_000 });

    const held = resolveVolumeWithDetent(1.2, first.detentReachedAt, 1_000 + PLAYER_VOLUME_DETENT_MS - 1);
    expect(held.volume).toBe(1);

    const released = resolveVolumeWithDetent(1.2, first.detentReachedAt, 1_000 + PLAYER_VOLUME_DETENT_MS);
    expect(released.volume).toBe(1.2);
  });

  it('resets the detent after returning below 100%', () => {
    expect(resolveVolumeWithDetent(0.75, 500, 2_000)).toEqual({
      volume: 0.75,
      detentReachedAt: null,
    });
  });
});
