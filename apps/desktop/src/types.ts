export type MediaKind = 'movie' | 'episode';
export type RootStatus = 'online' | 'disconnected' | 'scanning' | 'error';

export interface MediaTechnical { durationMs?: number; width?: number; height?: number; container?: string; videoCodec?: string; audioCodec?: string; hdrType?: string }
export interface MediaSummary {
  id: string; kind: MediaKind; title: string; year?: number; seriesTitle?: string; seasonNumber?: number; episodeNumber?: number;
  progressPercent: number; favorite: boolean; inWatchlist: boolean; completed: boolean; offline: boolean; addedAt: string;
  artworkUrl?: string; backdropUrl?: string; previewUrl?: string; technical: MediaTechnical;
}
export interface SeriesSummary { title: string; seasons: number; episodes: number; artworkUrl?: string; latestAddedAt: string }
export interface HomeDto { heroes: MediaSummary[]; continueWatching: MediaSummary[]; recentlyAdded: MediaSummary[]; movies: MediaSummary[]; series: SeriesSummary[]; favorites: MediaSummary[] }
export interface LibraryRoot { id:string; displayName:string; enabled:boolean; recursive:boolean; watchEnabled:boolean; status:RootStatus; lastScanAt?:string; disconnectedCount:number; localPath?:string }
export interface Account { id:string; name:string }
export interface ScanProgress { jobId?:string; running:boolean; cancelRequested:boolean; found:number; processed:number; skipped:number; errors:number; currentFile?:string; percent:number; message?:string }
export interface Bootstrap { roots:LibraryRoot[]; scan:ScanProgress; home:HomeDto; accounts:Account[]; activeAccount?:Account|null; ffprobeAvailable:boolean; playerAvailable:boolean }
export interface MediaTrack { id:string; trackType:string; streamIndex:number; language?:string; title?:string; codec?:string; channels?:number; defaultTrack:boolean; forcedTrack:boolean; external:boolean }
export interface MediaMetadataCandidate {
  id:string; language:string; pageId:number; title:string; year?:number|null; description?:string|null; sourceUrl:string;
}
export interface MediaDetail extends MediaSummary {
  overview?:string; genres:string[]; cast:string[]; runtimeMs?:number; tracks:MediaTrack[]; fileName:string;
  manualMetadata:boolean; metadataStatus:string; metadataSourceUrl?:string|null; metadataImportedAt?:string|null;
  metadataCandidates:MediaMetadataCandidate[]; recommendations:MediaSummary[];
}
export interface MediaMetadataUpdate {
  title:string; year?:number|null; overview?:string|null; genres:string[]; cast:string[]; posterPath?:string|null; backdropPath?:string|null;
}
