import { mockConvertFileSrc, mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import type { Bootstrap, HomeDto, ImageAnalysis, MediaDetail, MediaSummary, ScanProgress, SeriesSummary } from './types';

const now = new Date().toISOString();
const scan: ScanProgress = { running:false, cancelRequested:false, found:16, processed:16, skipped:0, errors:0, percent:100 };

const movie = (id:string,title:string,year:number,progressPercent=0): MediaSummary => ({
  id,
  kind:'movie',
  title,
  year,
  progressPercent,
  favorite:false,
  inWatchlist:false,
  completed:false,
  offline:false,
  addedAt:now,
  technical:{ height:1080, durationMs:7200000 }
});

const episode = (id:string,seriesTitle:string,seasonNumber:number,episodeNumber:number): MediaSummary => ({
  id,
  kind:'episode',
  title:`Episodio ${episodeNumber}`,
  seriesTitle,
  seasonNumber,
  episodeNumber,
  year:2022,
  progressPercent:0,
  favorite:false,
  inWatchlist:false,
  completed:false,
  offline:false,
  addedAt:now,
  technical:{ height:1080, durationMs:3600000 }
});

const movies = [
  movie('movie-1','Twilight',2008,42),
  movie('movie-2','The Twilight Saga New Moon',2009),
  movie('movie-3','The Twilight Saga Eclipse',2010),
  movie('movie-4','The Twilight Saga Breaking Dawn Part 2',2012),
  movie('movie-5','The Matrix Resurrections',2021),
  movie('movie-6','Interstellar',2014),
  movie('movie-7','Dune Part Two',2024)
];

const episodes = [
  episode('episode-dragon-1','La Casa Del Dragon',1,1),
  episode('episode-last-1','The Last Of Us',1,1),
  episode('episode-bear-1','The Bear',1,1)
];

const series: SeriesSummary[] = [
  { episodeId:'episode-dragon-1', title:'La Casa Del Dragon', seasons:1, episodes:2, latestAddedAt:now },
  { episodeId:'episode-last-1', title:'The Last Of Us', seasons:1, episodes:3, latestAddedAt:now },
  { episodeId:'episode-bear-1', title:'The Bear', seasons:1, episodes:4, latestAddedAt:now }
];

const home: HomeDto = {
  heroes:movies.slice(0,3),
  continueWatching:[movies[0]],
  recentlyAdded:movies,
  movies,
  series,
  favorites:[]
};

const catalog = [...movies,...episodes];
const detailFor = (item:MediaSummary): MediaDetail => ({
  ...item,
  overview:'Ficha de prueba para validar layout, clicks y scroll en desarrollo local.',
  genres:['Drama'],
  cast:['CINE WANA'],
  runtimeMs:item.technical.durationMs,
  tracks:[],
  fileName:`${item.id}.mp4`,
  manualMetadata:false,
  metadataStatus:'pending',
  metadataSourceUrl:null,
  metadataImportedAt:null,
  metadataCandidates:[],
  recommendations:movies.slice(1,4)
});

const boot: Bootstrap = {
  roots:[{ id:'root-1', displayName:'Biblioteca', enabled:true, recursive:true, watchEnabled:false, status:'online', lastScanAt:now, disconnectedCount:0, localPath:'D:\\peliculas-y-series' }],
  scan,
  home,
  accounts:[{ id:'account-1', name:'lucas' }],
  activeAccount:{ id:'account-1', name:'lucas' },
  ffprobeAvailable:true,
  playerAvailable:true
};

const imageAnalysis: ImageAnalysis = {
  averageLight:42,
  shadowsPercent:31,
  highlightsPercent:6,
  averageSaturation:28,
  warmth:7,
  sampledFrames:18,
  suggested:{ brightness:3, contrast:8, saturation:6, shadows:6, highlights:-2, temperature:-3 }
};

export function installDevTauriMock() {
  mockWindows('main');
  mockConvertFileSrc('windows');
  mockIPC((cmd,args) => {
    switch (cmd) {
      case 'bootstrap':
        return boot;
      case 'catalog':
        return catalog;
      case 'media_detail': {
        const id = (args as { id?: string }).id;
        const item = catalog.find(value => value.id === id);
        return item ? detailFor(item) : null;
      }
      case 'technical_path':
        return 'D:\\peliculas-y-series\\sample.mp4';
      case 'analyze_media_image':
        return imageAnalysis;
      case 'start_scan':
      case 'cancel_scan':
        return scan;
      case 'set_media_flag':
      case 'save_progress':
      case 'logout_account':
        return null;
      default:
        return null;
    }
  }, { shouldMockEvents:true });
}
