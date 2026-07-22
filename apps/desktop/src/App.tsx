import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import {
  Bookmark, Check, ChevronLeft, ChevronRight, CircleAlert, Clock3, Film, FolderCog, Heart, History,
  FolderOpen, Home, ImagePlus, KeyRound, Library, ListVideo, LoaderCircle, LogOut, Pencil, Play, RefreshCw, Save,
  Copy, QrCode, Radio, Search, Settings, ShieldCheck, Smartphone, Star, Tags, Tv, UserRound, Users, Wifi, X
} from 'lucide-react';
import type { Bootstrap, ClassificationUpdate, HomeDto, IdentificationReview, MediaDetail, MediaMetadataCandidate, MediaMetadataUpdate, MediaSummary, RemoteCommand, RemoteStatus, ScanProgress, SeriesSummary } from './types';
import { InternalPlayer, type InternalPlayerSource } from './InternalPlayer';

type Page = 'Inicio'|'Películas'|'Series'|'Continuar viendo'|'Mi lista'|'Favoritos'|'Agregadas recientemente'|'Historial'|'Configuración';
type AuthMode = 'create'|'login';
type PendingAccount = { name:string; password:string };
type PendingUpdate = Awaited<ReturnType<typeof check>>;
const emptyHome: HomeDto = { heroes:[],continueWatching:[],recentlyAdded:[],movies:[],series:[],favorites:[] };
const emptyScan: ScanProgress = {running:false,cancelRequested:false,found:0,processed:0,skipped:0,errors:0,percent:0};

const navigation: Array<{label:Page; icon: typeof Home}> = [
  {label:'Inicio',icon:Home},{label:'Películas',icon:Film},{label:'Series',icon:Tv},{label:'Continuar viendo',icon:Clock3},
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
  const [playerSource,setPlayerSource]=useState<InternalPlayerSource|null>(null);
  const [availableUpdate,setAvailableUpdate]=useState<PendingUpdate|null>(null);
  const [updateMessage,setUpdateMessage]=useState<string|null>(null);
  const [updating,setUpdating]=useState(false);
  const [metadataLoading,setMetadataLoading]=useState(false);
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

  const rescan=async()=>{try{if(scan.running){setScan(await invoke('cancel_scan'));}else{setScan(await invoke('start_scan',{reason:'manual'}));}}catch(cause){setError(String(cause));}};
  const chooseFolder=async()=>{const selected=await open({directory:true,multiple:false,title:'Elegir carpeta de películas y series'});if(typeof selected==='string'){try{await invoke('replace_library_root',{path:selected});await refresh();}catch(cause){setError(String(cause));}}};
  const openDetail=async(id:string)=>{try{const data=await invoke<MediaDetail|null>('media_detail',{id});setDetail(data);}catch(cause){setError(String(cause));}};
  const resolveIdentification=async(mediaId:string,classification:ClassificationUpdate)=>{try{setError(null);await invoke('resolve_identification',{mediaId,classification});await refresh();}catch(cause){setError(String(cause));}};
  const setFlag=async(item:MediaSummary,flag:'favorite'|'watchlist')=>{const value=flag==='favorite'?!item.favorite:!item.inWatchlist;await invoke('set_media_flag',{mediaId:item.id,flag,value});await refresh();if(detail?.id===item.id)setDetail(await invoke('media_detail',{id:item.id}));};
  const saveMetadata=async(mediaId:string,metadata:MediaMetadataUpdate)=>{try{setError(null);await invoke('update_media_metadata',{mediaId,metadata});const next=await invoke<MediaDetail|null>('media_detail',{id:mediaId});setDetail(next);await refresh();}catch(cause){setError(String(cause));}};
  const refreshMetadata=async(mediaId:string)=>{try{setMetadataLoading(true);setError(null);await invoke('refresh_media_metadata',{mediaId});const next=await invoke<MediaDetail|null>('media_detail',{id:mediaId});setDetail(next);await refresh();}catch(cause){setError(String(cause));}finally{setMetadataLoading(false);}};
  const applyMetadataCandidate=async(mediaId:string,candidate:MediaMetadataCandidate)=>{try{setMetadataLoading(true);setError(null);await invoke('apply_metadata_candidate',{mediaId,candidate});const next=await invoke<MediaDetail|null>('media_detail',{id:mediaId});setDetail(next);await refresh();}catch(cause){setError(String(cause));}finally{setMetadataLoading(false);}};
  const playMedia=async(id:string)=>{try{setError(null);const [detail,path]=await Promise.all([invoke<MediaDetail|null>('media_detail',{id}),invoke<string|null>('technical_path',{mediaId:id})]);if(!detail||!path){setError('No se encontró el archivo para reproducir.');return;}const durationMs=detail.runtimeMs||detail.technical.durationMs||0;const resumeMs=detail.completed?0:Math.round(durationMs*(detail.progressPercent/100));setDetail(null);setPlayerSource({detail,path,url:assetUrl(path),resumeMs});}catch(cause){setError(`No se pudo abrir el reproductor interno: ${String(cause)}`);}};
  const openExternalMedia=async(id:string)=>{try{setError(null);await invoke('player_command',{command:{type:'play',media_id:id}});}catch(cause){setError(`No se pudo abrir reproductor externo: ${String(cause)}`);}};
  const playNextMedia=async(id:string)=>{try{setError(null);const [detail,path]=await Promise.all([invoke<MediaDetail|null>('media_detail',{id}),invoke<string|null>('technical_path',{mediaId:id})]);if(!detail||!path){setError('No se encontro la siguiente parte para reproducir.');return;}setDetail(null);setPlayerSource({detail,path,url:assetUrl(path),resumeMs:0});}catch(cause){setError(`No se pudo abrir la siguiente parte: ${String(cause)}`);}};
  useEffect(()=>{let cleanup:(()=>void)|undefined;void listen<RemoteCommand>('remote-command',event=>{const command=event.payload;if(command.type==='library_play_media')void playMedia(command.media_id);if(command.type==='navigate_back'){if(detail)setDetail(null);else if(playerSource)setPlayerSource(null);else setPage('Inicio');}if(command.type==='navigate'){const pages:Page[]=['Inicio','Películas','Series','Mi lista','Favoritos'];const current=Math.max(0,pages.indexOf(page));if(command.direction==='left'||command.direction==='up')setPage(pages[(current-1+pages.length)%pages.length]);if(command.direction==='right'||command.direction==='down')setPage(pages[(current+1)%pages.length]);}}).then(unlisten=>cleanup=unlisten);return()=>cleanup?.();},[detail,page,playerSource]);
  const runRemoteAction=async(command:string,args:Record<string,unknown>={})=>{try{setRemoteBusy(true);setError(null);setRemoteStatus(await invoke<RemoteStatus>(command,args));}catch(cause){setError(String(cause));}finally{setRemoteBusy(false);}};
  const submitAccount=async(mode:AuthMode,name:string,password:string)=>{if(mode==='create'){setPendingAccount({name,password});return;}try{setError(null);await invoke('login_account',{name,password});setSearch('');setPage('Inicio');await refresh();}catch(cause){setError(String(cause));}};
  const confirmCreateAccount=async()=>{if(!pendingAccount)return;try{setError(null);await invoke('create_account',pendingAccount);setPendingAccount(null);setSearch('');setPage('Inicio');await refresh();}catch(cause){setError(String(cause));setPendingAccount(null);}};
  const logout=async()=>{try{await invoke('logout_account');setDetail(null);setPlayerSource(null);setSearch('');setPage('Inicio');await refresh();}catch(cause){setError(String(cause));}};
  const checkForUpdates=async()=>{try{setUpdating(true);setAvailableUpdate(null);setUpdateMessage('Buscando actualizaciones en GitHub Releases...');const update=await check();if(update){setAvailableUpdate(update);setUpdateMessage(`Actualización disponible: versión ${update.version}`);}else{setUpdateMessage('CINE WANA ya está en la última versión publicada.');}}catch(cause){setUpdateMessage(`No se pudo buscar actualizaciones: ${String(cause)}`);}finally{setUpdating(false);}};
  const installAvailableUpdate=async()=>{if(!availableUpdate)return;try{setUpdating(true);let downloaded=0;let contentLength=0;await availableUpdate.downloadAndInstall(event=>{if(event.event==='Started'){contentLength=event.data.contentLength||0;setUpdateMessage('Descargando actualización...');}else if(event.event==='Progress'){downloaded+=event.data.chunkLength;setUpdateMessage(contentLength?`Descargando ${Math.round(downloaded/contentLength*100)}%`:'Descargando actualización...');}else if(event.event==='Finished'){setUpdateMessage('Instalando actualización...');}});setUpdateMessage('Actualización instalada. En Windows la app se cerrará para terminar.');}catch(cause){setUpdateMessage(`No se pudo instalar la actualización: ${String(cause)}`);}finally{setUpdating(false);}};
  const hero=home.heroes[heroIndex];

  if(boot&&!boot.activeAccount)return <AuthScreen boot={boot} mode={authMode} setMode={setAuthMode} pendingAccount={pendingAccount} onSubmit={submitAccount} onConfirmCreate={confirmCreateAccount} onCancelCreate={()=>setPendingAccount(null)} error={error} clearError={()=>setError(null)}/>;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span>CINE</span><strong>WANA</strong></div>
      <nav>{navigation.map(({label,icon:Icon})=><button key={label} className={page===label?'active':''} title={label} onClick={()=>setPage(label)}><Icon size={18}/><span>{label}</span></button>)}</nav>
      <div className="sidebar-status"><span className={`status-dot ${boot?.roots.some(r=>r.status==='online')?'online':''}`}/><div><b>{boot?.roots[0]?.status==='online'?'Biblioteca conectada':'Biblioteca sin conexión'}</b><small>{items.length} archivos en catálogo</small></div></div>
    </aside>
    <main>
      <header className="topbar">
        <div className="search"><Search size={18}/><input value={search} onChange={e=>setSearch(e.target.value)} placeholder="Buscar títulos, años y series…"/>{search&&<button onClick={()=>setSearch('')}><X size={16}/></button>}</div>
        {boot?.activeAccount&&<div className="account-pill"><UserRound size={16}/><span>{boot.activeAccount.name}</span><button onClick={logout} title="Cerrar sesión"><LogOut size={15}/></button></div>}
        <button className={`scan-button ${scan.running?'working':''}`} onClick={rescan}>{scan.running?<><X size={17}/><span>Cancelar escaneo</span></>:<><RefreshCw size={17}/><span>Reescanear biblioteca</span></>}</button>
      </header>

      {scan.running&&<div className="scan-strip"><LoaderCircle className="spin" size={16}/><div><b>{scan.message||'Escaneando biblioteca'}</b><small>{scan.currentFile||`${scan.found} archivos encontrados`}</small></div><div className="scan-meter"><i style={{width:`${scan.percent}%`}}/></div><span>{Math.round(scan.percent)}%</span></div>}
      {error&&<div className="error-banner"><CircleAlert size={18}/><span>{error}</span><button onClick={()=>setError(null)}><X size={16}/></button></div>}

      {!boot?<Loading/>:page==='Configuración'?<RemoteSettingsPage boot={boot} scan={scan} updating={updating} updateMessage={updateMessage} updateVersion={availableUpdate?.version} onRescan={rescan} onChoose={chooseFolder} onLogout={logout} onCheckUpdates={checkForUpdates} onInstallUpdate={installAvailableUpdate} onResolveIdentification={resolveIdentification} remote={remoteStatus} remoteBusy={remoteBusy} runRemoteAction={runRemoteAction}/>:page==='Series'?<SeriesPage series={home.series} search={search} openSeries={setSeriesDetail}/>:page==='Inicio'&&!search?<HomePage home={home} hero={hero} heroIndex={heroIndex} setHeroIndex={setHeroIndex} openDetail={openDetail} openSeries={setSeriesDetail} setFlag={setFlag} playMedia={playMedia}/>:<CatalogPage title={page} items={visible} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>}
    </main>
    {seriesDetail&&<SeriesDetailModal series={seriesDetail} close={()=>setSeriesDetail(null)} openDetail={openDetail} playMedia={playMedia}/>}
    {detail&&<DetailModal detail={detail} close={()=>setDetail(null)} setFlag={setFlag} playMedia={playMedia} openExternalMedia={openExternalMedia} onSaveMetadata={saveMetadata} onRefreshMetadata={refreshMetadata} onApplyCandidate={applyMetadataCandidate} openDetail={openDetail} metadataLoading={metadataLoading}/>}
    {playerSource&&<InternalPlayer source={playerSource} onClose={()=>setPlayerSource(null)} onOpenExternal={openExternalMedia} onPlayNext={playNextMedia} onProgressSaved={()=>void refresh()}/>}
  </div>;
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
  </div>
}

function HomePage({home,hero,heroIndex,setHeroIndex,openDetail,openSeries,setFlag,playMedia}:{home:HomeDto;hero?:MediaSummary;heroIndex:number;setHeroIndex:(n:number)=>void;openDetail:(id:string)=>void;openSeries:(series:SeriesSummary)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void}){
  return <div className="content home-page">
    {hero?<section className="hero" style={{'--hero-hue':hueFor(hero.title),'--hero-image':hero.backdropUrl?`url("${assetUrl(hero.backdropUrl)}")`:'none'} as React.CSSProperties}>
      {hero.previewUrl&&<video key={hero.previewUrl} className="hero-video" src={assetUrl(hero.previewUrl)} muted autoPlay loop playsInline/>}<div className="hero-shade"/><div className="hero-noise"/><div className="hero-copy"><span className="eyebrow">VISTA PREVIA DE TU BIBLIOTECA</span><h1>{displayTitle(hero)}</h1><p>{hero.year||'Año sin identificar'} · {quality(hero)} {hero.technical.hdrType?`· ${hero.technical.hdrType}`:''}</p>
      <div className="hero-actions"><button className="primary" onClick={()=>void playMedia(hero.id)}><Play fill="currentColor" size={18}/>Reproducir</button><button onClick={()=>openDetail(hero.id)}>Ver detalles</button></div></div>
      <div className="hero-controls"><button onClick={()=>setHeroIndex((heroIndex-1+home.heroes.length)%home.heroes.length)}><ChevronLeft/></button><div>{home.heroes.map((_,i)=><button key={i} className={i===heroIndex?'active':''} onClick={()=>setHeroIndex(i)}/>)}</div><button onClick={()=>setHeroIndex((heroIndex+1)%home.heroes.length)}><ChevronRight/></button></div>
    </section>:<EmptyLibrary/>}
    <MediaRow title="Continuar viendo" items={home.continueWatching} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>
    <MediaRow title="Agregadas recientemente" items={home.recentlyAdded} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>
    <MediaRow title="Películas" items={home.movies} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>
    <SeriesCarouselRow title="Series" series={home.series} openSeries={openSeries}/>
    <MediaRow title="Favoritos" items={home.favorites} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>
  </div>
}

function EmptyLibrary(){return <section className="empty-library"><Library size={42}/><h1>Tu sala está lista</h1><p>Conectá la carpeta predeterminada o elegí otra desde Configuración y después reescaneá.</p></section>}
function MediaRow(props:{title:string;items:MediaSummary[];openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void}){if(!props.items.length)return null;return <section className="media-section"><div className="section-title"><h2>{props.title}</h2><span>{props.items.length}</span></div><CarouselRow label={props.title}>{props.items.map(i=><MediaCard key={i.id} item={i} {...props}/>)}</CarouselRow></section>}
function CatalogPage({title,items,openDetail,setFlag,playMedia}:{title:string;items:MediaSummary[];openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void}){return <div className="content catalog-page"><div className="page-heading"><div><span className="eyebrow">TU BIBLIOTECA</span><h1>{title}</h1></div><span>{items.length} resultados</span></div>{items.length?<div className="card-grid">{items.map(i=><MediaCard key={i.id} item={i} openDetail={openDetail} setFlag={setFlag} playMedia={playMedia}/>)}</div>:<div className="empty-results"><Film/><h2>No hay contenido en esta sección</h2><p>Los elementos aparecerán después del próximo escaneo.</p></div>}</div>}
function SeriesPage({series,search,openSeries}:{series:SeriesSummary[];search:string;openSeries:(series:SeriesSummary)=>void}){const q=search.toLocaleLowerCase();const list=series.filter(s=>s.title.toLocaleLowerCase().includes(q));return <div className="content catalog-page"><div className="page-heading"><div><span className="eyebrow">EPISODIOS AGRUPADOS</span><h1>Series</h1></div><span>{list.length} series</span></div><div className="card-grid">{list.map(s=><SeriesCard key={s.episodeId} series={s} openSeries={openSeries}/>)}</div></div>}

function MediaCard({item,openDetail,setFlag,playMedia}:{item:MediaSummary;openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void}){return <article className="media-card"><div className="poster-button"><button className="poster-detail" onClick={()=>openDetail(item.id)} aria-label={`Ver detalles de ${displayTitle(item)}`}><Poster title={displayTitle(item)} label={item.kind==='episode'?'SERIE':quality(item)} src={item.artworkUrl}/></button>{item.progressPercent>0&&<span className="card-progress"><i style={{width:`${item.progressPercent}%`}}/></span>}<button className="hover-play" onClick={()=>void playMedia(item.id)} title="Reproducir"><Play fill="currentColor"/></button></div><div className="card-copy"><div><button className="title-link" onClick={()=>openDetail(item.id)}>{displayTitle(item)}</button><p>{item.year||'Sin año'}{item.kind==='episode'?` · T${item.seasonNumber} E${item.episodeNumber}`:''}</p></div><div className="quick-actions"><button className={item.favorite?'selected':''} title="Favorito" onClick={()=>void setFlag(item,'favorite')}><Heart size={15} fill={item.favorite?'currentColor':'none'}/></button><button className={item.inWatchlist?'selected':''} title="Mi lista" onClick={()=>void setFlag(item,'watchlist')}><Bookmark size={15} fill={item.inWatchlist?'currentColor':'none'}/></button></div></div></article>}
function Poster({title,label,src}:{title:string;label:string;src?:string}){return <div className={`poster ${src?'has-image':''}`} style={{'--poster-hue':hueFor(title)} as React.CSSProperties}>{src&&<img src={assetUrl(src)} alt=""/>}<span className="poster-label">{label}</span>{!src&&<b>{initials(title)}</b>}<small>{title}</small></div>}

function SettingsPage({boot,scan,updating,updateMessage,updateVersion,onRescan,onChoose,onLogout,onCheckUpdates,onInstallUpdate}:{boot:Bootstrap;scan:ScanProgress;updating:boolean;updateMessage:string|null;updateVersion?:string;onRescan:()=>void;onChoose:()=>void;onLogout:()=>void;onCheckUpdates:()=>void;onInstallUpdate:()=>void}){const root=boot.roots.find(r=>r.enabled)||boot.roots[0];return <div className="content settings-page"><div className="page-heading"><div><span className="eyebrow">CINE WANA</span><h1>Configuración</h1></div></div><section className="settings-card compact"><div className="settings-icon"><UserRound/></div><div className="settings-main"><div className="settings-title"><div><h2>Cuenta local</h2><p>Progreso, historial y listas de esta sesión</p></div><span className="root-status online">{boot.activeAccount?.name}</span></div><div className="diagnostic-row"><span>Cuentas creadas</span><b>{boot.accounts.length}</b></div><button className="settings-rescan account-logout" onClick={onLogout}><LogOut/>Cerrar sesión</button></div></section><section className="settings-card compact"><div className="settings-icon"><RefreshCw/></div><div className="settings-main"><div className="settings-title"><div><h2>Actualizaciones</h2><p>GitHub Releases firmado para Windows x64</p></div><span className="root-status">{updateVersion?`v${updateVersion}`:'Manual'}</span></div>{updateMessage&&<div className="update-note">{updateMessage}</div>}<div className="update-actions"><button className="settings-rescan" disabled={updating} onClick={onCheckUpdates}>{updating?<><LoaderCircle className="spin"/>Buscando</>:<><RefreshCw/>Buscar actualizaciones</>}</button>{updateVersion&&<button className="primary settings-rescan" disabled={updating} onClick={onInstallUpdate}><Check/>Instalar versión</button>}</div></div></section><section className="settings-card"><div className="settings-icon"><FolderCog/></div><div className="settings-main"><div className="settings-title"><div><h2>Biblioteca</h2><p>Carpeta activa y lectura recursiva</p></div><span className={`root-status ${root?.status}`}>{root?.status==='online'?'Conectada':root?.status==='scanning'?'Escaneando':'Desconectada'}</span></div><div className="path-box"><code>{root?.localPath||'Sin carpeta configurada'}</code><button onClick={onChoose}>Cambiar carpeta</button></div><div className="settings-stats"><div><small>Último escaneo</small><b>{root?.lastScanAt?new Date(root.lastScanAt).toLocaleString('es-AR'):'Todavía no finalizó'}</b></div><div><small>Subcarpetas</small><b>{root?.recursive?'Incluidas':'Excluidas'}</b></div><div><small>Archivos desconectados</small><b>{root?.disconnectedCount||0}</b></div></div><button className="primary settings-rescan" onClick={onRescan}>{scan.running?<><X/>Cancelar escaneo</>:<><RefreshCw/>Reescanear biblioteca</>}</button></div></section><section className="settings-card compact"><div className="settings-icon"><Film/></div><div className="settings-main"><div className="settings-title"><div><h2>Componentes multimedia</h2><p>Diagnóstico del entorno de desarrollo</p></div></div><div className="diagnostic-row"><span>FFmpeg / ffprobe</span><b className={boot.ffprobeAvailable?'ok':'pending'}>{boot.ffprobeAvailable?<><Check/>Disponible</>:<><CircleAlert/>Pendiente</>}</b></div><div className="diagnostic-row"><span>Reproductor interno + externo</span><b className={boot.playerAvailable?'ok':'pending'}>{boot.playerAvailable?<><Check/>Disponible</>:<><CircleAlert/>No encontrado</>}</b></div></div></section></div>}

function IdentificationReviewSettings({reviews,onResolve}:{reviews:IdentificationReview[];onResolve:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>}){
  return <div className="content settings-page identification-settings">
    <section className="settings-card">
      <div className="settings-icon"><CircleAlert/></div>
      <div className="settings-main">
        <div className="settings-title"><div><h2>Identificacion por revisar</h2><p>Decisiones guardadas por CINE WANA sin modificar los archivos originales</p></div><span className={`root-status ${reviews.length?'pending':'online'}`}>{reviews.length} pendientes</span></div>
        {reviews.length===0?<div className="identification-empty"><Check/>No hay peliculas ni episodios dudosos.</div>:<div className="identification-review-list">{reviews.map(review=><IdentificationReviewCard key={review.mediaId} review={review} onResolve={onResolve}/>)}</div>}
      </div>
    </section>
  </div>
}

function IdentificationReviewCard({review,onResolve}:{review:IdentificationReview;onResolve:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>}){
  const [kind,setKind]=useState(review.kind);
  const [title,setTitle]=useState(review.title);
  const [seriesTitle,setSeriesTitle]=useState(review.seriesTitle||'');
  const [seasonNumber,setSeasonNumber]=useState(review.seasonNumber?.toString()||'');
  const [episodeNumber,setEpisodeNumber]=useState(review.episodeNumber?.toString()||'');
  const [saving,setSaving]=useState(false);
  const [rescanning,setRescanning]=useState(false);
  const [revealError,setRevealError]=useState('');
  const [rescanMessage,setRescanMessage]=useState('');
  const valid=title.trim().length>0&&(kind==='movie'||(seriesTitle.trim().length>0&&Number(seasonNumber)>0&&Number(episodeNumber)>0));
  const save=async()=>{
    if(!valid)return;
    setSaving(true);
    try{
      await onResolve(review.mediaId,{kind,title:title.trim(),seriesTitle:kind==='episode'?seriesTitle.trim():null,seasonNumber:kind==='episode'?Number(seasonNumber):null,episodeNumber:kind==='episode'?Number(episodeNumber):null});
    }finally{setSaving(false);}
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
    }catch(cause){setRescanMessage(String(cause));}
    finally{setRescanning(false);}
  };
  return <article className="identification-review-card">
    <div className="identification-warning"><CircleAlert/><div><b>{review.fileName}</b><span>{review.reason}</span>{revealError&&<small>{revealError}</small>}{rescanMessage&&<small>{rescanMessage}</small>}</div><div className="identification-file-actions"><button onClick={()=>void reveal()}><FolderOpen/>Mostrar archivo</button><button disabled={rescanning} onClick={()=>void rescanOne()}>{rescanning?<LoaderCircle className="spin"/>:<RefreshCw/>}Reescanear este archivo</button></div></div>
    <div className="identification-kind"><button className={kind==='movie'?'selected':''} onClick={()=>setKind('movie')}>Pelicula</button><button className={kind==='episode'?'selected':''} onClick={()=>setKind('episode')}>Serie / episodio</button></div>
    <div className="identification-fields">
      <label><span>{kind==='movie'?'Titulo de la pelicula':'Titulo del episodio'}</span><input value={title} onChange={event=>setTitle(event.target.value)}/></label>
      {kind==='episode'&&<><label><span>Serie</span><input value={seriesTitle} onChange={event=>setSeriesTitle(event.target.value)}/></label><label><span>Temporada</span><input inputMode="numeric" value={seasonNumber} onChange={event=>setSeasonNumber(event.target.value.replace(/\D/g,''))}/></label><label><span>Episodio</span><input inputMode="numeric" value={episodeNumber} onChange={event=>setEpisodeNumber(event.target.value.replace(/\D/g,''))}/></label></>}
    </div>
    <button className="primary identification-save" disabled={!valid||saving} onClick={()=>void save()}>{saving?<LoaderCircle className="spin"/>:<Save/>}Guardar identificacion</button>
  </article>
}

type SettingsProps={boot:Bootstrap;scan:ScanProgress;updating:boolean;updateMessage:string|null;updateVersion?:string;onRescan:()=>void;onChoose:()=>void;onLogout:()=>void;onCheckUpdates:()=>void;onInstallUpdate:()=>void;onResolveIdentification:(mediaId:string,classification:ClassificationUpdate)=>Promise<void>};
function RemoteSettingsPage(props:SettingsProps&{remote:RemoteStatus|null;remoteBusy:boolean;runRemoteAction:(command:string,args?:Record<string,unknown>)=>Promise<void>}){
  const {remote,remoteBusy,runRemoteAction,onResolveIdentification,...settings}=props;
  const copyUrl=async()=>{if(remote?.pairing?.url)await navigator.clipboard.writeText(remote.pairing.url);else if(remote?.url)await navigator.clipboard.writeText(remote.url);};
  return <><SettingsPage {...settings}/><IdentificationReviewSettings reviews={props.boot.identificationReviews} onResolve={onResolveIdentification}/><div className="content settings-page remote-settings-section"><section className="settings-card remote-settings-card"><div className="settings-icon"><Radio/></div><div className="settings-main">
    <div className="settings-title"><div><h2>Control remoto</h2><p>Vinculación privada desde la misma red Wi‑Fi</p></div><span className={`root-status ${remote?.enabled?'online':''}`}>{remote?.enabled?'Activo':'Desactivado'}</span></div>
    {!remote?.assetRootReady&&<div className="remote-notice"><CircleAlert/>La interfaz móvil todavía no está compilada. El botón Activar la compilará antes de la prueba.</div>}
    {remote?.error&&<div className="remote-notice error"><CircleAlert/>{remote.error}</div>}
    <div className="remote-actions"><button className={remote?.enabled?'settings-rescan':'primary settings-rescan'} disabled={remoteBusy} onClick={()=>void runRemoteAction(remote?.enabled?'remote_stop':'remote_start')}>{remoteBusy?<LoaderCircle className="spin"/>:remote?.enabled?<X/>:<Wifi/>}{remote?.enabled?'Desactivar':'Activar control remoto'}</button>{remote?.enabled&&<button className="settings-rescan" disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_create_pairing')}><QrCode/>{remote.pairing?'Renovar QR':'Mostrar QR'}</button>}</div>
    {remote?.enabled&&<div className="remote-address"><div><small>Dirección local</small><code>{remote.url}</code></div><button title="Copiar dirección" onClick={()=>void copyUrl()}><Copy/></button></div>}
    {remote?.pairing&&<div className="pairing-panel"><img src={remote.pairing.qrDataUrl} alt="QR para vincular el teléfono"/><div><span className="eyebrow">ESCANEÁ CON EL TELÉFONO</span><h3>Código {remote.pairing.code}</h3><p>Vence {new Date(remote.pairing.expiresAt).toLocaleTimeString('es-AR',{hour:'2-digit',minute:'2-digit'})}. También podés copiar la URL completa.</p><button onClick={()=>void copyUrl()}><Copy/>Copiar enlace</button></div></div>}
    {remote?.pending.map(request=><div className="pair-request" key={request.id}><Smartphone/><div><b>{request.deviceName}</b><small>Solicita vinculación</small></div><button className="approve" disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_approve_pairing',{requestId:request.id})}><ShieldCheck/>Aprobar</button><button disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_reject_pairing',{requestId:request.id})}><X/>Rechazar</button></div>)}
    {remote&&remote.devices.length>0&&<div className="paired-devices"><h3>Dispositivos vinculados</h3>{remote.devices.map(device=><div key={device.id}><Smartphone/><span><b>{device.name}</b><small>{device.lastSeenAt?`Última conexión ${new Date(device.lastSeenAt).toLocaleString('es-AR')}`:'Todavía no se conectó'}</small></span><button disabled={remoteBusy} onClick={()=>void runRemoteAction('remote_revoke_device',{deviceId:device.id})}>Desvincular</button></div>)}</div>}
    {remote?.enabled&&!remote.secureContext&&<div className="remote-security-note"><ShieldCheck/><span><b>Modo de prueba local</b>El control funciona por Wi‑Fi. La instalación offline como PWA se habilitará al incorporar HTTPS local confiable antes del instalador.</span></div>}
  </div></section></div></>;
}

const genrePresets=['Acción','Aventura','Animación','Ciencia ficción','Comedia','Documental','Drama','Romance','Suspenso','Terror'];
const parseList=(value:string)=>Array.from(new Set(value.split(',').map(part=>part.trim()).filter(Boolean)));
const addTag=(value:string,tag:string)=>parseList(`${value},${tag}`).join(', ');
const detailToForm=(detail:MediaDetail)=>({title:displayTitle(detail),year:detail.year?.toString()||'',overview:detail.overview||'',genres:detail.genres.join(', '),cast:detail.cast.join(', '),posterPath:'',backdropPath:''});
const metadataLabel=(status:string)=>status==='imported'?'Wikipedia importado':status==='ambiguous'?'Elegir coincidencia':'Información pendiente';

function DetailModal({detail,close,setFlag,playMedia,openExternalMedia,onSaveMetadata,onRefreshMetadata,onApplyCandidate,openDetail,metadataLoading}:{detail:MediaDetail;close:()=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playMedia:(id:string)=>void;openExternalMedia:(id:string)=>void;onSaveMetadata:(id:string,metadata:MediaMetadataUpdate)=>Promise<void>;onRefreshMetadata:(id:string)=>Promise<void>;onApplyCandidate:(id:string,candidate:MediaMetadataCandidate)=>Promise<void>;openDetail:(id:string)=>void;metadataLoading:boolean}){
  const [editing,setEditing]=useState(false);
  const [form,setForm]=useState(()=>detailToForm(detail));
  useEffect(()=>{setEditing(false);setForm(detailToForm(detail));},[detail]);
  const chooseImage=async(field:'posterPath'|'backdropPath')=>{
    const selected=await open({multiple:false,directory:false,title:field==='posterPath'?'Elegir portada':'Elegir fondo',filters:[{name:'Imagen',extensions:['png','jpg','jpeg','webp']}]});
    if(typeof selected==='string')setForm(prev=>({...prev,[field]:selected}));
  };
  const save=async()=>{await onSaveMetadata(detail.id,{title:form.title,year:form.year.trim()?Number(form.year):null,overview:form.overview.trim()||null,genres:parseList(form.genres),cast:parseList(form.cast),posterPath:form.posterPath||null,backdropPath:form.backdropPath||null});setEditing(false);};
  return <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)close();}}>
    <section className="detail-modal detail-modal-expanded" style={{'--detail-backdrop':detail.backdropUrl?`url("${assetUrl(detail.backdropUrl)}")`:'none'} as React.CSSProperties}>
      <button className="modal-close" onClick={close}><X/></button>
      <div className="detail-art"><Poster title={displayTitle(detail)} label={detail.kind==='episode'?'SERIE':quality(detail)} src={detail.artworkUrl}/>{editing&&<div className="art-buttons"><button onClick={()=>void chooseImage('posterPath')}><ImagePlus/>Portada</button><button onClick={()=>void chooseImage('backdropPath')}><ImagePlus/>Fondo</button></div>}</div>
      <div className="detail-copy">
        <div className="detail-headline"><span className="eyebrow">{detail.kind==='episode'?`TEMPORADA ${detail.seasonNumber} · EPISODIO ${detail.episodeNumber}`:'PELÍCULA'}</span><div><button disabled={metadataLoading} onClick={()=>void onRefreshMetadata(detail.id)}>{metadataLoading?<LoaderCircle className="spin"/>:<RefreshCw/>}Volver a buscar información</button><button onClick={()=>setEditing(value=>!value)}><Pencil/>Editar datos</button></div></div>
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
          <p className="overview">{detail.overview||'Todavía no hay descripción. Podés editar esta ficha y agregar la sinopsis, género, actores y portada.'}</p>
          <div className="cast-block"><h3><Users size={15}/> Reparto</h3>{detail.cast.length?<p>{detail.cast.join(', ')}</p>:<p>Sin actores cargados.</p>}</div>
          <div className="metadata-source"><h3>Información externa</h3>{detail.metadataSourceUrl?<p>Fuente: <a href={detail.metadataSourceUrl} target="_blank" rel="noreferrer">Wikipedia</a>{detail.metadataImportedAt?` · ${new Date(detail.metadataImportedAt).toLocaleDateString('es-AR')}`:''}</p>:<p>{detail.metadataStatus==='ambiguous'?'Wikipedia encontró varias posibilidades. Elegí la correcta abajo.':'Todavía no hay una fuente externa guardada.'}</p>}{detail.metadataCandidates.length>0&&<div className="metadata-candidates">{detail.metadataCandidates.map(candidate=><button key={candidate.id} disabled={metadataLoading} onClick={()=>void onApplyCandidate(detail.id,candidate)}><b>{candidate.title}</b><span>{candidate.year||'Sin año'} · {candidate.language.toUpperCase()}</span>{candidate.description&&<small>{candidate.description}</small>}</button>)}</div>}</div>
        </>}
        <div className="detail-actions"><button className="primary" onClick={()=>void playMedia(detail.id)}><Play fill="currentColor"/>Reproducir en CINE WANA</button><button onClick={()=>void openExternalMedia(detail.id)}>Abrir externo</button><button className={detail.inWatchlist?'selected':''} onClick={()=>void setFlag(detail,'watchlist')}><Bookmark/>Mi lista</button><button className={detail.favorite?'selected':''} onClick={()=>void setFlag(detail,'favorite')}><Heart/>Favorito</button></div>
        <div className="technical"><h3>Información técnica</h3><dl><div><dt>Archivo</dt><dd>{detail.fileName}</dd></div><div><dt>Contenedor</dt><dd>{detail.technical.container||'Pendiente de ffprobe'}</dd></div><div><dt>Video</dt><dd>{detail.technical.videoCodec||'Sin analizar'}</dd></div><div><dt>Audio</dt><dd>{detail.technical.audioCodec||'Sin analizar'}</dd></div><div><dt>Subtítulos externos</dt><dd>{detail.tracks.filter(t=>t.external).length}</dd></div></dl></div>
        {detail.recommendations.length>0&&<section className="recommendations"><h3>Más para ver</h3><div>{detail.recommendations.map(item=><button key={item.id} onClick={()=>openDetail(item.id)}><Poster title={displayTitle(item)} label={item.kind==='episode'?'SERIE':quality(item)} src={item.artworkUrl}/><span>{displayTitle(item)}</span></button>)}</div></section>}
      </div>
    </section>
  </div>
}

function CarouselRow({label,children}:{label:string;children:React.ReactNode}){
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
    <div ref={rowRef} className={`card-row ${dragging?'dragging':''}`} onScroll={updateCanScroll} onPointerDown={startDrag} onPointerMove={moveDrag} onPointerUp={endDrag} onPointerCancel={endDrag} onClickCapture={blockDraggedClick}>{children}</div>
    {canScroll&&<button className="carousel-button carousel-button-right" onClick={()=>scrollRow(1)} aria-label={`Ver siguientes en ${label}`} title="Siguientes"><ChevronRight size={20}/></button>}
  </div>
}

function SeriesCarouselRow({title,series,openSeries}:{title:string;series:SeriesSummary[];openSeries:(series:SeriesSummary)=>void}){
  if(!series.length)return null;
  return <section className="media-section">
    <div className="section-title"><h2>{title}</h2><span>{series.length}</span></div>
    <CarouselRow label={title}>
      {series.map(s=><SeriesCard key={s.episodeId} series={s} openSeries={openSeries}/>)}
    </CarouselRow>
  </section>
}

function SeriesCard({series,openSeries}:{series:SeriesSummary;openSeries:(series:SeriesSummary)=>void}){
  return <article className="media-card series-card">
    <button className="poster-detail" onClick={()=>openSeries(series)} aria-label={`Ver detalles de ${series.title}`}>
      <Poster title={series.title} label="SERIE" src={series.artworkUrl}/>
    </button>
    <div className="card-copy">
      <div>
        <button className="title-link" onClick={()=>openSeries(series)}>{series.title}</button>
        <p>{series.seasons} temporada{series.seasons===1?'':'s'} · {series.episodes} episodios</p>
      </div>
    </div>
  </article>
}

function SeriesDetailModal({series,close,openDetail,playMedia}:{series:SeriesSummary;close:()=>void;openDetail:(id:string)=>void;playMedia:(id:string)=>void}){
  return <div className="modal-backdrop" onMouseDown={event=>{if(event.target===event.currentTarget)close();}}>
    <section className="series-detail-modal">
      <button className="modal-close" onClick={close}><X/></button>
      <header>
        <span className="eyebrow">SERIE</span>
        <h1>{series.title}</h1>
        <p>{series.seasons} temporada{series.seasons===1?'':'s'} · {series.episodes} episodios</p>
      </header>
      <div className="series-seasons">
        {series.seasonItems.map(season=><section key={season.seasonNumber}>
          <div className="series-season-title"><h2>{season.title}</h2><span>{season.episodes.length} episodios</span></div>
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
    </section>
  </div>
}

const displayTitle=(item:MediaSummary)=>item.kind==='episode'?(item.seriesTitle||item.title):item.title;
const initials=(value:string)=>value.split(/\s+/).filter(Boolean).slice(0,3).map(v=>v[0]).join('').toUpperCase();
const hueFor=(value:string)=>[...value].reduce((a,c)=>a+c.charCodeAt(0),0)%360;
const quality=(item:MediaSummary)=>item.technical.height?(item.technical.height>=2160?'4K':`${item.technical.height}p`):'ORIGINAL';
const formatDuration=(ms:number)=>`${Math.floor(ms/3600000)} h ${Math.round((ms%3600000)/60000)} min`;
const assetUrl=(path:string)=>convertFileSrc(path);
