import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import {
  Bookmark, Check, ChevronLeft, ChevronRight, CircleAlert, Clock3, Film, FolderCog, Heart, History,
  Home, Library, ListVideo, LoaderCircle, Play, RefreshCw, Search, Settings, Star, Tv, X
} from 'lucide-react';
import type { Bootstrap, HomeDto, MediaDetail, MediaSummary, ScanProgress, SeriesSummary } from './types';

type Page = 'Inicio'|'Películas'|'Series'|'Continuar viendo'|'Mi lista'|'Favoritos'|'Agregadas recientemente'|'Historial'|'Configuración';
const emptyHome: HomeDto = { heroes:[],continueWatching:[],recentlyAdded:[],movies:[],series:[],favorites:[] };
const emptyScan: ScanProgress = {running:false,cancelRequested:false,found:0,processed:0,skipped:0,errors:0,percent:0};

const navigation: Array<{label:Page; icon: typeof Home}> = [
  {label:'Inicio',icon:Home},{label:'Películas',icon:Film},{label:'Series',icon:Tv},{label:'Continuar viendo',icon:Clock3},
  {label:'Mi lista',icon:ListVideo},{label:'Favoritos',icon:Heart},{label:'Agregadas recientemente',icon:Star},{label:'Historial',icon:History},{label:'Configuración',icon:Settings}
];

export function App() {
  const [page,setPage]=useState<Page>('Inicio');
  const [boot,setBoot]=useState<Bootstrap|null>(null);
  const [home,setHome]=useState<HomeDto>(emptyHome);
  const [items,setItems]=useState<MediaSummary[]>([]);
  const [scan,setScan]=useState<ScanProgress>(emptyScan);
  const [search,setSearch]=useState('');
  const [heroIndex,setHeroIndex]=useState(0);
  const [detail,setDetail]=useState<MediaDetail|null>(null);
  const [error,setError]=useState<string|null>(null);

  const refresh = useCallback(async()=>{
    try {
      const data=await invoke<Bootstrap>('bootstrap');
      setBoot(data);setHome(data.home);setScan(data.scan);
      const catalog=await invoke<MediaSummary[]>('catalog',{query:{search:null,kind:null,filter:null,sort:'added_desc',limit:1000,offset:0}});
      setItems(catalog);setError(null);
    } catch (cause) { setError(String(cause)); }
  },[]);

  useEffect(()=>{ void refresh(); const unsubs=[listen<ScanProgress>('scan-progress',e=>setScan(e.payload)),listen('library-changed',()=>void refresh())]; return()=>{void Promise.all(unsubs).then(values=>values.forEach(fn=>fn()));};},[refresh]);
  useEffect(()=>{if(home.heroes.length<2||detail)return;const timer=setInterval(()=>setHeroIndex(i=>(i+1)%home.heroes.length),8000);return()=>clearInterval(timer);},[home.heroes.length,detail]);
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
  const setFlag=async(item:MediaSummary,flag:'favorite'|'watchlist')=>{const value=flag==='favorite'?!item.favorite:!item.inWatchlist;await invoke('set_media_flag',{mediaId:item.id,flag,value});await refresh();if(detail?.id===item.id)setDetail(await invoke('media_detail',{id:item.id}));};
  const hero=home.heroes[heroIndex];

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span>CINE</span><strong>WANA</strong></div>
      <nav>{navigation.map(({label,icon:Icon})=><button key={label} className={page===label?'active':''} onClick={()=>setPage(label)}><Icon size={18}/><span>{label}</span></button>)}</nav>
      <div className="sidebar-status"><span className={`status-dot ${boot?.roots.some(r=>r.status==='online')?'online':''}`}/><div><b>{boot?.roots[0]?.status==='online'?'Biblioteca conectada':'Biblioteca sin conexión'}</b><small>{items.length} archivos en catálogo</small></div></div>
    </aside>
    <main>
      <header className="topbar">
        <div className="search"><Search size={18}/><input value={search} onChange={e=>setSearch(e.target.value)} placeholder="Buscar títulos, años y series…"/>{search&&<button onClick={()=>setSearch('')}><X size={16}/></button>}</div>
        <button className={`scan-button ${scan.running?'working':''}`} onClick={rescan}>{scan.running?<><X size={17}/>Cancelar escaneo</>:<><RefreshCw size={17}/>Reescanear biblioteca</>}</button>
      </header>

      {scan.running&&<div className="scan-strip"><LoaderCircle className="spin" size={16}/><div><b>{scan.message||'Escaneando biblioteca'}</b><small>{scan.currentFile||`${scan.found} archivos encontrados`}</small></div><div className="scan-meter"><i style={{width:`${scan.percent}%`}}/></div><span>{Math.round(scan.percent)}%</span></div>}
      {error&&<div className="error-banner"><CircleAlert size={18}/><span>{error}</span><button onClick={()=>setError(null)}><X size={16}/></button></div>}

      {!boot?<Loading/>:page==='Configuración'?<SettingsPage boot={boot} scan={scan} onRescan={rescan} onChoose={chooseFolder}/>:page==='Series'?<SeriesPage series={home.series} search={search}/>:page==='Inicio'&&!search?<HomePage home={home} hero={hero} heroIndex={heroIndex} setHeroIndex={setHeroIndex} openDetail={openDetail} setFlag={setFlag} playerAvailable={boot.playerAvailable}/>:<CatalogPage title={page} items={visible} openDetail={openDetail} setFlag={setFlag}/>} 
    </main>
    {detail&&<DetailModal detail={detail} playerAvailable={!!boot?.playerAvailable} close={()=>setDetail(null)} setFlag={setFlag}/>} 
  </div>;
}

function Loading(){return <div className="loading-screen"><LoaderCircle className="spin"/><b>Preparando tu biblioteca</b><span>La primera lectura puede tardar unos segundos.</span></div>}

function HomePage({home,hero,heroIndex,setHeroIndex,openDetail,setFlag,playerAvailable}:{home:HomeDto;hero?:MediaSummary;heroIndex:number;setHeroIndex:(n:number)=>void;openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void;playerAvailable:boolean}){
  return <div className="content home-page">
    {hero?<section className="hero" style={{'--hero-hue':hueFor(hero.title)} as React.CSSProperties}>
      <div className="hero-noise"/><div className="hero-copy"><span className="eyebrow">DESTACADO DE TU BIBLIOTECA</span><h1>{displayTitle(hero)}</h1><p>{hero.year||'Año sin identificar'} · {quality(hero)} {hero.technical.hdrType?`· ${hero.technical.hdrType}`:''}</p>
      <div className="hero-actions"><button className="primary" disabled={!playerAvailable}><Play fill="currentColor" size={18}/>{playerAvailable?'Reproducir':'Reproductor en preparación'}</button><button onClick={()=>openDetail(hero.id)}>Ver detalles</button></div></div>
      <div className="hero-controls"><button onClick={()=>setHeroIndex((heroIndex-1+home.heroes.length)%home.heroes.length)}><ChevronLeft/></button><div>{home.heroes.map((_,i)=><button key={i} className={i===heroIndex?'active':''} onClick={()=>setHeroIndex(i)}/>)}</div><button onClick={()=>setHeroIndex((heroIndex+1)%home.heroes.length)}><ChevronRight/></button></div>
    </section>:<EmptyLibrary/>}
    <MediaRow title="Continuar viendo" items={home.continueWatching} openDetail={openDetail} setFlag={setFlag}/>
    <MediaRow title="Agregadas recientemente" items={home.recentlyAdded} openDetail={openDetail} setFlag={setFlag}/>
    <MediaRow title="Películas" items={home.movies} openDetail={openDetail} setFlag={setFlag}/>
    <SeriesRow title="Series" series={home.series}/>
    <MediaRow title="Favoritos" items={home.favorites} openDetail={openDetail} setFlag={setFlag}/>
  </div>
}

function EmptyLibrary(){return <section className="empty-library"><Library size={42}/><h1>Tu sala está lista</h1><p>Conectá la carpeta predeterminada o elegí otra desde Configuración y después reescaneá.</p></section>}
function MediaRow(props:{title:string;items:MediaSummary[];openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void}){if(!props.items.length)return null;return <section className="media-section"><div className="section-title"><h2>{props.title}</h2><span>{props.items.length}</span></div><div className="card-row">{props.items.map(i=><MediaCard key={i.id} item={i} {...props}/>)}</div></section>}
function SeriesRow({title,series}:{title:string;series:SeriesSummary[]}){if(!series.length)return null;return <section className="media-section"><div className="section-title"><h2>{title}</h2><span>{series.length}</span></div><div className="card-row">{series.map(s=><article className="media-card" key={s.title}><Poster title={s.title} label="SERIE"/><h3>{s.title}</h3><p>{s.seasons} temporada{s.seasons===1?'':'s'} · {s.episodes} episodios</p></article>)}</div></section>}

function CatalogPage({title,items,openDetail,setFlag}:{title:string;items:MediaSummary[];openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void}){return <div className="content catalog-page"><div className="page-heading"><div><span className="eyebrow">TU BIBLIOTECA</span><h1>{title}</h1></div><span>{items.length} resultados</span></div>{items.length?<div className="card-grid">{items.map(i=><MediaCard key={i.id} item={i} openDetail={openDetail} setFlag={setFlag}/>)}</div>:<div className="empty-results"><Film/><h2>No hay contenido en esta sección</h2><p>Los elementos aparecerán después del próximo escaneo.</p></div>}</div>}
function SeriesPage({series,search}:{series:SeriesSummary[];search:string}){const q=search.toLocaleLowerCase();const list=series.filter(s=>s.title.toLocaleLowerCase().includes(q));return <div className="content catalog-page"><div className="page-heading"><div><span className="eyebrow">EPISODIOS AGRUPADOS</span><h1>Series</h1></div><span>{list.length} series</span></div><div className="card-grid">{list.map(s=><article className="media-card" key={s.title}><Poster title={s.title} label="SERIE"/><h3>{s.title}</h3><p>{s.seasons} temporadas · {s.episodes} episodios</p></article>)}</div></div>}

function MediaCard({item,openDetail,setFlag}:{item:MediaSummary;openDetail:(id:string)=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void}){return <article className="media-card"><button className="poster-button" onClick={()=>openDetail(item.id)}><Poster title={displayTitle(item)} label={item.kind==='episode'?'SERIE':quality(item)}/>{item.progressPercent>0&&<span className="card-progress"><i style={{width:`${item.progressPercent}%`}}/></span>}<span className="hover-play"><Play fill="currentColor"/></span></button><div className="card-copy"><div><h3>{displayTitle(item)}</h3><p>{item.year||'Sin año'}{item.kind==='episode'?` · T${item.seasonNumber} E${item.episodeNumber}`:''}</p></div><div className="quick-actions"><button className={item.favorite?'selected':''} title="Favorito" onClick={()=>void setFlag(item,'favorite')}><Heart size={15} fill={item.favorite?'currentColor':'none'}/></button><button className={item.inWatchlist?'selected':''} title="Mi lista" onClick={()=>void setFlag(item,'watchlist')}><Bookmark size={15} fill={item.inWatchlist?'currentColor':'none'}/></button></div></div></article>}
function Poster({title,label}:{title:string;label:string}){return <div className="poster" style={{'--poster-hue':hueFor(title)} as React.CSSProperties}><span className="poster-label">{label}</span><b>{initials(title)}</b><small>{title}</small></div>}

function SettingsPage({boot,scan,onRescan,onChoose}:{boot:Bootstrap;scan:ScanProgress;onRescan:()=>void;onChoose:()=>void}){const root=boot.roots.find(r=>r.enabled)||boot.roots[0];return <div className="content settings-page"><div className="page-heading"><div><span className="eyebrow">CINE WANA</span><h1>Configuración</h1></div></div><section className="settings-card"><div className="settings-icon"><FolderCog/></div><div className="settings-main"><div className="settings-title"><div><h2>Biblioteca</h2><p>Carpeta activa y lectura recursiva</p></div><span className={`root-status ${root?.status}`}>{root?.status==='online'?'Conectada':root?.status==='scanning'?'Escaneando':'Desconectada'}</span></div><div className="path-box"><code>{root?.localPath||'Sin carpeta configurada'}</code><button onClick={onChoose}>Cambiar carpeta</button></div><div className="settings-stats"><div><small>Último escaneo</small><b>{root?.lastScanAt?new Date(root.lastScanAt).toLocaleString('es-AR'):'Todavía no finalizó'}</b></div><div><small>Subcarpetas</small><b>{root?.recursive?'Incluidas':'Excluidas'}</b></div><div><small>Archivos desconectados</small><b>{root?.disconnectedCount||0}</b></div></div><button className="primary settings-rescan" onClick={onRescan}>{scan.running?<><X/>Cancelar escaneo</>:<><RefreshCw/>Reescanear biblioteca</>}</button></div></section><section className="settings-card compact"><div className="settings-icon"><Film/></div><div className="settings-main"><div className="settings-title"><div><h2>Componentes multimedia</h2><p>Diagnóstico del entorno de desarrollo</p></div></div><div className="diagnostic-row"><span>FFmpeg / ffprobe</span><b className={boot.ffprobeAvailable?'ok':'pending'}>{boot.ffprobeAvailable?<><Check/>Disponible</>:<><CircleAlert/>Pendiente</>}</b></div><div className="diagnostic-row"><span>Reproductor libmpv</span><b className={boot.playerAvailable?'ok':'pending'}>{boot.playerAvailable?<><Check/>Disponible</>:<><CircleAlert/>Pendiente de integrar</>}</b></div></div></section></div>}

function DetailModal({detail,playerAvailable,close,setFlag}:{detail:MediaDetail;playerAvailable:boolean;close:()=>void;setFlag:(i:MediaSummary,f:'favorite'|'watchlist')=>void}){return <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)close();}}><section className="detail-modal"><button className="modal-close" onClick={close}><X/></button><div className="detail-art"><Poster title={displayTitle(detail)} label={detail.kind==='episode'?'SERIE':quality(detail)}/></div><div className="detail-copy"><span className="eyebrow">{detail.kind==='episode'?`TEMPORADA ${detail.seasonNumber} · EPISODIO ${detail.episodeNumber}`:'PELÍCULA'}</span><h1>{displayTitle(detail)}</h1><div className="detail-meta"><span>{detail.year||'Sin año'}</span><span>{quality(detail)}</span>{detail.technical.hdrType&&<span>{detail.technical.hdrType}</span>}{detail.runtimeMs&&<span>{formatDuration(detail.runtimeMs)}</span>}</div><p className="overview">{detail.overview||'Todavía no hay una sinopsis disponible. CINE WANA conserva el título derivado del archivo hasta que se configuren metadatos.'}</p><div className="detail-actions"><button className="primary" disabled={!playerAvailable}><Play fill="currentColor"/>{playerAvailable?'Reproducir':'Reproductor en preparación'}</button><button className={detail.inWatchlist?'selected':''} onClick={()=>void setFlag(detail,'watchlist')}><Bookmark/>Mi lista</button><button className={detail.favorite?'selected':''} onClick={()=>void setFlag(detail,'favorite')}><Heart/>Favorito</button></div><div className="technical"><h3>Información técnica</h3><dl><div><dt>Archivo</dt><dd>{detail.fileName}</dd></div><div><dt>Contenedor</dt><dd>{detail.technical.container||'Pendiente de ffprobe'}</dd></div><div><dt>Video</dt><dd>{detail.technical.videoCodec||'Sin analizar'}</dd></div><div><dt>Audio</dt><dd>{detail.technical.audioCodec||'Sin analizar'}</dd></div><div><dt>Subtítulos externos</dt><dd>{detail.tracks.filter(t=>t.external).length}</dd></div></dl></div></div></section></div>}

const displayTitle=(item:MediaSummary)=>item.kind==='episode'?(item.seriesTitle||item.title):item.title;
const initials=(value:string)=>value.split(/\s+/).filter(Boolean).slice(0,3).map(v=>v[0]).join('').toUpperCase();
const hueFor=(value:string)=>[...value].reduce((a,c)=>a+c.charCodeAt(0),0)%360;
const quality=(item:MediaSummary)=>item.technical.height?(item.technical.height>=2160?'4K':`${item.technical.height}p`):'ORIGINAL';
const formatDuration=(ms:number)=>`${Math.floor(ms/3600000)} h ${Math.round((ms%3600000)/60000)} min`;

