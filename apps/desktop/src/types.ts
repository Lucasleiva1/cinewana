export type MediaKind = 'movie' | 'episode';
export type RootStatus = 'online' | 'disconnected' | 'scanning' | 'error';

export interface MediaTechnical { durationMs?: number; width?: number; height?: number; container?: string; videoCodec?: string; audioCodec?: string; hdrType?: string }
export interface ImageSettings { brightness: number; contrast: number; saturation: number; shadows: number; highlights: number; temperature: number }
export interface ImageAnalysis {
  averageLight: number; shadowsPercent: number; highlightsPercent: number; averageSaturation: number; warmth: number;
  sampledFrames: number; suggested: ImageSettings;
}
export interface ImageAnalysisProgress {
  mediaId: string; running: boolean; processedFrames: number; totalFrames: number; sampledFrames: number; percent: number;
}
export interface MediaSummary {
  id: string; kind: MediaKind; title: string; year?: number; seriesTitle?: string; seasonNumber?: number; episodeNumber?: number;
  overview?:string;
  progressPercent: number; favorite: boolean; inWatchlist: boolean; completed: boolean; offline: boolean; addedAt: string;
  artworkUrl?: string; backdropUrl?: string; previewUrl?: string; technical: MediaTechnical;
  categories: string[]; incomplete: boolean; sagaId?: string; sagaTitle?: string; sagaPosition?: number;
}
export type CategoryKind = 'movies' | 'series' | 'sagas' | 'custom' | 'uncategorized';
export interface CustomCategory { id:string; label:string; items:string[]; series:string[] }
export interface SagaSummary { id:string; title:string; artworkUrl?:string; items:MediaSummary[] }
export interface CategoryRow {
  id:string; label:string; kind:CategoryKind; count:number;
  items?:MediaSummary[]; series?:SeriesSummary[]; sagas?:SagaSummary[];
}
export interface CategoryOption { id:string; label:string; kind:CategoryKind; count:number; hidden:boolean }
export interface CategoryPreference { id:string; hidden:boolean }
export interface SeriesSeasonSummary { seasonNumber:number; title:string; episodes:MediaSummary[] }
export interface SeriesSummary { episodeId: string; title: string; seasons: number; episodes: number; artworkUrl?: string; latestAddedAt: string; seasonItems:SeriesSeasonSummary[] }
export interface HomeDto {
  heroes: MediaSummary[]; continueWatching: MediaSummary[]; recentlyAdded: MediaSummary[]; movies: MediaSummary[];
  series: SeriesSummary[]; favorites: MediaSummary[]; categories: CategoryRow[]; categorySettings: CategoryOption[];
  categoryStyle: CategoryStyle; customCategories: CustomCategory[];
}
export type CategoryStyle = 'gold' | 'dark';
export interface LibraryRoot { id:string; displayName:string; enabled:boolean; recursive:boolean; watchEnabled:boolean; status:RootStatus; lastScanAt?:string; disconnectedCount:number; localPath?:string }
export interface Account { id:string; name:string }
export interface ScanProgress { jobId?:string; running:boolean; cancelRequested:boolean; found:number; processed:number; skipped:number; errors:number; currentFile?:string; percent:number; message?:string }
export interface IdentificationReview { mediaId:string; fileName:string; kind:MediaKind; title:string; seriesTitle?:string; seasonNumber?:number; episodeNumber?:number; reason:string; identificationPending:boolean; metadataStatus:string; metadataCandidates:MediaMetadataCandidate[] }
export interface ClassificationUpdate { kind:MediaKind; title:string; seriesTitle?:string|null; seasonNumber?:number|null; episodeNumber?:number|null }
export interface Bootstrap { roots:LibraryRoot[]; scan:ScanProgress; home:HomeDto; accounts:Account[]; activeAccount?:Account|null; ffprobeAvailable:boolean; playerAvailable:boolean; identificationReviews:IdentificationReview[] }
export interface MediaTrack { id:string; trackType:string; streamIndex:number; language?:string; title?:string; codec?:string; channels?:number; defaultTrack:boolean; forcedTrack:boolean; external:boolean }
export interface MediaMetadataCandidate {
  id:string; language:string; pageId:number; title:string; year?:number|null; description?:string|null; sourceUrl:string; posterUrl?:string|null;
}
export interface MediaDetail extends MediaSummary {
  overview?:string; genres:string[]; cast:string[]; runtimeMs?:number; tracks:MediaTrack[]; fileName:string;
  manualMetadata:boolean; metadataStatus:string; metadataSourceUrl?:string|null; metadataImportedAt?:string|null;
  metadataCandidates:MediaMetadataCandidate[]; recommendations:MediaSummary[];
}
export interface MediaMetadataUpdate {
  title:string; year?:number|null; overview?:string|null; genres:string[]; cast:string[]; posterPath?:string|null; backdropPath?:string|null;
}

export interface RemoteDevice { id:string; name:string; createdAt:string; lastSeenAt?:string|null }
export interface PendingRemotePairing { id:string; deviceName:string; requestedAt:string }
export interface RemotePairing { url:string; code:string; expiresAt:string; qrDataUrl:string }
export interface RemoteStatus {
  enabled:boolean; autoStart:boolean; computerName:string; address:string; port:number; url?:string|null; secureContext:boolean;
  pairing?:RemotePairing|null; devices:RemoteDevice[]; pending:PendingRemotePairing[]; lastConnectedAt?:string|null;
  assetRootReady:boolean; error?:string|null;
}

export interface RemoteImageSetting { id:string; label:string; value:number; min:number; max:number; step:number; defaultValue:number }
export interface RemoteTrack { id:string; label:string; language?:string|null; channels?:number|null; active:boolean }
export interface RemoteNextUp { id:string; title:string; label:string; position?:string|null; secondsRemaining:number }
export interface RemotePlayerSnapshot {
  active:boolean; mediaId?:string|null; title?:string|null; year?:number|null; quality?:string|null;
  positionSeconds:number; durationSeconds:number; playing:boolean; volume:number; muted:boolean; fullscreen:boolean;
  imageAnalyzing:boolean; imageAnalysisPercent:number;
  nextUp?:RemoteNextUp|null;
  imageSettings:RemoteImageSetting[]; audioTracks:RemoteTrack[]; subtitleTracks:RemoteTrack[];
}

export type RemoteCommand =
  | {type:'player_toggle'} | {type:'player_seek_by';seconds:number} | {type:'player_seek_to';seconds:number}
  | {type:'player_set_volume';volume:number} | {type:'player_toggle_mute'} | {type:'player_toggle_fullscreen'}
  | {type:'player_start_next_up'} | {type:'player_cancel_next_up'}
  | {type:'player_analyze_image'} | {type:'player_set_image';setting_id:string;value:number} | {type:'player_reset_image'}
  | {type:'player_set_audio';track_id:string} | {type:'player_set_subtitle';track_id:string|null}
  | {type:'library_play_media';media_id:string} | {type:'navigate';direction:string} | {type:'navigate_back'};
