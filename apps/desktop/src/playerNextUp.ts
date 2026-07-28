export const NEXT_UP_LEAD_SECONDS = 60;

export function nextUpSecondsRemaining(current: number, duration: number) {
  if (!Number.isFinite(current) || !Number.isFinite(duration) || duration <= 0) return 0;
  return Math.max(0, duration - current);
}

export function shouldOfferNextUp(current: number, duration: number, hasCandidate: boolean, dismissed: boolean) {
  if (!hasCandidate || dismissed || duration <= 0) return false;
  const remaining = duration - current;
  return remaining >= 0 && remaining <= NEXT_UP_LEAD_SECONDS;
}

export function shouldAutoplayNextUp(hasCandidate: boolean, dismissed: boolean) {
  return hasCandidate && !dismissed;
}
