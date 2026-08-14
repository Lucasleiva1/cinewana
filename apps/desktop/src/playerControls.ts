export const PLAYER_SEEK_SECONDS = 10;
export const PLAYER_HOLD_DELAY_MS = 450;
export const PLAYER_MAX_VOLUME = 1;
export const PLAYER_VOLUME_DETENT = 0.5;
export const PLAYER_VOLUME_DETENT_MS = 420;

export interface PlayerChromeState {
  errorVisible: boolean;
  imagePanelOpen: boolean;
  settingsPanelOpen: boolean;
}

export function shouldPinPlayerChrome(state: PlayerChromeState) {
  return state.errorVisible || state.imagePanelOpen || state.settingsPanelOpen;
}

export function seekSecondsForPoint(clientX: number, left: number, width: number) {
  const direction = clientX < left + width / 2 ? -1 : 1;
  return direction * PLAYER_SEEK_SECONDS;
}

export function resolveVolumeWithDetent(
  requestedVolume: number,
  detentReachedAt: number | null,
  now: number,
) {
  const volume = clamp(requestedVolume, 0, PLAYER_MAX_VOLUME);
  if (volume < PLAYER_VOLUME_DETENT) {
    return { volume, detentReachedAt: null };
  }
  if (volume === PLAYER_VOLUME_DETENT) {
    return { volume, detentReachedAt: detentReachedAt ?? now };
  }
  const reachedAt = detentReachedAt ?? now;
  if (now - reachedAt < PLAYER_VOLUME_DETENT_MS) {
    return { volume: PLAYER_VOLUME_DETENT, detentReachedAt: reachedAt };
  }
  return { volume, detentReachedAt: reachedAt };
}

export function playerVolumeRouting(volume: number, muted: boolean, boostReady: boolean) {
  const normalized = clamp(volume, 0, PLAYER_MAX_VOLUME);
  return {
    nativeVolume: Math.min(normalized / PLAYER_VOLUME_DETENT, 1),
    nativeMuted: muted || normalized === 0,
    amplifiedGain: boostReady && !muted
      ? normalized / PLAYER_VOLUME_DETENT
      : 0,
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}
