import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import {
  ArrowDownUp, Bookmark, Camera, Check, ChevronLeft, ChevronRight, CircleAlert, Clock3,
  Crosshair, Drama, Eye, EyeOff, Film, Fingerprint, FolderCog, Ghost, GripVertical, Heart, History, Landmark,
  LayoutGrid, Layers, Mountain, Music, Palette, Puzzle, Rocket, Smile, Sparkles, Sun, Swords, VenetianMask,
  FolderOpen, Home, ImagePlus, KeyRound, Library, ListVideo, LoaderCircle, LogOut, Pencil, Play, RefreshCw, Save,
  Clapperboard, Copy, Plus, QrCode, Radio, RotateCcw, Search, Settings, ShieldCheck, Smartphone, Star, Tags, Tv, UserRound, Users, Wifi, X
} from 'lucide-react';
import type { Bootstrap, CategoryKind, CategoryOption, CategoryPreference, CategoryRow, CategoryStyle, CustomCategory, MediaPerson, MetadataRefreshProgress, ClassificationUpdate, HomeDto, IdentificationReview, MediaDetail, MediaMetadataCandidate, MediaMetadataUpdate, MediaSummary, RemoteCommand, RemoteStatus, SagaSummary, ScanProgress, SeriesSummary } from './types';
import { InternalPlayer, type InternalPlayerSource } from './InternalPlayer';
import { countLibrary } from './libraryCounts';
import tmdbLogo from './tmdb-logo.svg';

type Page = 'Inicio'|'Categorías'|'Sagas'|'Películas'|'Series'|'Continuar viendo'|'Mi lista'|'Favoritos'|'Agregadas recientemente'|'Historial'|'Configuración';
type AuthMode = 'create'|'login';
type PendingAccount = { name:string; password:string };
type PendingUpdate = Awaited<ReturnType<typeof check>>;
/* El arrastre lateral lo consulta cada fila. Va por contexto para no encadenar la preferencia por
   toda la jerarquia de componentes que hay entre Configuracion y una fila del inicio. */
const CarouselDragContext = createContext(false);

const emptyHome: HomeDto = { heroes:[],continueWatching:[],recentlyAdded:[],movies:[],series:[],favorites:[],categories:[],categorySettings:[],categoryStyle:'gold',customCategories:[],carouselDrag:false };
const emptyScan: ScanProgress = {running:false,cancelRequested:false,found:0,processed:0,skipped:0,errors:0,percent:0};

const navigation: Array<{label:Page; icon: typeof Home}> = [
  {label:'Inicio',icon:Home},{label:'Categorías',icon:Tags},{label:'Sagas',icon:Layers},{label:'Películas',icon:Film},{label:'Series',icon:Tv},{label:'Continuar viendo',icon:Clock3},
  {label:'Mi lista',icon:ListVideo},{label:'Favoritos',icon:Heart},{label:'Agregadas recientemente',icon:Star},{label:'Historial',icon:History},{label:'Configuración',icon:Settings}
];

export function App() {
  const [page,setPage]=useState<Page>('Inicio');
  const [boot,setBoot]=useState<Bootstrap|null>(null);
  const [authMode,setAuthMode]=useState<AuthMode>('create');
  const [pendingAccount,setPendingAccount]=useState<PendingAccount|null>(null);
  const [home,setHome]=useState<HomeDto>(emptyHome);
  const [items,setItems]=useState<MediaSummary[]>([]);
  const [scan,setScan]=useState<ScanProgress>(emptyScan);
  const [search,setSearch]=useState('');
  const [heroIndex,setHeroIndex]=useState(0);
  const [detail,setDetail]=useState<MediaDetail|null>(null);
  const [seriesDetail,setSeriesDetail]=useState<SeriesSummary|null>(null);
  const [sagaDetail,setSagaDetail]=useState<SagaSummary|null>(null);
  const [category,setCategory]=useState<string|null>(null);
  const [metadataRefresh,setMetadataRefresh]=useState<MetadataRefreshProgress|null>(null);
  const [playerSource,setPlayerSource]=useState<InternalPlayerSource|null>(null);
  const [availableUpdate,setAvailableUpdate]=useState<PendingUpdate|null>(null);
  const [updateMessage,setUpdateMessage]=useState<string|null>(null);
  const [updating,setUpdating]=useState(false);
  const [metadataLoading,setMetadataLoading]=useState(false);
  const [metadataNotice,setMetadataNotice]=useState<string|null>(null);
  const [remoteStatus,setRemoteStatus]=useState<RemoteStatus|null>(null);
  const [remoteBusy,setRemoteBusy]=useState(false);
  const [error,setError]=useState<string|null>(null);

  const refresh = useCallback(async()=>{
    try {
      const data=await invoke<Bootstrap>('bootstrap');
      setBoot(data);setHome(data.home);setScan(data.scan);
      if(!data.activeAccount){
        setItems([]);setHome(emptyHome);setAuthMode(data.accounts.length?'login':'create');setError(null);return;
      }
      const catalog=await invoke<MediaSummary[]>('catalog',{query:{search:null,kind:null,filter:null,sort:'added_desc',limit:1000,offset:0}});
      setItems(catalog);setError(null);
    } catch (cause) { setError(String(cause)); }
  },[]);

  useEffect(()=>{ void refresh(); const unsubs=[listen<ScanProgress>('scan-progress',e=>setScan(e.payload)),listen('library-changed',()=>void refresh())]; return()=>{void Promise.all(unsubs).then(values=>values.forEach(fn=>fn()));};},[refresh]);
  useEffect(()=>{let timer:number;const schedule=()=>{const now=new Date();const next=new Date(now);next.setHours(24,0,1,0);timer=window.setTimeout(()=>{void refresh();schedule();},next.getTime()-now.getTime());};schedule();return()=>window.clearTimeout(timer);},[refresh]);
  useEffect(()=>{let cleanup:(()=>void)|undefined;void listen<MetadataRefreshProgress>('metadata-refresh',event=>setMetadataRefresh(event.payload)).then(unlisten=>cleanup=unlisten);return()=>cleanup?.();},[]);
  useEffect(()=>{void invoke<RemoteStatus>('remote_status').then(setRemoteStatus).catch(()=>{});let cleanup:(()=>void)|undefined;void listen<RemoteStatus>('remote-status-changed',event=>setRemoteStatus(event.payload)).then(unlisten=>cleanup=unlisten);return()=>cleanup?.();},[]);
  useEffect(()=>{if(home.heroes.length<2||detail||seriesDetail)return;const timer=setInterval(()=>setHeroIndex(i=>(i+1)%home.heroes.length),8000);return()=>clearInterval(timer);},[home.heroes.length,detail,seriesDetail]);
  useEffect(()=>setHeroIndex(i=>Math.min(i,Math.max(home.heroes.length-1,0))),[home.heroes.length]);

  const visible=useMemo(()=>{
    let list=items;
    if(page==='Películas')list=list.filter(i=>i.kind==='movie');
    if(page==='Continuar viendo')list=list.filter(i=>i.progressPercent>0&&!i.completed);
    if(page==='Mi lista')list=list.filter(i=>i.inWatchlist);
    if(page==='Favoritos')list=list.filter(i=>i.favorite);
    if(page==='Historial')list=list.filter(i=>i.progressPercent>0||i.completed);
    if(search.trim()){const q=search.trim().toLocaleLowerCase();list=list.filter(i=>[i.title,i.seriesTitle,i.year?.toString()].some(v=>v?.toLocaleLowerCase().includes(q)));}
    return list;
  },[items,page,search]);

  const startMetadataRefresh=()=>{void invoke<MetadataRefreshProgress>('refresh_all_metadata').then(final=>{setMetadataRefresh(final);void refresh();}).catch(cause=>setError(String(cause)));};
  const cancelMetadataRefresh=()=>{void invoke('cancel_metadata_refresh').catch(cause=>setError(String(cause)));};
  const createCategory=async(label:string)=>{try{setError(null);setHome(await invoke<HomeDto>('create_category',{label}));}catch(cause){setError(String(cause));throw cause;}};
  const renameCategory=async(id:string,label:string)=>{try{setError(null);setHome(await invoke<HomeDto>('rename_category',{id,label}));}catch(cause){setError(String(cause));throw cause;}};
  const deleteCategory=async(id:string)=>{try{setError(null);setHome(await invoke<HomeDto>('delete_category',{id}));}catch(cause){setError(String(cause));throw cause;}};
  const setCategoryMember=async(id:string,member:boolean,mediaId?:string,seriesTitle?:string)=>{try{setError(null);setHome(await invoke<HomeDto>('set_category_member',{id,mediaId:mediaId??null,seriesTitle:seriesTitle??null,member}));}catch(cause){setError(String(cause));}};
  const toggleCarouselDrag=async(enabled:boolean)=>{try{setError(null);setHome(await invoke<HomeDto>('set_carousel_drag',{enabled}));}catch(cause){setError(String(cause));}};
  const saveCategoryStyle=async(style:CategoryStyle)=>{try{setError(null);setHome(await invoke<HomeDto>('set_category_style',{style}));}catch(cause){setError(String(cause));}};
  const saveCategories=async(preferences:CategoryPreference[])=>{try{setError(null);setHome(await invoke<HomeDto>('set_category_order',{preferences}));}catch(cause){setError(String(cause));throw cause;}};
  const rescan=async()=>{try{if(scan.running){setScan(await invoke('cancel_scan'));}else{setScan(await invoke('start_scan',{reason:'manual'}));}}catch(cause){setError(String(cause));}};
  const chooseFolder=async()=>{const selected=await open({directory:true,multiple:false,title:'Elegir carpeta de películas y series'});if(typeof selected==='string'){try{await invoke('replace_library_root',{path:selected});await refresh();}catch(cause){setError(String(cause));}}};
  const openDetail=async(id:string)=>{try{const data=await invoke<MediaDetail|null>('media_detail',{id});setDetail(data);}catch(cause){setError(String(cause));}};
  const resolveIdentification=async(mediaId:string,classification:ClassificationUpdate)=>{try{setError(null);await invoke('resolve_identification',{mediaId,classification});if(detail?.id===mediaId)setDetail(await invoke<MediaDetail|null>('media_detail',{id:mediaId}));await refresh();}catch(cause){setError(String(cause));throw cause;}};
  const setFlag=async(item:MediaSummary,flag:'favorite'|'watchlist')=>{const value=flag==='favorite'?!item.favorite:!item.inWatchlist;await invoke('set_media_flag',{mediaId:item.id,flag,value});await refresh();if(detail?.id===item.id)setDetail(await invoke('media_detail',{id:item.id}));};
  const saveMetadata=async(mediaId:string,metadata:MediaMetadataUpdate)=>{try{setError(null);await invoke('update_media_metadata',{mediaId,metadata});const next=await invoke<MediaDetail|null>('media_detail',{id:mediaId});setDetail(next);await refresh();}catch(cause){setError(String(cause));throw cause;}};
  const refreshMetadata=async(mediaId:string)=>{try{setMetadataLoading(true);setError(null);await invoke('refresh_media_metadata',{mediaId});const next=await invoke<MediaDetail|null>('media_detail',{id:mediaId});setDetail(next);await refresh();}catch(cause){setError(String(cause));throw cause;}finally{setMetadataLoading(false);}};
  const refreshMetadataFromSettings=async(mediaId:string)=>{try{setError(null);await invoke('refresh_media_metadata',{mediaId});await refresh();}catch(cause){setError(String(cause));throw cause;}};
  const applyMetadataCandidate=async(mediaId:string,candidate:MediaMetadataCandidate)=>{try{setMetadataLoading(true);setError(null);await invoke('apply_metadata_candidate',{mediaId,candidate,preserveTitle:true});const next=await invoke<MediaDetail|null>('media_detail',{id:mediaId});setDetail(next);await refresh();}catch(cause){setError(String(cause));throw cause;}finally{setMetadataLoading(false);}};
  const applyMetadataCandidateFromSettings=async(mediaId:string,candidate:MediaMetadataCandidate)=>{try{setError(null);await invoke('apply_metadata_candidate',{mediaId,candidate,preserveTitle:false});setMetadataNotice(`Portada aplicada: ${candidate.title}. El cambio ya quedó guardado en la biblioteca.`);await refresh();}catch(cause){setError(String(cause));throw cause;}};
  const playMedia=async(id:string)=>{try{setError(null);const [detail,url]=await Promise.all([invoke<MediaDetail|null>('media_detail',{id}),invoke<string>('player_media_url',{mediaId:id})]);if(!detail||!url){setError('No se encontró el archivo para reproducir.');return;}const durationMs=detail.runtimeMs||detail.technical.durationMs||0;const resumeMs=detail.completed?0:Math.round(durationMs*(detail.progressPercent/100));setDetail(null);setPlayerSource({detail,url,resumeMs});}catch(cause){setError(`No se pudo abrir el reproductor interno: ${String(cause)}`);}};
  const openExternalMedia=async(id:string)=>{try{setError(null);await invoke('player_command',{command:{type:'play',media_id:id}});}catch(cause){setError(`No se pudo abrir reproductor externo: ${String(cause)}`);}};
  const playNextMedia=async(id:string)=>{try{setError(null);const [detail,url]=await Promise.all([invoke<MediaDetail|null>('media_detail',{id}),invoke<string>('player_media_url',{mediaId:id})]);if(!detail||!url){setError('No se encontro la siguiente parte para reproducir.');return;}setDetail(null);setPlayerSource({detail,url,resumeMs:0});}catch(cause){setError(`No se pudo abrir la siguiente parte: ${String(cause)}`);}};
  useEffect(()=>{let cleanup:(()=>void)|undefined;void listen<RemoteCommand>('remote-command',event=>{const command=event.payload;if(command.type==='library_play_media')void playMedia(command.media_id);if(command.type==='navigate_back'){if(detail)setDetail(null);else if(playerSource)setPlayerSource(null);else setPage('Inicio');}if(command.type==='navigate'){const pages:Page[]=['Inicio','Películas','Series','Mi lista','Favoritos'];const current=Math.max(0,pages.indexOf(page));if(command.direction==='left'||command.direction==='up')setPage(pages[(current-1+pages.length)%pages.length]);if(command.direction==='right'||command.direction==='down')setPage(pages[(current+1)%pages.length]);}}).then(unlisten=>cleanup=unlisten);return()=>cleanup?.();},[detail,page,playerSource]);
  const runRemoteAction=async(command:string,args:Record<string,unknown>={})=>{try{setRemoteBusy(true);setError(null);setRemoteStatus(await invoke<RemoteStatus>(command,args));}catch(cause){setError(String(cause));}finally{setRemoteBusy(false);}};
  const submitAccount=async(mode:AuthMode,name:string,password:string)=>{if(mode==='create'){setPendingAccount({name,password});return;}try{setError(null);await invoke('login_account',{name,password});setSearch('');setPage('Inicio');await refresh();}catch(cause){setError(String(cause));}};
  const confirmCreateAccount=async()=>{if(!pendingAccount)return;try{setError(null);await invoke('create_account',pendingAccount);setPendingAccount(null);setSearch('');setPage('Inicio');await refresh();}catch(cause){setError(String(cause));setPendingAccount(null);}};
  const logout=async()=>{try{await invoke('logout_account');setDetail(null);setPlayerSource(null);setSearch('');setPage('Inicio');await refresh();}catch(cause){setError(String(cause));}};
  const checkForUpdates=async()=>{try{setUpdating(true);setAvailableUpdate(null);setUpdateMessage('Buscando actualizaciones en GitHub Releases...');const update=await check();if(update){setAvailableUpdate(update);setUpdateMessage(`Actualización disponible: versión ${update.version}`);}else{setUpdateMessage('CINE WANA ya está en la última versión publicada.');}}catch(cause){setUpdateMessage(`No se pudo buscar actualizaciones: ${String(cause)}`);}finally{setUpdating(false);}};
  const installAvailableUpdate=async()=>{if(!availableUpdate)return;try{setUpdating(true);let downloaded=0;let contentLength=0;await availableUpdate.downloadAndInstall(event=>{if(event.event==='Started'){contentLength=event.data.contentLength||0;setUpdateMessage('Descargando actualización...');}else if(event.event==='Progress'){downloaded+=event.data.chunkLength;setUpdateMessage(contentLength?`Descargando ${Math.round(downloaded/contentLength*100)}%`:'Descargando actualización...');}else if(event.event==='Finished'){setUpdateMessage('Instalando actualización...');}});setUpdateMessage('Actualización instalada. En Windows la app se cerrará para terminar.');}catch(cause){setUpdateMessage(`No se pudo instalar la actualización: ${String(cause)}`);}finally{setUpdating(false);}};
  const hero=home.heroes[heroIndex];
  const libraryCounts=useMemo(()=>countLibrary(home.movies,home.series),[home.movies,home.series]);
  const settingsProps=boot?{boot,scan,updating,updateMessage,updateVersion:availableUpdate?.version,metadataNotice,onRescan:rescan,onChoose:chooseFolder,onLogout:logout,onCheckUpdates:checkForUpdates,onInstallUpdate:installAvailableUpdate,onResolveIdentification:resolveIdentification,onApplyMetadataCandidate:applyMetadataCandidateFromSettings,onRefreshMetadata:refreshMetadataFromSettings,categoryOptions:home.categorySettings,onSaveCategories:saveCategories,categoryStyle:home.categoryStyle,onSaveCategoryStyle:saveCategoryStyle,carouselDrag:home.carouselDrag,onToggleCarouselDrag:toggleCarouselDrag,metadataRefresh,onStartRefresh:startMetadataRefresh,onCancelRefresh:cancelMetadataRefresh,customCategories:home.customCategories,onCreateCategory:createCategory,onRenameCategory:renameCategory,onDeleteCategory:deleteCategory,remote:remoteStatus,remoteBusy,runRemoteAction}:null;

  if(boot&&!boot.activeAccount)return <AuthScreen boot={boot} mode={authMode} setMode={setAuthMode} pendingAccount={pendingAccount} onSubmit={submitAccount} onConfirmCreate={confirmCreateAccount} onCancelCreate={()=>setPendingAccount(null)} error={error} clearError={()=>setError(null)}/>;

  return <CarouselDragContext.Provider value={home.carouselDrag}><div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span>CINE</span><strong>WANA</strong></div>
      <nav>{navigation.map(({label,icon:Icon})=><button key={label} className={page===label?'active':''} title={label} onClick={()=>setPage(label)}><Icon size={18}/><span>{label}</span></button>)}</nav>
      <div className="sidebar-status"><span className={`status-dot ${boot?.roots.some(r=>r.status==='online')?'online':''}`}/><section className="sidebar-status-copy"><b>{boot?.roots[0]?.status==='online'?'Biblioteca conectada':'Biblioteca sin conexión'}</b><small>{libraryCounts.files} archivos en catálogo</small><ul className="library-counts"><li><span>Películas</span><strong>{libraryCounts.movies}</strong></li><li><span>Series</span><strong>{libraryCounts.series}</strong></li><li><span>Capítulos</span><strong>{libraryCounts.chapters}</strong></li></ul></section></div>
    </aside>
    <main>
      <header className="topbar">
        <div className="search"><Search size={18}/><input value={search} onChange={e=>setSearch(e.target.value)} placeholder="Buscar títulos, años y series…"/>{search&&<button onClick={()=>setSearch('')}><X size={16}/></button>}</div>
        {boot?.activeAccount&&<div className="account-pill"><UserRound size={16}/><span>{boot.activeAccount.name}</span><button onClick={logout} title="Cerrar sesión"><LogOut size={15}/></button></div>}
        <button className={`scan-button ${scan.running?'working':''}`} onClick={rescan}>{scan.running?<><X size={17}/><span>Cancelar escaneo</span></>:<><RefreshCw size={17}/><span>Reescanear biblioteca</span></>}</button>
      </header>

      {scan.running&&<div className="scan-strip"><LoaderCircle className="spin" size={16}/><div><b>{scan.message||'Escaneando biblioteca'}</b><small>{scan.currentFile||`${scan.found} archivos encontrados`}</small></div><div className="scan-meter"><i style={{width:`${scan.percent}%`}}/></div><span>{Math.round(scan.percent)}%</span></div>}
      {error&&<div className="error-banner"><CircleAlert size={18}/><span>{error}</span><button onClick={()=>setError(null)}><X size={16}/></button></div>}

      {!boot?<Loading/>:page==='Configuración'&&settingsProps?<RemoteSettingsPage {...settingsProps}/>:page==='Series'?<SeriesPage series={home.series} search={search} openSeries={setSeriesDetail}/>:page==='Categorías'?<CategoriesPage categories={home.categories} onSelect={id=>{setCategory(id);setPage('Inicio');}}/>:page==='Sagas'?<SagasPage categories={home.categories} openSaga={setSagaDetail}/>:page==='Inicio'&&!search?<HomePage home={home} hero={hero} heroIndex={heroIndex} setHeroIndex={setHeroIndex} openDetail={openDetail} openSeries={setSeriesDetail} openSaga={setSagaDetail} setFlag={setFlag} playMedia={playMedia} category={category} setCategory={setCategory}/>:<CatalogPage title={page} items={visible} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>}
    </main>
    {sagaDetail&&<SagaDetailModal saga={sagaDetail} close={()=>setSagaDetail(null)} openDetail={openDetail} playMedia={playMedia}/>}
    {seriesDetail&&<SeriesDetailModal series={seriesDetail} close={()=>setSeriesDetail(null)} openDetail={openDetail} playMedia={playMedia} customCategories={home.customCategories} onToggleCategory={(id,member)=>setCategoryMember(id,member,undefined,seriesDetail.title)}/>}
    {detail&&<DetailModal detail={detail} review={boot?.identificationReviews.find(item=>item.mediaId===detail.id)} close={()=>setDetail(null)} setFlag={setFlag} playMedia={playMedia} openExternalMedia={openExternalMedia} onSaveMetadata={saveMetadata} onRefreshMetadata={refreshMetadata} onApplyCandidate={applyMetadataCandidate} onResolveIdentification={resolveIdentification} openDetail={openDetail} metadataLoading={metadataLoading} customCategories={home.customCategories} onToggleCategory={(id,member)=>setCategoryMember(id,member,detail.id)}/>}
    {playerSource&&settingsProps&&<InternalPlayer
      source={playerSource}
      onClose={()=>setPlayerSource(null)}
      onOpenExternal={openExternalMedia}
      onPlayNext={playNextMedia}
      onProgressSaved={()=>void refresh()}
      settingsPanel={<RemoteSettingsPage {...settingsProps}/>}
    />}
  </div></CarouselDragContext.Provider>;
}

function Loading(){return <div className="loading-screen"><LoaderCircle className="spin"/><b>Preparando tu biblioteca</b><span>La primera lectura puede tardar unos segundos.</span></div>}

function AuthScreen({boot,mode,setMode,pendingAccount,onSubmit,onConfirmCreate,onCancelCreate,error,clearError}:{boot:Bootstrap;mode:AuthMode;setMode:(mode:AuthMode)=>void;pendingAccount:PendingAccount|null;onSubmit:(mode:AuthMode,name:string,password:string)=>Promise<void>;onConfirmCreate:()=>Promise<void>;onCancelCreate:()=>void;error:string|null;clearError:()=>void}){
  const [name,setName]=useState('');
  const [password,setPassword]=useState('');
  const hasAccounts=boot.accounts.length>0;
  const passwordOk=/^[A-Za-z0-9]{4,10}$/.test(password);
  const canSubmit=name.trim().length>0&&passwordOk;
  const activeMode=mode==='login'&&hasAccounts?'login':'create';
  return <main className="auth-shell">
    <section className="auth-panel">
      <div className="auth-brand"><span>CINE</span><strong>WANA</strong></div>
      <div className="auth-heading"><span className="eyebrow">CUENTA LOCAL</span><h1>{activeMode==='create'?'Crear cuenta':'Entrar'}</h1><p>Nombre y contraseña local. Sin email.</p></div>
      {hasAccounts&&<div className="auth-tabs"><button className={activeMode==='login'?'active':''} onClick={()=>setMode('login')}>Entrar</button><button className={activeMode==='create'?'active':''} onClick={()=>setMode('create')}>Crear otra</button></div>}
      {hasAccounts&&activeMode==='login'&&<div className="known-accounts">{boot.accounts.map(account=><button key={account.id} onClick={()=>setName(account.name)}><UserRound size={13}/>{account.name}</button>)}</div>}
      {error&&<div className="error-banner auth-error"><CircleAlert size={18}/><span>{error}</span><button onClick={clearError}><X size={16}/></button></div>}
      <form onSubmit={event=>{event.preventDefault();if(canSubmit)void onSubmit(activeMode,name.trim(),password);}}>
        <label><span>Nombre</span><div><UserRound size={17}/><input value={name} onChange={event=>setName(event.target.value)} autoFocus maxLength={40} autoComplete="username"/></div></label>
        <label><span>Contraseña</span><div><KeyRound size={17}/><input value={password} onChange={event=>setPassword(event.target.value)} type="password" minLength={4} maxLength={10} pattern="[A-Za-z0-9]{4,10}" autoComplete={activeMode==='create'?'new-password':'current-password'}/></div><small>4 a 10 letras o números</small></label>
        <button className="primary auth-submit" disabled={!canSubmit}>{activeMode==='create'?'Crear cuenta':'Entrar'}</button>
      </form>
    </section>
    {pendingAccount&&<ConfirmAccountModal account={pendingAccount} onConfirm={onConfirmCreate} onCancel={onCancelCreate}/>}
  </main>
}

function ConfirmAccountModal({account,onConfirm,onCancel}:{account:PendingAccount;onConfirm:()=>Promise<void>;onCancel:()=>void}){
  return <div className="auth-confirm-backdrop" role="dialog" aria-modal="true">
    <section className="auth-confirm">
      <span className="eyebrow">CONFIRMAR CUENTA</span>
      <h2>¿Estás seguro que querés crear tu cuenta?</h2>
      <dl>
        <div><dt>Nombre</dt><dd>{account.name}</dd></div>
        <div><dt>Contraseña</dt><dd>{account.password}</dd></div>
      </dl>
      <div className="auth-confirm-actions">
        <button onClick={onCancel}>No, volver</button>
        <button className="primary" onClick={()=>void onConfirm()}>Sí, crear cuenta</button>
      </div>
    </section>
  </div>;
}

function MediaReviewDialog({detail,review,close,onSaveMetadata,onResolve,onApplyCandidate,onRetryMetadata,onChanged}:{detail:MediaDetail;review:IdentificationReview;close:()=>void;onSaveMetadata:(id:string,metadata:MediaMetadataUpdate)=>Promise<void>;onResolve:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>;onApplyCandidate:(id:string,candidate:MediaMetadataCandidate)=>Promise<void>;onRetryMetadata:(id:string)=>Promise<void>;onChanged:()=>Promise<void>}){
  const [artworkBusy,setArtworkBusy]=useState(false);
  const [artworkMessage,setArtworkMessage]=useState('');
  const [posterOptions,setPosterOptions]=useState<MediaMetadataCandidate[]>([]);
  const [posterOptionsLoading,setPosterOptionsLoading]=useState(false);
  const candidateKey=review.metadataCandidates.map(candidate=>candidate.id).join('|');
  useEffect(()=>{
    let cancelled=false;
    setPosterOptionsLoading(true);
    invoke<MediaMetadataCandidate[]>('metadata_poster_options',{mediaId:detail.id})
      .then(options=>{if(!cancelled)setPosterOptions(options);})
      .catch(()=>{if(!cancelled)setPosterOptions([]);})
      .finally(()=>{if(!cancelled)setPosterOptionsLoading(false);});
    return()=>{cancelled=true;};
  },[candidateKey,detail.id,detail.metadataSourceUrl,detail.metadataStatus]);
  const chooseArtwork=async(field:'posterPath'|'backdropPath')=>{
    const selected=await open({multiple:false,directory:false,title:field==='posterPath'?'Elegir una portada propia':'Elegir un fondo propio',filters:[{name:'Imagen',extensions:['png','jpg','jpeg','webp']}]});
    if(typeof selected!=='string')return;
    try{
      setArtworkBusy(true);setArtworkMessage('');
      await onSaveMetadata(detail.id,{title:detail.title,year:detail.year??null,overview:detail.overview||null,genres:detail.genres,cast:detail.cast,posterPath:field==='posterPath'?selected:null,backdropPath:field==='backdropPath'?selected:null});
      setArtworkMessage(field==='posterPath'?'Portada propia aplicada y guardada.':'Fondo propio aplicado y guardado.');
    }catch(cause){setArtworkMessage(`No se pudo guardar la imagen: ${String(cause)}`);}
    finally{setArtworkBusy(false);}
  };
  const reviewWithPosters=posterOptions.length?{...review,metadataCandidates:posterOptions}:review;
  return <div className="media-review-backdrop" onMouseDown={event=>{if(event.target===event.currentTarget)close();}}>
    <section className="media-review-dialog" role="dialog" aria-modal="true" aria-label={`Revisión de ${displayTitle(detail)}`}>
      <header><div><span className="eyebrow">REVISIÓN INDIVIDUAL</span><h2>{displayTitle(detail)}</h2><p>Corregí esta ficha sin buscarla en la lista general de Configuración.</p></div><button className="modal-close" onClick={close} aria-label="Cerrar revisión"><X/></button></header>
      <div className="manual-artwork-panel"><div><ImagePlus/><span><b>Imágenes propias</b><small>La imagen elegida se copia a la caché y al paquete portátil `.cinewana`.</small></span></div><div><button disabled={artworkBusy} onClick={()=>void chooseArtwork('posterPath')}><ImagePlus/>Elegir portada</button><button disabled={artworkBusy} onClick={()=>void chooseArtwork('backdropPath')}><ImagePlus/>Elegir fondo</button></div></div>
      {artworkMessage&&<div className={`metadata-card-message ${artworkMessage.includes('aplicad')?'success':'error'}`} role="status">{artworkMessage.includes('aplicad')?<Check/>:<CircleAlert/>}<span>{artworkMessage}</span></div>}
      {posterOptionsLoading&&<div className="poster-options-loading"><LoaderCircle className="spin"/>Buscando portadas alternativas en TMDB…</div>}
      <IdentificationReviewCard review={reviewWithPosters} onResolve={onResolve} onApplyCandidate={onApplyCandidate} onRetryMetadata={onRetryMetadata} onChanged={onChanged}/>
    </section>
  </div>;
}

function HomePage({home,hero,heroIndex,setHeroIndex,openDetail,openSeries,openSaga,setFlag,playMedia,category,setCategory}:{home:HomeDto;hero?:MediaSummary;heroIndex:number;setHeroIndex:(n:number)=>void;openDetail:(id:string)=>void;openSeries:(series:SeriesSummary)=>void;openSaga:(saga:SagaSummary)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void;category:string|null;setCategory:(id:string|null)=>void}){
  const active=home.categories.find(row=>row.id===category)||null;
  return <div className="content home-page">
    {hero?<section className="hero" style={{'--hero-hue':hueFor(hero.title),'--hero-image':hero.backdropUrl?`url("${assetUrl(hero.backdropUrl)}")`:'none'} as React.CSSProperties}>
      {hero.previewUrl&&<video key={hero.previewUrl} className="hero-video" src={assetUrl(hero.previewUrl)} muted autoPlay loop playsInline/>}<div className="hero-shade"/><div className="hero-noise"/><div className="hero-copy"><span className="eyebrow">VISTA PREVIA DE TU BIBLIOTECA</span><h1>{displayTitle(hero)}</h1><p>{hero.year||'Año sin identificar'} · {quality(hero)} {hero.technical.hdrType?`· ${hero.technical.hdrType}`:''}</p>
      <div className="hero-actions"><button className="primary" onClick={()=>void playMedia(hero.id)}><Play fill="currentColor" size={18}/>Reproducir</button><button onClick={()=>openDetail(hero.id)}>Ver detalles</button></div></div>
      <div className="hero-controls"><button onClick={()=>setHeroIndex((heroIndex-1+home.heroes.length)%home.heroes.length)}><ChevronLeft/></button><div>{home.heroes.map((_,i)=><button key={i} className={i===heroIndex?'active':''} onClick={()=>setHeroIndex(i)}/>)}</div><button onClick={()=>setHeroIndex((heroIndex+1)%home.heroes.length)}><ChevronRight/></button></div>
    </section>:<EmptyLibrary/>}
    <CategoryStrip categories={home.categories} active={category} onSelect={setCategory} style={home.categoryStyle}/>
    {active
      ?<CategoryDetail row={active} openDetail={openDetail} openSeries={openSeries} openSaga={openSaga} setFlag={setFlag} playMedia={playMedia}/>
      :<>
        <MediaRow title="Continuar viendo" items={home.continueWatching} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>
        <MediaRow title="Agregadas recientemente" items={home.recentlyAdded} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>
        {home.categories.map(row=><CategoryCarousel key={row.id} row={row} openDetail={openDetail} openSeries={openSeries} openSaga={openSaga} setFlag={setFlag} playMedia={playMedia} onSelect={setCategory}/>)}
        <MediaRow title="Favoritos" items={home.favorites} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>
      </>}
  </div>;
}

/* Cada categoría lleva su ícono, así la tira se lee de un vistazo sin tener que leer palabra por
   palabra. Las que no están acá caen en el ícono genérico. */
const categoryIcons: Record<string,typeof Home> = {
  'ciencia-ficcion':Rocket, sagas:Star, accion:Crosshair, suspenso:VenetianMask, drama:Drama,
  aventura:Mountain, comedia:Smile, terror:Ghost, animacion:Palette, belica:Swords, crimen:Fingerprint,
  documental:Camera, familia:Users, fantasia:Sparkles, historia:Landmark, misterio:Puzzle, musica:Music,
  romance:Heart, western:Sun, 'sin-categoria':CircleAlert
};
const categoryIcon=(row:{id:string;kind:CategoryKind})=>row.kind==='custom'?Bookmark:row.kind==='series'?Tv:categoryIcons[row.id]||Tags;

/* La tira de nombres vive pegada abajo de la portada: es donde cae la vista después de mirar el
   fragmento, y se queda fija arriba mientras se recorren las filas. */
function CategoryStrip({categories,active,onSelect,style}:{categories:CategoryRow[];active:string|null;onSelect:(id:string|null)=>void;style:CategoryStyle}){
  const stripRef=useRef<HTMLElement|null>(null);
  const [edges,setEdges]=useState({left:false,right:false});
  const updateEdges=useCallback(()=>{
    const strip=stripRef.current;
    if(!strip)return;
    setEdges({left:strip.scrollLeft>4,right:strip.scrollLeft+strip.clientWidth<strip.scrollWidth-4});
  },[]);
  useEffect(()=>{
    updateEdges();
    window.addEventListener('resize',updateEdges);
    return()=>window.removeEventListener('resize',updateEdges);
  },[categories,style,updateEdges]);
  /* La categoría elegida puede quedar fuera de la vista cuando se entra desde la página de
     Categorías, así que se la trae sola en vez de obligar a buscarla a mano. */
  useEffect(()=>{
    const strip=stripRef.current;
    if(!strip||!active)return;
    strip.querySelector(`[data-category="${CSS.escape(active)}"]`)?.scrollIntoView({behavior:'smooth',block:'nearest',inline:'center'});
  },[active]);
  const scrollStrip=(direction:-1|1)=>{
    const strip=stripRef.current;
    if(!strip)return;
    strip.scrollBy({left:direction*Math.max(240,strip.clientWidth*0.7),behavior:'smooth'});
    window.setTimeout(updateEdges,260);
  };
  if(!categories.length)return null;
  return <div className="category-strip-shell">
    <button className="strip-arrow strip-arrow-left" disabled={!edges.left} onClick={()=>scrollStrip(-1)} aria-label="Ver categorías anteriores" title="Anteriores"><ChevronLeft size={17}/></button>
    <nav ref={stripRef} className={`category-strip ${style}`} aria-label="Categorías" onScroll={updateEdges}>
      <button className={active?'':'active'} data-category="" onClick={()=>onSelect(null)}><LayoutGrid size={15}/>Todas</button>
      {categories.map(row=>{const Icon=categoryIcon(row);return <button key={row.id} data-category={row.id} className={`${active===row.id?'active':''} ${row.kind==='uncategorized'?'pending':''} ${row.kind==='sagas'?'saga':''}`} onClick={()=>onSelect(row.id)}><Icon size={15}/>{row.label}<i>{row.count}</i></button>;})}
    </nav>
    <button className="strip-arrow strip-arrow-right" disabled={!edges.right} onClick={()=>scrollStrip(1)} aria-label="Ver más categorías" title="Siguientes"><ChevronRight size={17}/></button>
  </div>;
}

/* Una fila por categoría. Las sagas y las series traen su propia tarjeta, así que la fila elige
   qué dibujar según lo que la categoría contiene. */
function CategoryCarousel({row,openDetail,openSeries,openSaga,setFlag,playMedia,onSelect}:{row:CategoryRow;openDetail:(id:string)=>void;openSeries:(series:SeriesSummary)=>void;openSaga:(saga:SagaSummary)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void;onSelect:(id:string)=>void}){
  const items=row.items||[];const series=row.series||[];const sagas=row.sagas||[];
  if(!items.length&&!series.length&&!sagas.length)return null;
  const pending=row.kind==='uncategorized';
  return <section className={`media-section ${pending?'pending-section':''}`}>
    <div className="section-title">
      <h2>{row.label}</h2><span className={pending?'pending':''}>{row.count}</span>
      <button className="section-more" onClick={()=>onSelect(row.id)}>{pending?'Arreglar fichas':'Ver todo'}<ChevronRight size={13}/></button>
    </div>
    <CarouselRow label={row.label}>
      {sagas.map(saga=><SagaCard key={saga.id} saga={saga} openSaga={openSaga}/>)}
      {series.map(show=><SeriesCard key={show.episodeId} series={show} openSeries={openSeries}/>)}
      {items.map(item=><MediaCard key={item.id} item={item} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia} flagPending={pending}/>)}
    </CarouselRow>
  </section>;
}

/* La categoría elegida desde la tira reemplaza las filas por una grilla completa. */
function CategoryDetail({row,openDetail,openSeries,openSaga,setFlag,playMedia}:{row:CategoryRow;openDetail:(id:string)=>void;openSeries:(series:SeriesSummary)=>void;openSaga:(saga:SagaSummary)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void}){
  const items=row.items||[];const series=row.series||[];const sagas=row.sagas||[];
  const pending=row.kind==='uncategorized';
  return <section className="media-section">
    <div className="page-heading">
      <div><span className="eyebrow">{pending?'FICHAS PARA COMPLETAR':'CATEGORÍA'}</span><h1>{row.label}</h1></div>
      <span>{row.count} {row.kind==='sagas'?'sagas':'títulos'}</span>
    </div>
    {pending&&<p className="pending-note"><CircleAlert size={15}/>Estas fichas no tienen género o les falta la sinopsis. Abrí cada una y elegí la portada correcta de TMDB o escribí los datos a mano.</p>}
    <div className="card-grid">
      {sagas.map(saga=><SagaCard key={saga.id} saga={saga} openSaga={openSaga}/>)}
      {series.map(show=><SeriesCard key={show.episodeId} series={show} openSeries={openSeries}/>)}
      {items.map(item=><MediaCard key={item.id} item={item} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia} flagPending={pending}/>)}
    </div>
  </section>;
}

function CategoriesPage({categories,onSelect}:{categories:CategoryRow[];onSelect:(id:string)=>void}){
  const titles=categories.reduce((total,row)=>total+row.count,0);
  return <div className="content catalog-page">
    <div className="page-heading"><div><span className="eyebrow">TU BIBLIOTECA ORDENADA</span><h1>Categorías</h1></div><span>{categories.length} categorías · {titles} títulos</span></div>
    {categories.length
      ?<div className="category-grid">{categories.map(row=><button key={row.id} className={`category-tile ${row.kind==='uncategorized'?'pending':''}`} onClick={()=>onSelect(row.id)}>
        <span className="category-tile-kind">{row.kind==='sagas'?'COLECCIONES':row.kind==='series'?'SERIES':row.kind==='uncategorized'?'PARA COMPLETAR':'PELÍCULAS'}</span>
        <b>{row.label}</b><i>{row.count}</i>
      </button>)}</div>
      :<div className="empty-results"><Tags/><h2>Todavía no hay categorías</h2><p>Van a aparecer solas después del próximo escaneo, con cada película en su lugar.</p></div>}
    <p className="pending-note"><CircleAlert size={15}/>El orden se cambia desde Configuración, y vale también para la tira de arriba del inicio.</p>
  </div>;
}

function SagasPage({categories,openSaga}:{categories:CategoryRow[];openSaga:(saga:SagaSummary)=>void}){
  const sagas=categories.find(row=>row.kind==='sagas')?.sagas||[];
  const movies=sagas.reduce((total,saga)=>total+saga.items.length,0);
  return <div className="content catalog-page">
    <div className="page-heading"><div><span className="eyebrow">PELÍCULAS QUE VAN JUNTAS</span><h1>Sagas</h1></div><span>{sagas.length} sagas · {movies} películas</span></div>
    {sagas.length
      ?<div className="card-grid">{sagas.map(saga=><SagaCard key={saga.id} saga={saga} openSaga={openSaga}/>)}</div>
      :<div className="empty-results"><Layers/><h2>Todavía no se armó ninguna saga</h2><p>Se arman solas con los datos de TMDB, y con los números del título cuando TMDB no reconoce la película.</p></div>}
  </div>;
}

function SagaCard({saga,openSaga}:{saga:SagaSummary;openSaga:(saga:SagaSummary)=>void}){
  return <article className="media-card series-card saga-card">
    <button className="poster-detail" onClick={()=>openSaga(saga)} aria-label={`Ver la saga ${saga.title}`}>
      <Poster title={saga.title} label="SAGA" src={saga.artworkUrl}/>
      <span className="saga-count">{saga.items.length}</span>
    </button>
    <div className="card-copy"><div><button className="title-link" onClick={()=>openSaga(saga)}>{saga.title}</button><p>{saga.items.length} películas</p></div></div>
  </article>;
}

function SagaDetailModal({saga,close,openDetail,playMedia}:{saga:SagaSummary;close:()=>void;openDetail:(id:string)=>void;playMedia:(id:string)=>void}){
  return <div className="modal-backdrop" onClick={close}><section className="series-detail-modal" onClick={event=>event.stopPropagation()}>
    <button className="modal-close" onClick={close} aria-label="Cerrar saga" title="Cerrar"><X size={17} aria-hidden="true"/></button>
    <div className="detail-scroll">
    <div className="page-heading"><div><span className="eyebrow">SAGA COMPLETA</span><h1>{saga.title}</h1></div><span>{saga.items.length} películas</span></div>
    <div className="card-grid">{saga.items.map((item,index)=><article key={item.id} className="media-card">
      <div className="poster-button"><button className="poster-detail" onClick={()=>openDetail(item.id)} aria-label={`Ver detalles de ${displayTitle(item)}`}><Poster title={displayTitle(item)} label={`PARTE ${item.sagaPosition||index+1}`} src={item.artworkUrl}/></button><button className="hover-play" onClick={()=>void playMedia(item.id)} title="Reproducir"><Play fill="currentColor"/></button></div>
      <div className="card-copy"><div><button className="title-link" onClick={()=>openDetail(item.id)}>{displayTitle(item)}</button><p>{item.year||'Sin año'}</p></div></div>
    </article>)}</div>
  </div></section></div>;
}

function EmptyLibrary(){return <section className="empty-library"><Library size={42}/><h1>Tu sala está lista</h1><p>Conectá la carpeta predeterminada o elegí otra desde Configuración y después reescaneá.</p></section>}
function MediaRow(props:{title:string;items:MediaSummary[];openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void}){if(!props.items.length)return null;return <section className="media-section"><div className="section-title"><h2>{props.title}</h2><span>{props.items.length}</span></div><CarouselRow label={props.title}>{props.items.map(i=><MediaCard key={i.id} item={i} {...props}/>)}</CarouselRow></section>}
function CatalogPage({title,items,openDetail,setFlag,playMedia}:{title:string;items:MediaSummary[];openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void}){return <div className="content catalog-page"><div className="page-heading"><div><span className="eyebrow">TU BIBLIOTECA</span><h1>{title}</h1></div><span>{items.length} resultados</span></div>{items.length?<div className="card-grid">{items.map(i=><MediaCard key={i.id} item={i} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>)}</div>:<div className="empty-results"><Film/><h2>No hay contenido en esta sección</h2><p>Los elementos aparecerán después del próximo escaneo.</p></div>}</div>}
function SeriesPage({series,search,openSeries}:{series:SeriesSummary[];search:string;openSeries:(series:SeriesSummary)=>void}){const q=search.toLocaleLowerCase();const list=series.filter(s=>s.title.toLocaleLowerCase().includes(q));const chapters=list.reduce((total,item)=>total+item.episodes,0);return <div className="content catalog-page"><div className="page-heading"><div><span className="eyebrow">CAPÍTULOS AGRUPADOS</span><h1>Series</h1></div><span>{list.length} series · {chapters} capítulos</span></div><div className="card-grid">{list.map(s=><SeriesCard key={s.episodeId} series={s} openSeries={openSeries}/>)}</div></div>}

function MediaCard({item,openDetail,setFlag,playMedia,flagPending}:{item:MediaSummary;openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void;flagPending?:boolean}){return <article className="media-card"><div className="poster-button"><button className="poster-detail" onClick={()=>openDetail(item.id)} aria-label={`Ver detalles de ${displayTitle(item)}`}><Poster title={displayTitle(item)} label={flagPending&&item.incomplete?'FALTA FICHA':item.kind==='episode'?'SERIE':quality(item)} src={item.artworkUrl}/></button>{item.progressPercent>0&&<span className="card-progress"><i style={{width:`${item.progressPercent}%`}}/></span>}<button className="hover-play" onClick={()=>void playMedia(item.id)} title="Reproducir"><Play fill="currentColor"/></button></div><div className="card-copy"><div><button className="title-link" onClick={()=>openDetail(item.id)}>{displayTitle(item)}</button><p>{item.year||'Sin año'}{item.kind==='episode'?` · T${item.seasonNumber} E${item.episodeNumber}`:''}</p></div><div className="quick-actions"><button className={item.favorite?'selected':''} title="Favorito" onClick={()=>void setFlag(item,'favorite')}><Heart size={15} fill={item.favorite?'currentColor':'none'}/></button><button className={item.inWatchlist?'selected':''} title="Mi lista" onClick={()=>void setFlag(item,'watchlist')}><Bookmark size={15} fill={item.inWatchlist?'currentColor':'none'}/></button></div></div></article>}
function Poster({title,label,src}:{title:string;label:string;src?:string}){return <div className={`poster ${src?'has-image':''}`} style={{'--poster-hue':hueFor(title)} as React.CSSProperties}>{src&&<img src={assetUrl(src)} alt=""/>}<span className="poster-label">{label}</span>{!src&&<b>{initials(title)}</b>}<small>{title}</small></div>}

function SettingsPage({boot,scan,updating,updateMessage,updateVersion,onRescan,onChoose,onLogout,onCheckUpdates,onInstallUpdate}:{boot:Bootstrap;scan:ScanProgress;updating:boolean;updateMessage:string|null;updateVersion?:string;onRescan:()=>void;onChoose:()=>void;onLogout:()=>void;onCheckUpdates:()=>void;onInstallUpdate:()=>void}){const root=boot.roots.find(r=>r.enabled)||boot.roots[0];return <div className="content settings-page"><div className="page-heading"><div><span className="eyebrow">CINE WANA</span><h1>Configuración</h1></div></div><section className="settings-card compact"><div className="settings-icon"><UserRound/></div><div className="settings-main"><div className="settings-title"><div><h2>Cuenta local</h2><p>Progreso, historial y listas de esta sesión</p></div><span className="root-status online">{boot.activeAccount?.name}</span></div><div className="diagnostic-row"><span>Cuentas creadas</span><b>{boot.accounts.length}</b></div><button className="settings-rescan account-logout" onClick={onLogout}><LogOut/>Cerrar sesión</button></div></section><section className="settings-card compact"><div className="settings-icon"><RefreshCw/></div><div className="settings-main"><div className="settings-title"><div><h2>Actualizaciones</h2><p>GitHub Releases firmado para Windows x64</p></div><span className="root-status">{updateVersion?`v${updateVersion}`:'Manual'}</span></div>{updateMessage&&<div className="update-note">{updateMessage}</div>}<div className="update-actions"><button className="settings-rescan" disabled={updating} onClick={onCheckUpdates}>{updating?<><LoaderCircle className="spin"/>Buscando</>:<><RefreshCw/>Buscar actualizaciones</>}</button>{updateVersion&&<button className="primary settings-rescan" disabled={updating} onClick={onInstallUpdate}><Check/>Instalar versión</button>}</div></div></section><section className="settings-card"><div className="settings-icon"><FolderCog/></div><div className="settings-main"><div className="settings-title"><div><h2>Biblioteca</h2><p>Carpeta activa y lectura recursiva</p></div><span className={`root-status ${root?.status}`}>{root?.status==='online'?'Conectada':root?.status==='scanning'?'Escaneando':'Desconectada'}</span></div><div className="path-box"><code>{root?.localPath||'Sin carpeta configurada'}</code><button onClick={onChoose}>Cambiar carpeta</button></div><div className="settings-stats"><div><small>Último escaneo</small><b>{root?.lastScanAt?new Date(root.lastScanAt).toLocaleString('es-AR'):'Todavía no finalizó'}</b></div><div><small>Subcarpetas</small><b>{root?.recursive?'Incluidas':'Excluidas'}</b></div><div><small>Archivos desconectados</small><b>{root?.disconnectedCount||0}</b></div></div><button className="primary settings-rescan" onClick={onRescan}>{scan.running?<><X/>Cancelar escaneo</>:<><RefreshCw/>Reescanear biblioteca</>}</button></div></section><section className="settings-card compact"><div className="settings-icon"><Film/></div><div className="settings-main"><div className="settings-title"><div><h2>Componentes multimedia</h2><p>Diagnóstico del entorno de desarrollo</p></div></div><div className="diagnostic-row"><span>FFmpeg / ffprobe</span><b className={boot.ffprobeAvailable?'ok':'pending'}>{boot.ffprobeAvailable?<><Check/>Disponible</>:<><CircleAlert/>Pendiente</>}</b></div><div className="diagnostic-row"><span>Reproductor interno + externo</span><b className={boot.playerAvailable?'ok':'pending'}>{boot.playerAvailable?<><Check/>Disponible</>:<><CircleAlert/>No encontrado</>}</b></div></div></section></div>}

function TmdbCreditsCard(){return <div className="content settings-page tmdb-credits"><section className="settings-card compact"><div className="tmdb-logo"><img src={tmdbLogo} alt="The Movie Database (TMDB)"/></div><div className="settings-main"><div className="settings-title"><div><h2>Créditos de datos e imágenes</h2><p>Portadas, fondos y fichas proporcionados por <a href="https://www.themoviedb.org" target="_blank" rel="noreferrer">TMDB</a>.</p></div></div><p className="tmdb-notice">This product uses the TMDB API but is not endorsed or certified by TMDB.</p></div></section></div>}

function IdentificationReviewSettings({reviews,metadataNotice,onResolve,onApplyCandidate,onRetryMetadata}:{reviews:IdentificationReview[];metadataNotice:string|null;onResolve:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>;onApplyCandidate:(mediaId:string,candidate:MediaMetadataCandidate)=>Promise<void>;onRetryMetadata:(mediaId:string)=>Promise<void>}){
  return <div className="content settings-page identification-settings">
    <section className="settings-card">
      <div className="settings-icon"><CircleAlert/></div>
      <div className="settings-main">
        <div className="settings-title"><div><h2>Errores y coincidencias por revisar</h2><p>Títulos dudosos y portadas de TMDB que necesitan tu decisión</p></div><span className={`root-status ${reviews.length?'pending':'online'}`}>{reviews.length} pendientes</span></div>
        {metadataNotice&&<div className="metadata-applied-notice" role="status"><Check/><div><b>{metadataNotice}</b><span>Si esa película también necesita corregir el nombre o el tipo, seguirá apareciendo como pendiente.</span></div></div>}
        {reviews.length===0?<div className="identification-empty"><Check/>No hay películas, episodios ni portadas dudosas.</div>:<div className="identification-review-list">{reviews.map(review=><IdentificationReviewCard key={review.mediaId} review={review} onResolve={onResolve} onApplyCandidate={onApplyCandidate} onRetryMetadata={onRetryMetadata}/>)}</div>}
      </div>
    </section>
  </div>;
}

export function IdentificationReviewCard({review,onResolve,onApplyCandidate,onRetryMetadata,onChanged}:{review:IdentificationReview;onResolve:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>;onApplyCandidate:(mediaId:string,candidate:MediaMetadataCandidate)=>Promise<void>;onRetryMetadata:(mediaId:string)=>Promise<void>;onChanged?:()=>Promise<void>}){
  const [kind,setKind]=useState(review.kind);
  const [title,setTitle]=useState(review.title);
  const [seriesTitle,setSeriesTitle]=useState(review.seriesTitle||'');
  const [seasonNumber,setSeasonNumber]=useState(review.seasonNumber?.toString()||'');
  const [episodeNumber,setEpisodeNumber]=useState(review.episodeNumber?.toString()||'');
  const [saving,setSaving]=useState(false);
  const [rescanning,setRescanning]=useState(false);
  const [revealError,setRevealError]=useState('');
  const [rescanMessage,setRescanMessage]=useState('');
  const [metadataBusy,setMetadataBusy]=useState(false);
  const [metadataMessage,setMetadataMessage]=useState('');
  useEffect(()=>{
    setKind(review.kind);setTitle(review.title);setSeriesTitle(review.seriesTitle||'');
    setSeasonNumber(review.seasonNumber?.toString()||'');setEpisodeNumber(review.episodeNumber?.toString()||'');
  },[review.episodeNumber,review.kind,review.mediaId,review.seasonNumber,review.seriesTitle,review.title]);
  const valid=title.trim().length>0&&(kind==='movie'||(seriesTitle.trim().length>0&&Number(seasonNumber)>0&&Number(episodeNumber)>0));
  const save=async()=>{
    if(!valid)return;
    setSaving(true);
    try{
      await onResolve(review.mediaId,{kind,title:title.trim(),seriesTitle:kind==='episode'?seriesTitle.trim():null,seasonNumber:kind==='episode'?Number(seasonNumber):null,episodeNumber:kind==='episode'?Number(episodeNumber):null});
    }catch(cause){setRescanMessage(`No se pudo guardar la identificación: ${String(cause)}`);}finally{setSaving(false);}
  };
  const reveal=async()=>{
    try{setRevealError('');await invoke('reveal_media_file',{mediaId:review.mediaId});}
    catch(cause){setRevealError(String(cause));}
  };
  const rescanOne=async()=>{
    try{
      setRescanning(true);setRescanMessage('');
      const stillNeedsReview=await invoke<boolean>('rescan_media_item',{mediaId:review.mediaId});
      setRescanMessage(stillNeedsReview?'El nuevo nombre todavia necesita revision.':'Identificacion corregida. Quitando esta alerta...');
      await onChanged?.();
    }catch(cause){setRescanMessage(String(cause));}
    finally{setRescanning(false);}
  };
  const chooseCandidate=async(candidate:MediaMetadataCandidate)=>{try{setMetadataBusy(true);setMetadataMessage('');await onApplyCandidate(review.mediaId,candidate);setMetadataMessage(`Portada aplicada: ${candidate.title}.`);}catch(cause){setMetadataMessage(`No se pudo aplicar la portada: ${String(cause)}`);}finally{setMetadataBusy(false);}};
  const retryMetadata=async()=>{try{setMetadataBusy(true);setMetadataMessage('');await onRetryMetadata(review.mediaId);}catch(cause){setMetadataMessage(`No se pudo buscar en TMDB: ${String(cause)}`);}finally{setMetadataBusy(false);}};
  const hasIssue=review.identificationPending||['ambiguous','not_found','artwork_missing','pending'].includes(review.metadataStatus);
  return <article className="identification-review-card">
    <div className={`identification-warning ${hasIssue?'':'resolved'}`}>{hasIssue?<CircleAlert/>:<Check/>}<div><b>{review.fileName}</b><span>{review.reason}</span>{revealError&&<small>{revealError}</small>}{rescanMessage&&<small>{rescanMessage}</small>}</div><div className="identification-file-actions"><button onClick={()=>void reveal()}><FolderOpen/>Mostrar archivo</button><button disabled={rescanning} onClick={()=>void rescanOne()}>{rescanning?<LoaderCircle className="spin"/>:<RefreshCw/>}Reescanear este archivo</button></div></div>
    <div className="identification-kind"><button className={kind==='movie'?'selected':''} onClick={()=>setKind('movie')}>Pelicula</button><button className={kind==='episode'?'selected':''} onClick={()=>setKind('episode')}>Serie / episodio</button></div>
    {metadataMessage&&<div className={`metadata-card-message ${metadataMessage.startsWith('Portada aplicada')?'success':'error'}`} role="status">{metadataMessage.startsWith('Portada aplicada')?<Check/>:<CircleAlert/>}<span>{metadataMessage}</span></div>}
    {review.metadataCandidates.length>0&&<div className="metadata-candidates">{review.metadataCandidates.map(candidate=><button key={candidate.id} aria-label={`Usar portada ${candidate.title}`} disabled={metadataBusy} onClick={()=>void chooseCandidate(candidate)}>{candidate.posterUrl&&<img src={candidate.posterUrl} alt={`Portada de ${candidate.title}`}/>}<b>{candidate.title}</b><span>{candidate.year||'Sin año'} · TMDB</span>{candidate.description&&<small>{candidate.description}</small>}<strong className="metadata-candidate-action"><Check/>Usar esta portada</strong></button>)}</div>}
    <div className="identification-fields">
      <label><span>{kind==='movie'?'Titulo de la pelicula':'Titulo del episodio'}</span><input value={title} onChange={event=>setTitle(event.target.value)}/></label>
      {kind==='episode'&&<><label><span>Serie</span><input value={seriesTitle} onChange={event=>setSeriesTitle(event.target.value)}/></label><label><span>Temporada</span><input inputMode="numeric" value={seasonNumber} onChange={event=>setSeasonNumber(event.target.value.replace(/\D/g,''))}/></label><label><span>Episodio</span><input inputMode="numeric" value={episodeNumber} onChange={event=>setEpisodeNumber(event.target.value.replace(/\D/g,''))}/></label></>}
    </div>
    <div className="identification-review-actions"><button className="primary identification-save" disabled={!valid||saving||metadataBusy} onClick={()=>void save()}>{saving?<LoaderCircle className="spin"/>:<Save/>}Guardar y buscar portada</button><button className="settings-rescan" disabled={metadataBusy} onClick={()=>void retryMetadata()}>{metadataBusy?<LoaderCircle className="spin"/>:<RefreshCw/>}Volver a buscar en TMDB</button></div>
  </article>
}

/* Las fichas viejas no tienen fotos ni sagas porque se importaron antes de que existieran. Este
   boton las repasa una por una: tarda, pero se deja corriendo y se puede cortar cuando sea. */
function MetadataRefreshCard({progress,onStart,onCancel}:{progress:MetadataRefreshProgress|null;onStart:()=>void;onCancel:()=>void}){
  const running=Boolean(progress?.running);
  const percent=progress&&progress.total>0?Math.round(progress.processed/progress.total*100):0;
  return <div className="content settings-page"><section className="settings-card">
    <div className="settings-icon"><Users/></div>
    <div className="settings-main">
      <div className="settings-title"><div><h2>Actualizar fichas</h2><p>Vuelve a pedirle a TMDB los datos de cada título: actores con foto, dirección, guion y sagas</p></div><span className={`root-status ${running?'scanning':''}`}>{running?'En curso':'Manual'}</span></div>
      {progress&&(running||progress.finished)&&<>
        <div className="refresh-meter"><i style={{width:`${percent}%`}}/></div>
        <div className="refresh-stats">
          <span>{progress.processed} de {progress.total}</span>
          <span>{progress.updated} actualizadas</span>
          {progress.failed>0&&<span className="failed">{progress.failed} sin coincidencia</span>}
          {running&&progress.currentTitle&&<span className="current">{progress.currentTitle}</span>}
          {progress.finished&&!running&&<span className="done"><Check size={13}/>{progress.cancelRequested?'Cancelado':'Terminado'}</span>}
        </div>
      </>}
      <p className="pending-note"><CircleAlert size={15}/>Va de a una película por vez para no saturar a TMDB, así que tarda un rato largo. Podés seguir usando CINE WANA mientras corre, y cortarlo cuando quieras sin perder lo ya hecho.</p>
      <button className={`primary settings-rescan ${running?'':''}`} onClick={()=>running?onCancel():onStart()}>{running?<><X/>Cancelar</>:<><RefreshCw/>Actualizar todas las fichas</>}</button>
    </div>
  </section></div>;
}

/* Categorías propias: se crean acá y se llenan desde la ficha de cada película o serie, que es
   donde uno está mirando cuando decide que algo va a una lista. */
function CustomCategoriesCard({categories,onCreate,onRename,onDelete}:{categories:CustomCategory[];onCreate:(label:string)=>Promise<void>;onRename:(id:string,label:string)=>Promise<void>;onDelete:(id:string)=>Promise<void>}){
  const [label,setLabel]=useState('');
  const [editing,setEditing]=useState<string|null>(null);
  const [draft,setDraft]=useState('');
  const [busy,setBusy]=useState(false);
  const create=async()=>{if(!label.trim())return;try{setBusy(true);await onCreate(label.trim());setLabel('');}finally{setBusy(false);}};
  const rename=async(id:string)=>{if(!draft.trim()){setEditing(null);return;}try{setBusy(true);await onRename(id,draft.trim());setEditing(null);}finally{setBusy(false);}};
  return <div className="content settings-page"><section className="settings-card">
    <div className="settings-icon"><Bookmark/></div>
    <div className="settings-main">
      <div className="settings-title"><div><h2>Categorías propias</h2><p>Las armás vos y les ponés lo que quieras, sin depender de los géneros de TMDB</p></div><span className="root-status">{categories.length} creadas</span></div>
      <div className="custom-create">
        <input value={label} maxLength={40} placeholder="Nombre de la categoría" onChange={event=>setLabel(event.target.value)} onKeyDown={event=>{if(event.key==='Enter')void create();}}/>
        <button className="primary settings-rescan" disabled={busy||!label.trim()} onClick={()=>void create()}><Check/>Crear</button>
      </div>
      {categories.length>0&&<ul className="custom-list">
        {categories.map(category=><li key={category.id}>
          <Bookmark size={15}/>
          {editing===category.id
            ?<input autoFocus value={draft} maxLength={40} onChange={event=>setDraft(event.target.value)} onKeyDown={event=>{if(event.key==='Enter')void rename(category.id);if(event.key==='Escape')setEditing(null);}} onBlur={()=>void rename(category.id)}/>
            :<b>{category.label}</b>}
          <span className="category-order-count">{category.items.length+category.series.length}</span>
          <button title="Cambiar el nombre" disabled={busy} onClick={()=>{setEditing(category.id);setDraft(category.label);}}><Pencil size={15}/></button>
          <button title="Borrar la categoría" className="danger" disabled={busy} onClick={()=>void onDelete(category.id)}><X size={15}/></button>
        </li>)}
      </ul>}
      <p className="pending-note"><CircleAlert size={15}/>Para agregarle películas o series, abrí la ficha del título y tocá la categoría en <b>Mis categorías</b>. Borrar una categoría no borra ni toca ninguna película.</p>
    </div>
  </section></div>;
}

/* Los interruptores que aparecen en la ficha de un título. */
function CustomCategoryPicker({categories,mediaId,seriesTitle,onToggle}:{categories:CustomCategory[];mediaId?:string;seriesTitle?:string;onToggle:(id:string,member:boolean)=>Promise<void>}){
  if(!categories.length)return null;
  const belongs=(category:CustomCategory)=>seriesTitle?category.series.includes(seriesTitle):Boolean(mediaId&&category.items.includes(mediaId));
  return <div className="custom-picker">
    <h3><Bookmark size={13}/>Mis categorías</h3>
    <div>{categories.map(category=>{const member=belongs(category);return <button key={category.id} className={member?'selected':''} onClick={()=>void onToggle(category.id,!member)}>{member?<Check size={12}/>:<Plus size={12}/>}{category.label}</button>;})}</div>
  </div>;
}

/* Las dos tiras se dibujan con las categorías reales de la biblioteca, así se elige mirando lo que
   uno va a ver de verdad y no una muestra inventada. */
function CategoryStyleCard({categories,style,onSelect,carouselDrag,onToggleDrag}:{categories:CategoryOption[];style:CategoryStyle;onSelect:(style:CategoryStyle)=>Promise<void>;carouselDrag:boolean;onToggleDrag:(enabled:boolean)=>Promise<void>}){
  const sample=categories.filter(row=>!row.hidden).slice(0,6);
  const options:Array<[CategoryStyle,string,string]>=[['gold','Dorada','Todo el renglón en dorado, con la elegida encendida.'],['dark','Sobria','Fondo gris oscuro y sólo los íconos en dorado.']];
  return <div className="content settings-page"><section className="settings-card">
    <div className="settings-icon"><Palette/></div>
    <div className="settings-main">
      <div className="settings-title"><div><h2>Estilo de la tira</h2><p>Cómo se ven los nombres de categoría abajo de la portada</p></div><span className="root-status">{style==='gold'?'Dorada':'Sobria'}</span></div>
      <label className="style-toggle">
        <input type="checkbox" checked={carouselDrag} onChange={event=>void onToggleDrag(event.target.checked)}/>
        <span><b>Arrastrar las filas con el mouse</b><small>Con esto activado el puntero se convierte en una manito sobre las portadas y sirve para correr la fila de costado. Desactivado, las filas se mueven solo con las flechas y hacer clic en una película es más directo.</small></span>
      </label>
      <div className="style-options">
        {options.map(([id,label,hint])=><button key={id} className={`style-option ${style===id?'selected':''}`} onClick={()=>void onSelect(id)}>
          <span className="style-option-head"><b>{label}</b>{style===id?<span className="style-option-active"><Check size={12}/>En uso</span>:<span className="style-option-pick">Usar esta</span>}</span>
          <nav className={`category-strip ${id}`} aria-hidden="true">
            <span className="active"><LayoutGrid size={15}/>Todas</span>
            {sample.map(row=>{const Icon=categoryIcon(row);return <span key={row.id} className={`${row.kind==='uncategorized'?'pending':''} ${row.kind==='sagas'?'saga':''}`}><Icon size={15}/>{row.label}<i>{row.count}</i></span>;})}
          </nav>
          <small>{hint}</small>
        </button>)}
      </div>
    </div>
  </section></div>;
}

/* Una sola lista manda sobre la tira de arriba y sobre las filas: ordenar dos veces lo mismo sólo
   consigue que la tira diga una cosa y las filas otra. */
function CategoryOrderCard({categories,style,onSave}:{categories:CategoryOption[];style:CategoryStyle;onSave:(preferences:CategoryPreference[])=>Promise<void>}){
  const [list,setList]=useState(categories);
  const [dragId,setDragId]=useState<string|null>(null);
  const [offset,setOffset]=useState(0);
  const [saving,setSaving]=useState(false);
  const [notice,setNotice]=useState<string|null>(null);
  const grab=useRef({id:'',y:0,moved:false});
  useEffect(()=>{setList(categories);},[categories]);
  const save=async(next:CategoryOption[],message:string)=>{try{setSaving(true);await onSave(next.map(row=>({id:row.id,hidden:row.hidden})));setNotice(message);}finally{setSaving(false);}};
  const toggle=(row:CategoryOption)=>{const next=list.map(entry=>entry.id===row.id?{...entry,hidden:!entry.hidden}:entry);setList(next);void save(next,row.hidden?`${row.label} vuelve al inicio.`:`${row.label} queda apagada. Sigue acá por si la querés de vuelta.`);};
  const byCount=()=>{const next=[...list].sort((a,b)=>(a.kind==='uncategorized'?1:0)-(b.kind==='uncategorized'?1:0)||b.count-a.count||a.label.localeCompare(b.label));setList(next);void save(next,'Ordenadas de la más grande a la más chica.');};
  /* Se agarra la fila y se la lleva: mientras el dedo o el mouse pasan por encima de otra, las dos
     cambian de lugar en el momento, y al soltar se guarda solo. */
  const startDrag=(event:React.PointerEvent<HTMLLIElement>,row:CategoryOption)=>{
    if(event.button!==0&&event.pointerType==='mouse')return;
    event.currentTarget.setPointerCapture(event.pointerId);
    grab.current={id:row.id,y:event.clientY,moved:false};
    setDragId(row.id);setOffset(0);setNotice(null);
  };
  const moveDrag=(event:React.PointerEvent<HTMLLIElement>)=>{
    if(grab.current.id!==dragId||!dragId)return;
    const over=(document.elementFromPoint(event.clientX,event.clientY) as HTMLElement|null)?.closest('[data-order-id]')?.getAttribute('data-order-id');
    if(over&&over!==dragId){
      grab.current.y=event.clientY;grab.current.moved=true;
      setOffset(0);
      setList(current=>{
        const from=current.findIndex(entry=>entry.id===dragId);
        const to=current.findIndex(entry=>entry.id===over);
        if(from<0||to<0)return current;
        const next=[...current];const [row]=next.splice(from,1);next.splice(to,0,row);
        return next;
      });
      return;
    }
    setOffset(event.clientY-grab.current.y);
  };
  const endDrag=(event:React.PointerEvent<HTMLLIElement>)=>{
    if(!dragId)return;
    try{event.currentTarget.releasePointerCapture(event.pointerId);}catch{/* El navegador pudo haberlo soltado antes. */}
    const moved=grab.current.moved;
    grab.current={id:'',y:0,moved:false};
    setDragId(null);setOffset(0);
    if(moved)void save(list,'Listo, ese es tu orden.');
  };
  const visible=list.filter(row=>!row.hidden);
  return <div className="content settings-page"><section className="settings-card">
    <div className="settings-icon"><ArrowDownUp/></div>
    <div className="settings-main">
      <div className="settings-title"><div><h2>Orden de categorías</h2><p>Agarrá una y llevala a donde quieras. Se guarda sola, y vale para la tira de arriba y para las filas del inicio.</p></div><span className="root-status">Tu cuenta</span></div>
      {notice&&<div className="update-note">{notice}</div>}
      <ol className="category-order">
        {list.map((row,index)=><li key={row.id} data-order-id={row.id}
          className={`${dragId===row.id?'dragging':''} ${row.hidden?'hidden-row':''} ${row.kind==='uncategorized'?'pending':''}`}
          style={dragId===row.id?{transform:`translateY(${offset}px)`}:undefined}
          onPointerDown={event=>startDrag(event,row)} onPointerMove={moveDrag} onPointerUp={endDrag} onPointerCancel={endDrag}>
          <GripVertical size={15}/>
          <span className="category-order-index">{index+1}</span>
          <b>{row.label}</b>
          <span className="category-order-count">{row.count}</span>
          <button title={row.hidden?'Mostrar en el inicio':'Ocultar del inicio'} className={row.hidden?'':'on'} onPointerDown={event=>event.stopPropagation()} onClick={()=>toggle(row)}>{row.hidden?<EyeOff size={15}/>:<Eye size={15}/>}</button>
        </li>)}
      </ol>
      {!list.length&&<p className="pending-note">Todavía no hay categorías. Van a aparecer después del próximo escaneo.</p>}
      <div className="category-order-actions">
        <button className="settings-rescan" disabled={saving} onClick={byCount}><ArrowDownUp/>Ordenar por cantidad</button>
        <button className="settings-rescan" disabled={saving} onClick={()=>{setList(categories);void save([],'Orden restablecido: ciencia ficción primero.');}}><RotateCcw/>Restablecer</button>
      </div>
      <div className="category-order-preview">
        <span className="eyebrow">ASÍ QUEDA EN EL INICIO</span>
        <nav className={`category-strip ${style}`} aria-hidden="true"><span className="active"><LayoutGrid size={15}/>Todas</span>{visible.map(row=>{const Icon=categoryIcon(row);return <span key={row.id} className={`${row.kind==='uncategorized'?'pending':''} ${row.kind==='sagas'?'saga':''}`}><Icon size={15}/>{row.label}<i>{row.count}</i></span>;})}</nav>
      </div>
    </div>
  </section></div>;
}

type SettingsProps={boot:Bootstrap;scan:ScanProgress;updating:boolean;updateMessage:string|null;updateVersion?:string;metadataNotice:string|null;onRescan:()=>void;onChoose:()=>void;onLogout:()=>void;onCheckUpdates:()=>void;onInstallUpdate:()=>void;onResolveIdentification:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>;onApplyMetadataCandidate:(mediaId:string,candidate:MediaMetadataCandidate)=>Promise<void>;onRefreshMetadata:(mediaId:string)=>Promise<void>;categoryOptions:CategoryOption[];onSaveCategories:(preferences:CategoryPreference[])=>Promise<void>;categoryStyle:CategoryStyle;onSaveCategoryStyle:(style:CategoryStyle)=>Promise<void>;carouselDrag:boolean;onToggleCarouselDrag:(enabled:boolean)=>Promise<void>;metadataRefresh:MetadataRefreshProgress|null;onStartRefresh:()=>void;onCancelRefresh:()=>void;customCategories:CustomCategory[];onCreateCategory:(label:string)=>Promise<void>;onRenameCategory:(id:string,label:string)=>Promise<void>;onDeleteCategory:(id:string)=>Promise<void>};
function RemoteSettingsPage(props:SettingsProps&{remote:RemoteStatus|null;remoteBusy:boolean;runRemoteAction:(command:string,args?:Record<string,unknown>)=>Promise<void>}){
  const {remote,remoteBusy,runRemoteAction,onResolveIdentification,onApplyMetadataCandidate,onRefreshMetadata,categoryOptions,onSaveCategories,categoryStyle,onSaveCategoryStyle,carouselDrag,onToggleCarouselDrag,metadataRefresh,onStartRefresh,onCancelRefresh,customCategories,onCreateCategory,onRenameCategory,onDeleteCategory,...settings}=props;
  const copyUrl=async()=>{if(remote?.pairing?.url)await navigator.clipboard.writeText(remote.pairing.url);else if(remote?.url)await navigator.clipboard.writeText(remote.url);};
  return <div className="cw-shared-settings"><SettingsPage {...settings}/><CategoryStyleCard categories={categoryOptions} style={categoryStyle} onSelect={onSaveCategoryStyle} carouselDrag={carouselDrag} onToggleDrag={onToggleCarouselDrag}/><MetadataRefreshCard progress={metadataRefresh} onStart={onStartRefresh} onCancel={onCancelRefresh}/><CustomCategoriesCard categories={customCategories} onCreate={onCreateCategory} onRename={onRenameCategory} onDelete={onDeleteCategory}/><CategoryOrderCard categories={categoryOptions} style={categoryStyle} onSave={onSaveCategories}/><TmdbCreditsCard/><IdentificationReviewSettings reviews={props.boot.identificationReviews} metadataNotice={props.metadataNotice} onResolve={onResolveIdentification} onApplyCandidate={onApplyMetadataCandidate} onRetryMetadata={onRefreshMetadata}/><div className="content settings-page remote-settings-section"><section className="settings-card remote-settings-card"><div className="settings-icon"><Radio/></div><div className="settings-main">
    <div className="settings-title"><div><h2>Control remoto</h2><p>Vinculación privada desde la misma red Wi‑Fi</p></div><span className={`root-status ${remote?.enabled?'online':''}`}>{remote?.enabled?'Activo':'Desactivado'}</span></div>
    {!remote?.assetRootReady&&<div className="remote-notice"><CircleAlert/>La interfaz móvil todavía no está compilada. El botón Activar la compilará antes de la prueba.</div>}
    {remote?.error&&<div className="remote-notice error"><CircleAlert/>{remote.error}</div>}
    <div className="remote-actions"><button className={remote?.enabled?'settings-rescan':'primary settings-rescan'} disabled={remoteBusy} onClick={()=>void runRemoteAction(remote?.enabled?'remote_stop':'remote_start')}>{remoteBusy?<LoaderCircle className="spin"/>:remote?.enabled?<X/>:<Wifi/>}{remote?.enabled?'Desactivar':'Activar control remoto'}</button><label className="remote-autostart"><span><b>Siempre activo</b><small>Se inicia al abrir CINE WANA</small></span><input type="checkbox" role="switch" aria-label="Activar siempre el control remoto" checked={Boolean(remote?.autoStart)} disabled={remoteBusy||!remote} onChange={event=>void runRemoteAction('remote_set_auto_start',{enabled:event.target.checked})}/><i aria-hidden="true"/></label>{remote?.enabled&&<button className="settings-rescan" disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_create_pairing')}><QrCode/>{remote.pairing?'Renovar QR':'Mostrar QR'}</button>}</div>
    {remote?.enabled&&<div className="remote-address"><div><small>Dirección local</small><code>{remote.url}</code></div><button title="Copiar dirección" onClick={()=>void copyUrl()}><Copy/></button></div>}
    {remote?.pairing&&<div className="pairing-panel"><img src={remote.pairing.qrDataUrl} alt="QR para vincular el teléfono"/><div><span className="eyebrow">ESCANEÁ CON EL TELÉFONO</span><h3>Código {remote.pairing.code}</h3><p>Vence {new Date(remote.pairing.expiresAt).toLocaleTimeString('es-AR',{hour:'2-digit',minute:'2-digit'})}. También podés copiar la URL completa.</p><button onClick={()=>void copyUrl()}><Copy/>Copiar enlace</button></div></div>}
    {remote?.pending.map(request=><div className="pair-request" key={request.id}><Smartphone/><div><b>{request.deviceName}</b><small>Solicita vinculación</small></div><button className="approve" disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_approve_pairing',{requestId:request.id})}><ShieldCheck/>Aprobar</button><button disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_reject_pairing',{requestId:request.id})}><X/>Rechazar</button></div>)}
    {remote&&remote.devices.length>0&&<div className="paired-devices"><h3>Dispositivos vinculados</h3>{remote.devices.map(device=><div key={device.id}><Smartphone/><span><b>{device.name}</b><small>{device.lastSeenAt?`Última conexión ${new Date(device.lastSeenAt).toLocaleString('es-AR')}`:'Todavía no se conectó'}</small></span><button disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_revoke_device',{deviceId:device.id})}>Desvincular</button></div>)}</div>}
    {remote?.enabled&&!remote.secureContext&&<div className="remote-security-note"><ShieldCheck/><span><b>Modo de prueba local</b>El control funciona por Wi‑Fi. La instalación offline como PWA se habilitará al incorporar HTTPS local confiable antes del instalador.</span></div>}
  </div></section></div></div>;
}

const genrePresets=['Acción','Aventura','Animación','Ciencia ficción','Comedia','Documental','Drama','Romance','Suspenso','Terror'];
const parseList=(value:string)=>Array.from(new Set(value.split(',').map(part=>part.trim()).filter(Boolean)));
const addTag=(value:string,tag:string)=>parseList(`${value},${tag}`).join(', ');
const detailToForm=(detail:MediaDetail)=>({title:displayTitle(detail),year:detail.year?.toString()||'',overview:detail.overview||'',genres:detail.genres.join(', '),cast:detail.cast.join(', '),posterPath:'',backdropPath:''});
const metadataLabel=(status:string)=>status==='imported'?'TMDB importado':status==='ambiguous'?'Elegir portada':status==='not_found'?'Revisar título':status==='artwork_missing'?'Falta portada':'Información pendiente';
const metadataSourceLabel=(url:string)=>url.includes('themoviedb.org')?'TMDB':'Wikipedia (anterior)';
const detailReview=(detail:MediaDetail):IdentificationReview=>({
  mediaId:detail.id,fileName:detail.fileName,kind:detail.kind,title:detail.title,
  seriesTitle:detail.seriesTitle,seasonNumber:detail.seasonNumber,episodeNumber:detail.episodeNumber,
  reason:detail.metadataStatus==='imported'?'No hay errores detectados. Igualmente podés cambiar la identificación o las imágenes.':detail.metadataStatus==='ambiguous'?'TMDB encontró varias coincidencias posibles. Elegí la correcta.':detail.metadataStatus==='not_found'?'TMDB no encontró una coincidencia segura. Corregí el título o volvé a buscar.':detail.metadataStatus==='artwork_missing'?'La portada guardada no está disponible. Elegí otra coincidencia o una imagen propia.':'La información externa todavía está pendiente.',
  identificationPending:false,metadataStatus:detail.metadataStatus,metadataCandidates:detail.metadataCandidates,
});
const reviewHasIssue=(review:IdentificationReview)=>review.identificationPending||['ambiguous','not_found','artwork_missing','pending'].includes(review.metadataStatus);

/* El reparto va primero porque es lo que se busca al abrir una ficha; direccion y guion despues.
   Sin foto se dibujan las iniciales: una silueta gris no dice nada. */
function PeopleBlock({people,fallback,status,onReview}:{people:MediaPerson[];fallback:string[];status:string;onReview:()=>void}){
  const [zoom,setZoom]=useState<{person:MediaPerson;note?:string}|null>(null);
  const openZoom=(person:MediaPerson,note?:string)=>setZoom({person,note});
  const crew=people.filter(person=>person.role!=='actor');
  const cast=people.filter(person=>person.role==='actor');
  if(!people.length){
    const problema=status==='ambiguous'?'varias':status==='not_found'?'ninguna':'';
    return <div className="cast-block">
      <h3><Users size={15}/> Reparto</h3>
      {fallback.length>0&&<p>{fallback.join(', ')}</p>}
      {problema
        ?<button className="cast-problem" onClick={onReview}>
          <CircleAlert size={15}/>
          <span><b>{problema==='varias'?'Hay varias películas posibles':'No se encontró esta película'}</b>
          <small>{problema==='varias'?'TMDB no puede elegir sola. Abrí Revisión y marcá la correcta para traer el reparto.':'El nombre del archivo no alcanza. Abrí Revisión y corregí el título para traer el reparto.'}</small></span>
          <ChevronRight size={15}/>
        </button>
        :<small className="cast-hint">Esta ficha se guardó antes de que existieran las fotos. Actualizala desde Configuración para verlas.</small>}
    </div>;
  }
  return <div className="cast-block">
    {cast.length>0&&<><h3><Users size={15}/> Reparto</h3>
      <div className="people-row">{cast.map(person=><PersonCard key={person.name} person={person} note={person.character} onOpen={openZoom}/>)}</div></>}
    {crew.length>0&&<><h3 className={cast.length?'spaced':''}><Clapperboard size={15}/> Dirección y guion</h3>
      <div className="people-row crew">{crew.map(person=><PersonCard key={`${person.role}-${person.name}`} person={person} note={person.role==='director'?'Dirección':'Guion'} onOpen={openZoom}/>)}</div></>}
    {zoom&&<PersonZoom person={zoom.person} note={zoom.note} close={()=>setZoom(null)}/>}
  </div>;
}

const personInitials=(name:string)=>name.split(/\s+/).filter(Boolean).slice(0,2).map(part=>part[0]).join('').toUpperCase();

function PersonCard({person,note,onOpen}:{person:MediaPerson;note?:string;onOpen:(person:MediaPerson,note?:string)=>void}){
  return <button type="button" className="person-card" aria-label={`Ver foto de ${person.name}`} title={note?`${person.name} — ${note}`:person.name} onClick={()=>onOpen(person,note)}>
    <span className="person-photo" style={{'--person-hue':hueFor(person.name)} as React.CSSProperties}>
      {person.photoUrl?<img src={assetUrl(person.photoUrl)} alt="" loading="lazy"/>:<span>{personInitials(person.name)}</span>}
    </span>
    <span className="person-caption"><b>{person.name}</b>{note&&<small>{note}</small>}</span>
  </button>;
}

/* Visor centrado para la persona: conserva el contexto de la ficha y se cierra desde cualquier tamaño de ventana. */
function PersonZoom({person,note,close}:{person:MediaPerson;note?:string;close:()=>void}){
  const closeButtonRef=useRef<HTMLButtonElement|null>(null);
  useEffect(()=>{
    closeButtonRef.current?.focus();
    const onKey=(event:KeyboardEvent)=>{if(event.key==='Escape'){event.stopImmediatePropagation();close();}};
    window.addEventListener('keydown',onKey);
    return()=>window.removeEventListener('keydown',onKey);
  },[close]);
  return <div className="person-viewer" role="dialog" aria-modal="true" aria-label={`Foto de ${person.name}`} onMouseDown={event=>{if(event.target===event.currentTarget)close();}}>
    <figure className="person-viewer-card">
      <div className="person-viewer-photo" style={{'--person-hue':hueFor(person.name)} as React.CSSProperties}>
        <button ref={closeButtonRef} type="button" className="person-viewer-close" onClick={close} aria-label="Cerrar foto del actor" title="Cerrar">
          <X size={21} strokeWidth={1.9} aria-hidden="true"/>
        </button>
        {person.photoUrl?<img src={assetUrl(person.photoUrl)} alt={person.name}/>:<span>{personInitials(person.name)}</span>}
      </div>
      <figcaption className="person-viewer-copy">
        <span>{person.role==='director'?'Dirección':person.role==='writer'?'Guion':'Reparto'}</span>
        <h2>{person.name}</h2>
        {note&&<p>{note}</p>}
        <small>Presioná Esc o hacé clic fuera de la foto para cerrar.</small>
      </figcaption>
    </figure>
  </div>;
}

function DetailModal({detail,review,close,setFlag,playMedia,openExternalMedia,onSaveMetadata,onRefreshMetadata,onApplyCandidate,onResolveIdentification,openDetail,metadataLoading,customCategories,onToggleCategory}:{detail:MediaDetail;review?:IdentificationReview;close:()=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void;openExternalMedia:(id:string)=>void;onSaveMetadata:(id:string,metadata:MediaMetadataUpdate)=>Promise<void>;onRefreshMetadata:(id:string)=>Promise<void>;onApplyCandidate:(id:string,candidate:MediaMetadataCandidate)=>Promise<void>;onResolveIdentification:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>;openDetail:(id:string)=>Promise<void>;metadataLoading:boolean;customCategories:CustomCategory[];onToggleCategory:(id:string,member:boolean)=>Promise<void>}){
  const [editing,setEditing]=useState(false);
  const [reviewOpen,setReviewOpen]=useState(false);
  const [form,setForm]=useState(()=>detailToForm(detail));
  useEffect(()=>{setEditing(false);setForm(detailToForm(detail));},[detail]);
  useEffect(()=>{
    const onKey=(event:KeyboardEvent)=>{
      if(event.key==='Escape'&&!document.querySelector('.person-viewer,.media-review-backdrop'))close();
    };
    window.addEventListener('keydown',onKey);
    return()=>window.removeEventListener('keydown',onKey);
  },[close]);
  const chooseImage=async(field:'posterPath'|'backdropPath')=>{
    const selected=await open({multiple:false,directory:false,title:field==='posterPath'?'Elegir portada':'Elegir fondo',filters:[{name:'Imagen',extensions:['png','jpg','jpeg','webp']}]});
    if(typeof selected==='string')setForm(prev=>({...prev,[field]:selected}));
  };
  const save=async()=>{try{await onSaveMetadata(detail.id,{title:form.title,year:form.year.trim()?Number(form.year):null,overview:form.overview.trim()||null,genres:parseList(form.genres),cast:parseList(form.cast),posterPath:form.posterPath||null,backdropPath:form.backdropPath||null});setEditing(false);}catch{/* El aviso global conserva el error y el formulario queda abierto. */}};
  const currentReview=review??detailReview(detail);
  const hasReviewIssue=reviewHasIssue(currentReview);
  return <><div className="modal-backdrop detail-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)close();}}>
    <section className="detail-modal detail-modal-expanded" role="dialog" aria-modal="true" aria-label={`Detalles de ${displayTitle(detail)}`} style={{'--detail-backdrop':detail.backdropUrl?`url("${assetUrl(detail.backdropUrl)}")`:'none'} as React.CSSProperties}>
      <button type="button" className="modal-close detail-close" onClick={close} aria-label="Cerrar detalles" title="Cerrar detalles">
        <X size={24} strokeWidth={1.8} aria-hidden="true"/>
      </button>
      <div className="detail-scroll">
      <div className="detail-art"><Poster title={displayTitle(detail)} label={detail.kind==='episode'?'SERIE':quality(detail)} src={detail.artworkUrl}/>{editing&&<div className="art-buttons"><button onClick={()=>void chooseImage('posterPath')}><ImagePlus/>Portada</button><button onClick={()=>void chooseImage('backdropPath')}><ImagePlus/>Fondo</button></div>}</div>
      <div className="detail-copy">
        <div className="detail-headline"><span className="eyebrow">{detail.kind==='episode'?`TEMPORADA ${detail.seasonNumber} · EPISODIO ${detail.episodeNumber}`:'PELÍCULA'}</span><div><button disabled={metadataLoading} onClick={()=>void onRefreshMetadata(detail.id).catch(()=>{})}>{metadataLoading?<LoaderCircle className="spin"/>:<RefreshCw/>}Volver a buscar información</button><button onClick={()=>setEditing(value=>!value)}><Pencil/>Editar datos</button></div></div>
        {editing?<div className="metadata-editor">
          <label><span>Título</span><input value={form.title} onChange={e=>setForm({...form,title:e.target.value})}/></label>
          <label><span>Año</span><input value={form.year} onChange={e=>setForm({...form,year:e.target.value.replace(/\D/g,'').slice(0,4)})}/></label>
          <label className="wide"><span>Descripción</span><textarea value={form.overview} onChange={e=>setForm({...form,overview:e.target.value})}/></label>
          <label className="wide"><span>Géneros</span><input value={form.genres} onChange={e=>setForm({...form,genres:e.target.value})}/><small>{genrePresets.map(genre=><button type="button" key={genre} onClick={()=>setForm(prev=>({...prev,genres:addTag(prev.genres,genre)}))}>{genre}</button>)}</small></label>
          <label className="wide"><span>Actores</span><input value={form.cast} onChange={e=>setForm({...form,cast:e.target.value})}/></label>
          <div className="editor-actions"><button onClick={()=>{setEditing(false);setForm(detailToForm(detail));}}>Cancelar</button><button className="primary" onClick={()=>void save()}><Save/>Guardar</button></div>
        </div>:<>
          <h1>{displayTitle(detail)}</h1>
          <div className="detail-meta"><span>{detail.year||'Sin año'}</span><span>{quality(detail)}</span>{detail.technical.hdrType&&<span>{detail.technical.hdrType}</span>}{detail.runtimeMs&&<span>{formatDuration(detail.runtimeMs)}</span>}{detail.manualMetadata&&<span>EDITADO</span>}<span>{metadataLabel(detail.metadataStatus)}</span></div>
          <div className="genre-pills">{detail.genres.length?detail.genres.map(genre=><span key={genre}><Tags size={12}/>{genre}</span>):<span><Tags size={12}/>Sin género</span>}</div>
          <CustomCategoryPicker categories={customCategories} mediaId={detail.id} onToggle={onToggleCategory}/>
          <p className="overview">{detail.overview||'Todavía no hay descripción. Podés editar esta ficha y agregar la sinopsis, género, actores y portada.'}</p>
          <button className={`detail-review-button ${hasReviewIssue?'pending':''}`} onClick={()=>setReviewOpen(true)}>{hasReviewIssue?<CircleAlert/>:<ShieldCheck/>}<span><b>Revisión</b><small>{hasReviewIssue?currentReview.reason:'Identidad, portada y archivo'}</small></span></button>
          <PeopleBlock people={detail.people} fallback={detail.cast} status={detail.metadataStatus} onReview={()=>setReviewOpen(true)}/>
          <div className="metadata-source"><h3>Información externa</h3>{detail.metadataSourceUrl?<p>Fuente: <a href={detail.metadataSourceUrl} target="_blank" rel="noreferrer">{metadataSourceLabel(detail.metadataSourceUrl)}</a>{detail.metadataImportedAt?` · ${new Date(detail.metadataImportedAt).toLocaleDateString('es-AR')}`:''}</p>:<p>{detail.metadataStatus==='ambiguous'?'TMDB encontró varias posibilidades. Abrí Revisión para elegir la correcta.':detail.metadataStatus==='not_found'?'TMDB no encontró una coincidencia segura. Abrí Revisión para corregirla.':'Todavía no hay una fuente externa guardada.'}</p>}</div>
        </>}
        <div className="detail-actions"><button className="primary" onClick={()=>void playMedia(detail.id)}><Play fill="currentColor"/>Reproducir en CINE WANA</button><button onClick={()=>void openExternalMedia(detail.id)}>Abrir externo</button><button className={detail.inWatchlist?'selected':''} onClick={()=>void setFlag(detail,'watchlist')}><Bookmark/>Mi lista</button><button className={detail.favorite?'selected':''} onClick={()=>void setFlag(detail,'favorite')}><Heart/>Favorito</button></div>
        <div className="technical"><h3>Información técnica</h3><dl><div><dt>Archivo</dt><dd>{detail.fileName}</dd></div><div><dt>Contenedor</dt><dd>{detail.technical.container||'Pendiente de ffprobe'}</dd></div><div><dt>Video</dt><dd>{detail.technical.videoCodec||'Sin analizar'}</dd></div><div><dt>Audio</dt><dd>{detail.technical.audioCodec||'Sin analizar'}</dd></div><div><dt>Subtítulos externos</dt><dd>{detail.tracks.filter(t=>t.external).length}</dd></div></dl></div>
        {detail.recommendations.length>0&&<section className="recommendations"><h3>Más para ver</h3><div>{detail.recommendations.map(item=><button key={item.id} onClick={()=>openDetail(item.id)}><Poster title={displayTitle(item)} label={item.kind==='episode'?'SERIE':quality(item)} src={item.artworkUrl}/><span>{displayTitle(item)}</span></button>)}</div></section>}
      </div>
      </div>
    </section>
  </div>{reviewOpen&&<MediaReviewDialog detail={detail} review={currentReview} close={()=>setReviewOpen(false)} onSaveMetadata={onSaveMetadata} onResolve={onResolveIdentification} onApplyCandidate={onApplyCandidate} onRetryMetadata={onRefreshMetadata} onChanged={()=>openDetail(detail.id)}/>}</>
}

function CarouselRow({label,children}:{label:string;children:React.ReactNode}){
  const dragEnabled=useContext(CarouselDragContext);
  const rowRef=useRef<HTMLDivElement|null>(null);
  const drag=useRef({active:false,pointerId:-1,startX:0,scrollLeft:0,moved:false});
  const [dragging,setDragging]=useState(false);
  const [canScroll,setCanScroll]=useState(false);
  const updateCanScroll=useCallback(()=>{
    const row=rowRef.current;
    setCanScroll(Boolean(row&&row.scrollWidth>row.clientWidth+1));
  },[]);
  const scrollRow=(direction:-1|1)=>{
    const row=rowRef.current;
    if(!row)return;
    row.scrollBy({left:direction*Math.max(260,row.clientWidth*0.82),behavior:'smooth'});
    window.setTimeout(updateCanScroll,240);
  };
  useEffect(()=>{
    updateCanScroll();
    window.addEventListener('resize',updateCanScroll);
    return()=>window.removeEventListener('resize',updateCanScroll);
  },[children,updateCanScroll]);
  const startDrag=(event:React.PointerEvent<HTMLDivElement>)=>{
    if(event.button!==0)return;
    const row=rowRef.current;
    if(!row||row.scrollWidth<=row.clientWidth)return;
    drag.current={active:true,pointerId:event.pointerId,startX:event.clientX,scrollLeft:row.scrollLeft,moved:false};
  };
  const moveDrag=(event:React.PointerEvent<HTMLDivElement>)=>{
    const row=rowRef.current;
    const state=drag.current;
    if(!row||!state.active||state.pointerId!==event.pointerId)return;
    const delta=event.clientX-state.startX;
    if(Math.abs(delta)>4&&!state.moved){
      state.moved=true;
      row.setPointerCapture(event.pointerId);
      setDragging(true);
    }
    if(state.moved){
      row.scrollLeft=state.scrollLeft-delta;
      event.preventDefault();
    }
  };
  const endDrag=(event:React.PointerEvent<HTMLDivElement>)=>{
    const row=rowRef.current;
    if(row&&drag.current.pointerId===event.pointerId){
      try{row.releasePointerCapture(event.pointerId);}catch{/* Pointer capture may already be released by the browser. */}
    }
    drag.current.active=false;
    setDragging(false);
  };
  const blockDraggedClick=(event:React.MouseEvent<HTMLDivElement>)=>{
    if(!drag.current.moved)return;
    event.preventDefault();
    event.stopPropagation();
    drag.current.moved=false;
  };
  return <div className="carousel-shell">
    {canScroll&&<button className="carousel-button carousel-button-left" onClick={()=>scrollRow(-1)} aria-label={`Ver anteriores en ${label}`} title="Anteriores"><ChevronLeft size={20}/></button>}
    <div ref={rowRef} className={`card-row ${dragging?'dragging':''} ${dragEnabled?'draggable':''}`} onScroll={updateCanScroll}
      {...(dragEnabled?{onPointerDown:startDrag,onPointerMove:moveDrag,onPointerUp:endDrag,onPointerCancel:endDrag,onClickCapture:blockDraggedClick}:{})}>{children}</div>
    {canScroll&&<button className="carousel-button carousel-button-right" onClick={()=>scrollRow(1)} aria-label={`Ver siguientes en ${label}`} title="Siguientes"><ChevronRight size={20}/></button>}
  </div>
}

function SeriesCard({series,openSeries}:{series:SeriesSummary;openSeries:(series:SeriesSummary)=>void}){
  return <article className="media-card series-card">
    <button className="poster-detail" onClick={()=>openSeries(series)} aria-label={`Ver detalles de ${series.title}`}>
      <Poster title={series.title} label="SERIE" src={series.artworkUrl}/>
    </button>
    <div className="card-copy">
      <div>
        <button className="title-link" onClick={()=>openSeries(series)}>{series.title}</button>
        <p>{series.seasons} temporada{series.seasons===1?'':'s'} · {series.episodes} capítulos</p>
        <div className="series-season-counts" aria-label="Capítulos por temporada">{series.seasonItems.map(season=><span key={season.seasonNumber}>T{season.seasonNumber} <b>{season.episodes.length}</b></span>)}</div>
      </div>
    </div>
  </article>
}

function SeriesDetailModal({series,close,openDetail,playMedia,customCategories,onToggleCategory}:{series:SeriesSummary;close:()=>void;openDetail:(id:string)=>void;playMedia:(id:string)=>void;customCategories:CustomCategory[];onToggleCategory:(id:string,member:boolean)=>Promise<void>}){
  return <div className="modal-backdrop" onMouseDown={event=>{if(event.target===event.currentTarget)close();}}>
    <section className="series-detail-modal">
      <button className="modal-close" onClick={close} aria-label="Cerrar serie" title="Cerrar"><X aria-hidden="true"/></button>
      <div className="detail-scroll">
      <header>
        <span className="eyebrow">SERIE</span>
        <h1>{series.title}</h1>
        <p>{series.seasons} temporada{series.seasons===1?'':'s'} · {series.episodes} capítulos</p>
        <CustomCategoryPicker categories={customCategories} seriesTitle={series.title} onToggle={onToggleCategory}/>
      </header>
      <div className="series-seasons">
        {series.seasonItems.map(season=><section key={season.seasonNumber}>
          <div className="series-season-title"><h2>{season.title}</h2><span>{season.episodes.length} capítulos</span></div>
          <div className="episode-list">
            {season.episodes.map(episode=>{
              const thumbnail=episode.backdropUrl||episode.artworkUrl;
              const genericTitle=episode.title.toLocaleLowerCase()===`episodio ${episode.episodeNumber}`;
              return <article key={episode.id}>
                <button className="episode-thumbnail" onClick={()=>void playMedia(episode.id)} aria-label={`Reproducir episodio ${episode.episodeNumber}`}>
                  {thumbnail?<img src={assetUrl(thumbnail)} alt=""/>:<span>{initials(series.title)}</span>}
                  <i><Play fill="currentColor"/></i>
                </button>
                <div className="episode-copy">
                  <button onClick={()=>void openDetail(episode.id)}><b>Episodio {episode.episodeNumber}</b><span>{genericTitle?series.title:episode.title}</span></button>
                  <p>{episode.overview||'Sin descripcion cargada. Abri la ficha del episodio para agregarla.'}</p>
                  <div><button className="episode-action-play" onClick={()=>void playMedia(episode.id)}><Play fill="currentColor"/>Reproducir</button><button onClick={()=>void openDetail(episode.id)}>Ver detalles</button></div>
                </div>
                {episode.progressPercent>0&&<span className="episode-progress">{Math.round(episode.progressPercent)}%</span>}
              </article>;
            })}
          </div>
        </section>)}
      </div>
      </div>
    </section>
  </div>
}

const displayTitle=(item:MediaSummary)=>item.kind==='episode'?(item.seriesTitle||item.title):item.title;
const initials=(value:string)=>value.split(/\s+/).filter(Boolean).slice(0,3).map(v=>v[0]).join('').toUpperCase();
const hueFor=(value:string)=>[...value].reduce((a,c)=>a+c.charCodeAt(0),0)%360;
const quality=(item:MediaSummary)=>item.technical.height?(item.technical.height>=2160?'4K':`${item.technical.height}p`):'ORIGINAL';
const formatDuration=(ms:number)=>`${Math.floor(ms/3600000)} h ${Math.round((ms%3600000)/60000)} min`;
const assetUrl=(path:string)=>convertFileSrc(path);
