export type ConnectionState = 'unpaired' | 'pairing' | 'connected' | 'reconnecting' | 'disconnected';
export type View = 'home' | 'movies' | 'series' | 'watchlist' | 'search';

export interface ImageSetting {
  id: string; label: string; value: number; min: number; max: number; step: number; defaultValue: number;
}

export interface Track { id: string; label: string; language?: string; channels?: number; active: boolean }

export interface PlayerSnapshot {
  active: boolean; mediaId?: string; title?: string; year?: number; quality?: string;
  positionSeconds: number; durationSeconds: number; playing: boolean; volume: number; muted: boolean; fullscreen: boolean;
  imageSettings: ImageSetting[]; audioTracks: Track[]; subtitleTracks: Track[];
}

export interface MediaItem {
  id: string; kind: 'movie' | 'episode'; title: string; year?: number; seriesTitle?: string;
  seasonNumber?: number; episodeNumber?: number; progressPercent: number; favorite: boolean; inWatchlist: boolean;
  completed: boolean; artworkAvailable: boolean; backdropAvailable: boolean; overview?: string; durationMs?: number; quality?: string;
}

export interface MediaDetail extends MediaItem { overview?: string; genres: string[]; runtimeMs?: number; tracks: Track[] }

export interface SeriesSeason {
  seasonNumber: number; title: string; episodes: MediaItem[];
}

export interface SeriesItem {
  episodeId: string; title: string; seasons: number; episodes: number; artworkAvailable: boolean; seasonItems: SeriesSeason[];
}

export type RemoteCommand =
  | { type: 'player_toggle' }
  | { type: 'player_seek_by'; seconds: number }
  | { type: 'player_seek_to'; seconds: number }
  | { type: 'player_set_volume'; volume: number }
  | { type: 'player_toggle_mute' }
  | { type: 'player_toggle_fullscreen' }
  | { type: 'player_set_image'; setting_id: string; value: number }
  | { type: 'player_reset_image' }
  | { type: 'player_set_audio'; track_id: string }
  | { type: 'player_set_subtitle'; track_id: string | null }
  | { type: 'library_play_media'; media_id: string }
  | { type: 'library_refresh' }
  | { type: 'library_set_flag'; media_id: string; flag: 'favorite' | 'watchlist'; value: boolean };

export interface StoredCredentials { deviceId: string; token: string }
