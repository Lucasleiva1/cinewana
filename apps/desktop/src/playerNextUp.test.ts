import { describe, expect, it } from 'vitest';
import { nextUpSecondsRemaining, shouldAutoplayNextUp, shouldOfferNextUp } from './playerNextUp';

describe('next-up playback policy', () => {
  it('opens the offer at 30 seconds and counts down to the real ending', () => {
    expect(shouldOfferNextUp(69.9, 100, true, false)).toBe(false);
    expect(shouldOfferNextUp(70, 100, true, false)).toBe(true);
    expect(nextUpSecondsRemaining(82.5, 100)).toBe(17.5);
    expect(nextUpSecondsRemaining(100, 100)).toBe(0);
  });

  it('keeps manual play available while cancellation disables autoplay', () => {
    const hasCandidate = true;
    expect(shouldAutoplayNextUp(hasCandidate, false)).toBe(true);
    expect(shouldAutoplayNextUp(hasCandidate, true)).toBe(false);
    expect(hasCandidate).toBe(true);
  });
});
