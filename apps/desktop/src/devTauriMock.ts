import { mockConvertFileSrc, mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import type { Bootstrap, CategoryOption, CategoryPreference, CategoryRow, CategoryStyle, CustomCategory, HomeDto, MediaPerson, ImageAnalysis, MediaDetail, MediaSummary, SagaSummary, ScanProgress, SeriesSummary } from './types';

const now = new Date().toISOString();
const scan: ScanProgress = { running:false, cancelRequested:false, found:16, processed:16, skipped:0, errors:0, percent:100 };
const UNCATEGORIZED = 'Sin categoría';

type MovieSeed = { id:string; title:string; year:number; genres:string[]; overview?:string; saga?:[string,string,number]; progress?:number };

const movie = ({id,title,year,genres,overview,saga,progress=0}:MovieSeed): MediaSummary => {
  const incomplete = genres.length === 0 || !overview;
  return {
    id,
    kind:'movie',
    title,
    year,
    overview,
    progressPercent:progress,
    favorite:false,
    inWatchlist:false,
    completed:false,
    offline:false,
    addedAt:now,
    technical:{ height:1080, durationMs:7200000 },
    categories:incomplete ? [...genres, UNCATEGORIZED] : genres,
    incomplete,
    sagaId:saga?.[0],
    sagaTitle:saga?.[1],
    sagaPosition:saga?.[2]
  };
};

const episode = (id:string,seriesTitle:string,seasonNumber:number,episodeNumber:number,genres:string[]): MediaSummary => ({
  id,
  kind:'episode',
  title:`Episodio ${episodeNumber}`,
  seriesTitle,
  seasonNumber,
  episodeNumber,
  year:2022,
  overview:genres.length ? 'Ficha de prueba.' : undefined,
  progressPercent:0,
  favorite:false,
  inWatchlist:false,
  completed:false,
  offline:false,
  addedAt:now,
  technical:{ height:1080, durationMs:3600000 },
  categories:genres.length ? genres : [UNCATEGORIZED],
  incomplete:genres.length === 0
});

const sinopsis = 'Ficha de prueba para validar el orden de categorías en desarrollo local.';

const movies = [
  movie({ id:'movie-matrix', title:'Matrix', year:1999, genres:['Ciencia ficción','Acción'], overview:sinopsis, progress:42 }),
  movie({ id:'movie-interstellar', title:'Interstellar', year:2014, genres:['Ciencia ficción','Drama'], overview:sinopsis }),
  movie({ id:'movie-dune', title:'Dune Parte Dos', year:2024, genres:['Ciencia ficción','Aventura'], overview:sinopsis }),
  movie({ id:'movie-especies-1', title:'Especies', year:1995, genres:['Ciencia ficción','Terror'], overview:sinopsis, saga:['tmdb:1575','Especies',1] }),
  movie({ id:'movie-especies-2', title:'Especies 2', year:1998, genres:['Ciencia ficción','Terror'], overview:sinopsis, saga:['tmdb:1575','Especies',2] }),
  movie({ id:'movie-especies-3', title:'Especies 3', year:2004, genres:['Ciencia ficción','Terror'], overview:sinopsis, saga:['tmdb:1575','Especies',3] }),
  movie({ id:'movie-especies-4', title:'Especies 4', year:2007, genres:['Ciencia ficción','Terror'], overview:sinopsis, saga:['tmdb:1575','Especies',4] }),
  movie({ id:'movie-chucky-1', title:'Chucky El Muñeco Diabólico', year:1988, genres:['Terror'], overview:sinopsis, saga:['tmdb:91361','Chucky',1] }),
  movie({ id:'movie-chucky-2', title:'Chucky El Muñeco Diabólico 2', year:1990, genres:['Terror'], overview:sinopsis, saga:['tmdb:91361','Chucky',2] }),
  movie({ id:'movie-chucky-3', title:'Chucky El Muñeco Diabólico 3', year:1991, genres:['Terror'], overview:sinopsis, saga:['tmdb:91361','Chucky',3] }),
  movie({ id:'movie-exorcista', title:'El Exorcista', year:1973, genres:['Terror','Suspenso'], overview:sinopsis }),
  movie({ id:'movie-creepers', title:'Jeepers Creepers', year:2001, genres:['Terror'], overview:sinopsis }),
  movie({ id:'movie-robocop-1', title:'Robocop', year:1987, genres:['Acción','Ciencia ficción'], overview:sinopsis, saga:['tmdb:1345','Robocop',1] }),
  movie({ id:'movie-robocop-2', title:'Robocop 2', year:1990, genres:['Acción','Ciencia ficción'], overview:sinopsis, saga:['tmdb:1345','Robocop',2] }),
  movie({ id:'movie-robocop-3', title:'Robocop 3', year:1993, genres:['Acción','Ciencia ficción'], overview:sinopsis, saga:['tmdb:1345','Robocop',3] }),
  movie({ id:'movie-deadpool', title:'Deadpool', year:2016, genres:['Acción','Comedia'], overview:sinopsis }),
  movie({ id:'movie-vengador', title:'El Vengador Del Futuro', year:1990, genres:[] }),
  movie({ id:'movie-manos', title:'El Joven Manos De Tijera', year:1990, genres:[] }),
  movie({ id:'movie-chainsaw', title:'Chainsaw Man Reze', year:2025, genres:[] }),
  movie({ id:'movie-back', title:'Back To The Future 3', year:1990, genres:['Aventura'] })
];

const episodes = [
  episode('episode-dragon-1','La Casa Del Dragon',1,1,['Ciencia ficción','Fantasía']),
  episode('episode-last-1','The Last Of Us',1,1,['Terror','Drama']),
  episode('episode-avatar-1','Avatar Aang',1,1,[])
];

const series: SeriesSummary[] = [
  { episodeId:'episode-dragon-1', title:'La Casa Del Dragon', seasons:1, episodes:1, latestAddedAt:now, seasonItems:[{seasonNumber:1,title:'Temporada 1',episodes:[episodes[0]]}] },
  { episodeId:'episode-last-1', title:'The Last Of Us', seasons:1, episodes:1, latestAddedAt:now, seasonItems:[{seasonNumber:1,title:'Temporada 1',episodes:[episodes[1]]}] },
  { episodeId:'episode-avatar-1', title:'Avatar Aang', seasons:1, episodes:1, latestAddedAt:now, seasonItems:[{seasonNumber:1,title:'Temporada 1',episodes:[episodes[2]]}] }
];

const GENRE_ORDER = ['Ciencia ficción','Acción','Aventura','Animación','Bélica','Comedia','Crimen','Documental','Drama','Familia','Fantasía','Historia','Misterio','Música','Romance','Suspenso','Terror','Western'];
const slug = (label:string) => label.normalize('NFD').replace(/[\u0300-\u036f]/g,'').toLowerCase().replace(/\s+/g,'-');
const seriesGenres = (show:SeriesSummary) => show.seasonItems.flatMap(season=>season.episodes.flatMap(item=>item.categories)).filter(genre=>genre!==UNCATEGORIZED);

const sagas: SagaSummary[] = Object.values(movies.filter(item=>item.sagaId).reduce<Record<string,SagaSummary>>((groups,item)=>{
  const id = item.sagaId as string;
  groups[id] = groups[id] || { id, title:item.sagaTitle||item.title, items:[] };
  groups[id].items.push(item);
  return groups;
},{})).sort((a,b)=>b.items.length-a.items.length||a.title.localeCompare(b.title));

/* Reproduce el armado del backend para que la vista previa del navegador reordene de verdad. */
function buildCategories(preferences:CategoryPreference[]): { categories:CategoryRow[]; categorySettings:CategoryOption[] } {
  const rows:CategoryRow[] = [];
  for (const category of customCategories) {
    const items = movies.filter(item=>category.items.includes(item.id));
    const shows = series.filter(show=>category.series.includes(show.title));
    rows.push({ id:category.id, label:category.label, kind:'custom', count:items.length+shows.length, items, series:shows });
  }
  if (series.length) rows.push({ id:'series', label:'Series', kind:'series', count:series.length, series });
  for (const label of GENRE_ORDER) {
    const items = movies.filter(item=>item.categories.includes(label));
    if (items.length) rows.push({ id:slug(label), label, kind:'movies', count:items.length, items });
    const shows = series.filter(show=>seriesGenres(show).includes(label));
    if (shows.length) rows.push({ id:`series-${slug(label)}`, label:`Series de ${label.toLowerCase()}`, kind:'series', count:shows.length, series:shows });
  }
  if (sagas.length) rows.push({ id:'sagas', label:'Sagas', kind:'sagas', count:sagas.length, sagas });
  const pendingMovies = movies.filter(item=>item.incomplete);
  const pendingSeries = series.filter(show=>seriesGenres(show).length===0);
  if (pendingMovies.length||pendingSeries.length) rows.push({ id:'sin-categoria', label:UNCATEGORIZED, kind:'uncategorized', count:pendingMovies.length+pendingSeries.length, items:pendingMovies, series:pendingSeries });

  const tier = (row:CategoryRow) => row.kind==='custom'?0:row.id==='ciencia-ficcion'?1:row.kind==='sagas'?2:row.kind==='movies'?3:row.id==='series'?5:row.kind==='series'?4:6;
  const hiddenByDefault = (row:CategoryRow) => row.id.startsWith('series-');
  const rank = (row:CategoryRow) => preferences.findIndex(preference=>preference.id===row.id);
  rows.sort((left,right)=>{
    const [a,b] = [rank(left),rank(right)];
    if (a>=0&&b>=0) return a-b;
    if (a>=0) return -1;
    if (b>=0) return 1;
    return tier(left)-tier(right)||right.count-left.count||left.label.localeCompare(right.label);
  });
  const categorySettings = rows.map(row=>({ id:row.id, label:row.label, kind:row.kind, count:row.count, hidden:preferences.find(preference=>preference.id===row.id)?.hidden??hiddenByDefault(row) }));
  return { categories:rows.filter(row=>!categorySettings.find(option=>option.id===row.id)?.hidden), categorySettings };
}

let preferences:CategoryPreference[] = [];
let categoryStyle:CategoryStyle = 'gold';
let carouselDrag = false;
let customCategories:CustomCategory[] = [{ id:'custom:demo', label:'Para ver con Rox', items:['movie-matrix','movie-deadpool'], series:[] }];

const buildHome = (): HomeDto => ({
  heroes:movies.slice(0,3),
  continueWatching:[movies[0]],
  recentlyAdded:movies.slice(0,8),
  movies,
  series,
  favorites:[],
  categoryStyle,
  carouselDrag,
  customCategories,
  ...buildCategories(preferences)
});

/* Sin fotos reales: el mock corre en el navegador y no toca el cache de Windows, asi que las
   caritas se dibujan con iniciales, que es el mismo camino que sigue una persona sin foto. */
const samplePeople: MediaPerson[] = [
  { name:'Ridley Scott', role:'director' },
  { name:'Dan O’Bannon', role:'writer' },
  { name:'Sigourney Weaver', role:'actor', character:'Ripley' },
  { name:'Tom Skerritt', role:'actor', character:'Dallas' },
  { name:'John Hurt', role:'actor', character:'Kane' },
  { name:'Veronica Cartwright', role:'actor', character:'Lambert' },
  { name:'Harry Dean Stanton', role:'actor', character:'Brett' },
  { name:'Yaphet Kotto', role:'actor', character:'Parker' }
];

const catalog = [...movies,...episodes];
const detailFor = (item:MediaSummary): MediaDetail => ({
  ...item,
  overview:item.overview||'Esta ficha todavía no tiene sinopsis. Elegí la portada correcta de TMDB o escribí los datos a mano.',
  genres:item.categories.filter(genre=>genre!==UNCATEGORIZED),
  cast:['CINE WANA'],
  runtimeMs:item.technical.durationMs,
  tracks:[],
  fileName:`${item.id}.mp4`,
  manualMetadata:false,
  metadataStatus:item.incomplete?'not_found':'imported',
  metadataSourceUrl:null,
  metadataImportedAt:null,
  metadataCandidates:[],
  recommendations:movies.slice(1,4),
  people:item.incomplete?[]:samplePeople
});

const boot = (): Bootstrap => ({
  roots:[{ id:'root-1', displayName:'Biblioteca', enabled:true, recursive:true, watchEnabled:false, status:'online', lastScanAt:now, disconnectedCount:0, localPath:'D:\\peliculas-y-series' }],
  scan,
  home:buildHome(),
  accounts:[{ id:'account-1', name:'lucas' }],
  activeAccount:{ id:'account-1', name:'lucas' },
  ffprobeAvailable:true,
  playerAvailable:true,
  identificationReviews:[]
});

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
        return boot();
      case 'catalog':
        return catalog;
      case 'set_category_order':
        preferences = (args as { preferences?: CategoryPreference[] }).preferences || [];
        return buildHome();
      case 'set_carousel_drag':
        carouselDrag = Boolean((args as { enabled?: boolean }).enabled);
        return buildHome();
      case 'set_category_style':
        categoryStyle = (args as { style?: CategoryStyle }).style || 'gold';
        return buildHome();
      case 'create_category': {
        const label = String((args as { label?: string }).label || '').trim();
        if (label) customCategories = [...customCategories,{ id:`custom:${label.toLowerCase()}`, label, items:[], series:[] }];
        return buildHome();
      }
      case 'rename_category': {
        const { id, label } = args as { id?: string; label?: string };
        customCategories = customCategories.map(category=>category.id===id&&label?{ ...category, label }:category);
        return buildHome();
      }
      case 'delete_category':
        customCategories = customCategories.filter(category=>category.id!==(args as { id?: string }).id);
        return buildHome();
      case 'set_category_member': {
        const { id, mediaId, seriesTitle, member } = args as { id?: string; mediaId?: string|null; seriesTitle?: string|null; member?: boolean };
        customCategories = customCategories.map(category=>{
          if (category.id !== id) return category;
          const items = mediaId ? category.items.filter(value=>value!==mediaId).concat(member?[mediaId]:[]) : category.items;
          const shows = seriesTitle ? category.series.filter(value=>value!==seriesTitle).concat(member?[seriesTitle]:[]) : category.series;
          return { ...category, items, series:shows };
        });
        return buildHome();
      }
      case 'media_detail': {
        const id = (args as { id?: string }).id;
        const item = catalog.find(value => value.id === id);
        return item ? detailFor(item) : null;
      }
      case 'next_up': {
        const mediaId = (args as { mediaId?: string }).mediaId;
        const currentIndex = catalog.findIndex(value => value.id === mediaId);
        return currentIndex >= 0 ? catalog[currentIndex + 1] ?? null : null;
      }
      case 'technical_path':
        return 'D:\\peliculas-y-series\\sample.mp4';
      case 'rescan_media_item':
        return false;
      case 'analyze_media_image':
        return imageAnalysis;
      case 'metadata_poster_options':
        return [];
      case 'start_scan':
      case 'cancel_scan':
        return scan;
      case 'set_media_flag':
      case 'resolve_identification':
      case 'reveal_media_file':
      case 'save_progress':
      case 'logout_account':
        return null;
      default:
        return null;
    }
  }, { shouldMockEvents:true });
}
