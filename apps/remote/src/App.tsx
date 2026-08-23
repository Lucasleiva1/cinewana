import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Bookmark, Check, Expand, Film, Heart, Home, Image as ImageIcon,
  LoaderCircle, Pause, Play, RotateCcw, Search, SkipBack, SkipForward, Smartphone, Tv,
  Volume2, VolumeX, Wifi, WifiOff, X
} from 'lucide-react';
import { useRemote } from './useRemote';
import type { ImageSetting, MediaDetail, MediaItem, SeriesItem, View } from './types';

const tabs: Array<{id:View;label:string;icon:typeof Home}>=[
  {id:'home',label:'Inicio',icon:Home},{id:'movies',label:'Películas',icon:Film},{id:'series',label:'Series',icon:Tv},
  {id:'watchlist',label:'Mi lista',icon:Bookmark},{id:'search',label:'Buscar',icon:Search}
];

export function App(){
  const remote=useRemote();
  const [view,setView]=useState<View>('home');
  const [query,setQuery]=useState('');
  const [detail,setDetail]=useState<MediaDetail|null>(null);
  const [selectedSeries,setSelectedSeries]=useState<SeriesItem|null>(null);
  const [detailLoading,setDetailLoading]=useState(false);
  const [sheet,setSheet]=useState<'image'|'audio'|'subtitle'|null>(null);
  const player=remote.player;
  const filtered=useMemo(()=>{
    let list=remote.items;
    if(view==='movies')list=list.filter(item=>item.kind==='movie');
    if(view==='watchlist')list=list.filter(item=>item.inWatchlist);
    if(view==='search'){const needle=query.trim().toLocaleLowerCase();list=needle?list.filter(item=>displayTitle(item).toLocaleLowerCase().includes(needle)||String(item.year||'').includes(needle)):[];}
    return list;
  },[query,remote.items,view]);
  const homeRows=useMemo(()=>[
    ['Continuar viendo',remote.continueWatching],
    ['Agregadas recientemente',remote.recentlyAdded],
    ['Mi lista',remote.items.filter(item=>item.inWatchlist)]
  ] as Array<[string,MediaItem[]]>,[remote.continueWatching,remote.items,remote.recentlyAdded]);
  /* El teléfono respeta el orden elegido en la computadora. Las sagas llegan agrupadas y acá se
     estiran en una sola fila, parte por parte, porque una pantalla chica no da para una fila por saga. */
  const categoryRows=useMemo(()=>remote.categories.map(category=>({
    id:category.id,
    label:category.label,
    kind:category.kind,
    series:category.series,
    items:category.kind==='sagas'?category.sagas.flatMap(saga=>saga.items):category.items
  })),[remote.categories]);

  const openDetail=async(item:MediaItem)=>{setDetailLoading(true);try{setDetail(await remote.loadDetail(item.id));}catch(cause){remote.setError(String(cause));}finally{setDetailLoading(false);}};
  const play=(id:string)=>{if(remote.send({type:'library_play_media',media_id:id})){setDetail(null);setSelectedSeries(null);}};
  const openEpisodeDetail=(item:MediaItem)=>{setSelectedSeries(null);void openDetail(item);};
  const openImage=()=>{setSheet('image');remote.send({type:'player_analyze_image'});};

  if(remote.connection==='unpaired')return <PairScreen error={remote.error} retry={remote.retry}/>;
  if(remote.connection==='pairing')return <PairingScreen pending={Boolean(remote.pairRequestId)} error={remote.error}/>;

  return <div className="remote-shell">
    <header className="app-header"><Brand/><ConnectionPill state={remote.connection}/></header>
    {remote.error&&<div className="inline-error"><WifiOff/><span>{remote.error}</span><button onClick={()=>remote.setError(null)}><X/></button></div>}
    <main>
      {view==='home'?<>
        <NowPlaying player={player} send={remote.send} openSheet={setSheet} openImage={openImage}/>
        {homeRows.map(([title,items])=>items.length?<MediaRow key={title} title={title} items={items.slice(0,18)} openDetail={openDetail} loadArtwork={remote.loadArtwork}/>:null)}
        {categoryRows.length>0&&<nav className="category-strip" aria-label="Categorías">{categoryRows.map(category=><a key={category.id} href={`#categoria-${category.id}`} className={category.kind==='uncategorized'?'pending':''}>{category.label}</a>)}</nav>}
        {categoryRows.map(category=><div key={category.id} id={`categoria-${category.id}`} className="category-anchor">
          {category.items.length>0&&category.kind!=='series'&&<MediaRow title={category.label} items={category.items.slice(0,18)} openDetail={openDetail} loadArtwork={remote.loadArtwork}/>}
          {category.series.length>0&&<SeriesRow title={category.kind==='custom'&&category.items.length>0?`${category.label} · series`:category.label} items={category.series} openSeries={setSelectedSeries} loadArtwork={remote.loadArtwork}/>}
        </div>)}
        {remote.series.length>0&&<SeriesRow title="Todas las series" items={remote.series} openSeries={setSelectedSeries} loadArtwork={remote.loadArtwork}/>}
      </>:<section className="library-view">
        <div className="view-heading"><span className="eyebrow">TU BIBLIOTECA</span><h1>{tabs.find(tab=>tab.id===view)?.label}</h1></div>
        {view==='search'&&<label className="search-box"><Search/><input autoFocus value={query} onChange={event=>setQuery(event.target.value)} placeholder="Buscar títulos o años"/><button className={query?'visible':''} onClick={()=>setQuery('')}><X/></button></label>}
        {view==='series'?(remote.series.length?<div className="media-grid">{remote.series.map(item=><SeriesCard key={item.title} item={item} openSeries={setSelectedSeries} loadArtwork={remote.loadArtwork}/>)}</div>:<EmptyState search={false}/>):filtered.length?<div className="media-grid">{filtered.map(item=><MediaCard key={item.id} item={item} openDetail={openDetail} loadArtwork={remote.loadArtwork}/>)}</div>:<EmptyState search={view==='search'}/>}
      </section>}
    </main>
    <nav className="bottom-nav">{tabs.map(({id,label,icon:Icon})=><button key={id} className={view===id?'active':''} onClick={()=>setView(id)}><Icon/><span>{label}</span></button>)}</nav>
    {detailLoading&&<div className="loading-float"><LoaderCircle className="spin"/></div>}
    {selectedSeries&&<SeriesSheet key={selectedSeries.title} series={selectedSeries} close={()=>setSelectedSeries(null)} play={play} openDetail={openEpisodeDetail} loadArtwork={remote.loadArtwork} loadBackdrop={remote.loadBackdrop}/>}
    {detail&&<DetailSheet detail={detail} close={()=>setDetail(null)} play={play} send={remote.send} loadArtwork={remote.loadArtwork}/>}
    {sheet==='image'&&<ImageSheet settings={player.imageSettings} analyzing={player.imageAnalyzing} percent={player.imageAnalysisPercent} close={()=>setSheet(null)} send={remote.send}/>}
    {sheet==='audio'&&<TrackSheet title="Audio" tracks={player.audioTracks} close={()=>setSheet(null)} select={id=>id&&remote.send({type:'player_set_audio',track_id:id})}/>}
    {sheet==='subtitle'&&<TrackSheet title="Subtítulos" tracks={player.subtitleTracks} allowOff close={()=>setSheet(null)} select={id=>remote.send({type:'player_set_subtitle',track_id:id})}/>}
  </div>;
}

function Brand(){return <div className="brand" aria-label="CINE WANA"><span>CINE</span><strong>WANA</strong></div>}
function ConnectionPill({state}:{state:string}){const online=state==='connected';return <div className={`connection ${online?'online':''}`}>{online?<Wifi/>:<LoaderCircle className="spin"/>}<span>{online?'Conectado':'Reconectando'}</span></div>}

function PairScreen({error,retry}:{error:string|null;retry:()=>void}){return <main className="pair-screen"><Brand/><div className="pair-orbit"><Smartphone/><i/><i/></div><span className="eyebrow">CONTROL REMOTO LOCAL</span><h1>Vinculá este teléfono</h1><p>En la computadora abrí <b>Configuración → Control remoto</b>, activá el servidor y escaneá el QR.</p>{error&&<div className="pair-error">{error}</div>}<button className="primary wide" onClick={retry}><RotateCcw/>Reintentar</button><small>El teléfono y la computadora deben estar en la misma red Wi‑Fi.</small></main>}
function PairingScreen({pending,error}:{pending:boolean;error:string|null}){return <main className="pair-screen"><Brand/><LoaderCircle className="pair-loader spin"/><span className="eyebrow">VINCULACIÓN SEGURA</span><h1>{pending?'Aprobá este teléfono':'Conectando…'}</h1><p>{pending?'Mirá CINE WANA en la computadora y presioná Aprobar en la solicitud pendiente.':'Estamos verificando el código temporal.'}</p>{error&&<div className="pair-error">{error}</div>}<small>No cierres esta pantalla.</small></main>}

/* Mientras el dedo está sobre la barra manda el valor local. Antes cada foto que llegaba de la
   computadora devolvía la perilla al valor anterior, así que el volumen se movía solo hacia atrás
   y había que esperar a que dejaran de llegar actualizaciones para poder cambiarlo. */
function useLiveRange(remote:number,commit:(value:number)=>void){
  const [local,setLocal]=useState<number|null>(null);
  const timer=useRef<number|undefined>(undefined);
  useEffect(()=>()=>window.clearTimeout(timer.current),[]);
  const change=(value:number)=>{window.clearTimeout(timer.current);setLocal(value);commit(value);};
  const release=()=>{window.clearTimeout(timer.current);timer.current=window.setTimeout(()=>setLocal(null),700);};
  return {value:local??remote,change,release};
}

function NowPlaying({player,send,openSheet,openImage}:{player:ReturnType<typeof useRemote>['player'];send:ReturnType<typeof useRemote>['send'];openSheet:(sheet:'image'|'audio'|'subtitle')=>void;openImage:()=>void}){
  /* Los dos controles se preparan antes de cualquier salida temprana: el panel pasa de apagado a
     encendido en cuanto llega la primera foto del reproductor, y si los hooks se saltearan en ese
     paso React perdería el orden justo cuando se abre el control remoto. */
  const position=useLiveRange(Math.min(player.positionSeconds,player.durationSeconds||0),seconds=>send({type:'player_seek_to',seconds}));
  const volume=useLiveRange(player.muted?0:player.volume,value=>send({type:'player_set_volume',volume:value}));
  if(!player.active)return <section className="now-playing empty"><span className="eyebrow">REPRODUCTOR</span><div><Film/><h1>No hay nada reproduciéndose</h1><p>Elegí una película o serie desde el teléfono para verla en la computadora.</p></div></section>;
  const percent=player.durationSeconds?player.positionSeconds/player.durationSeconds*100:0;
  return <section className="now-playing">
    <span className="eyebrow">REPRODUCIENDO AHORA</span>
    <div className="title-line"><div><h1>{player.title}</h1><p>{[player.year,player.quality].filter(Boolean).join(' · ')}</p></div></div>
    {player.nextUp&&<div className="remote-next-up"><div><span>{player.nextUp.label}</span><b>{player.nextUp.title}</b><small>{player.nextUp.position?`${player.nextUp.position} · `:''}Empieza al terminar · {Math.ceil(player.nextUp.secondsRemaining)} s</small></div><div><button className="primary" onClick={()=>send({type:'player_start_next_up'})}><Play fill="currentColor"/>Reproducir ahora</button><button onClick={()=>send({type:'player_cancel_next_up'})}><X/>Cancelar</button></div></div>}
    <label className="progress"><span>{formatTime(player.positionSeconds)}</span><input aria-label="Posición" type="range" min={0} max={Math.max(player.durationSeconds,1)} step={1} value={position.value} onChange={event=>position.change(Number(event.target.value))} onPointerUp={position.release} onPointerCancel={position.release} onTouchEnd={position.release} onBlur={position.release}/><span>{formatTime(player.durationSeconds)}</span><i style={{width:`${percent}%`}}/></label>
    <div className="transport"><button onClick={()=>send({type:'player_seek_by',seconds:-10})}><SkipBack/><small>10</small></button><button className="play" onClick={()=>send({type:'player_toggle'})}>{player.playing?<Pause fill="currentColor"/>:<Play fill="currentColor"/>}</button><button onClick={()=>send({type:'player_seek_by',seconds:10})}><SkipForward/><small>10</small></button></div>
    <div className="volume"><button onClick={()=>send({type:'player_toggle_mute'})}>{player.muted?<VolumeX/>:<Volume2/>}</button><input aria-label="Volumen" type="range" min={0} max={1} step={0.01} value={volume.value} onChange={event=>volume.change(Number(event.target.value))} onPointerUp={volume.release} onPointerCancel={volume.release} onTouchEnd={volume.release} onBlur={volume.release}/><span>{Math.round(volume.value*100)}</span></div>
    <div className="quick-controls">
      {player.subtitleTracks.length>0&&<button onClick={()=>openSheet('subtitle')}><span>CC</span>Subtítulos</button>}
      {player.audioTracks.length>0&&<button onClick={()=>openSheet('audio')}><Volume2/>Audio</button>}
      {player.imageSettings.length>0&&<button onClick={openImage}><ImageIcon/>Imagen</button>}
      <button onClick={()=>send({type:'player_toggle_fullscreen'})}><Expand/>Pantalla</button>
    </div>
  </section>;
}

function MediaRow({title,items,openDetail,loadArtwork}:{title:string;items:MediaItem[];openDetail:(item:MediaItem)=>void;loadArtwork:(id:string)=>Promise<string|null>}){return <section className="media-row"><div className="row-title"><h2>{title}</h2><span>{items.length}</span></div><div className="row-scroll">{items.map(item=><MediaCard key={item.id} item={item} openDetail={openDetail} loadArtwork={loadArtwork}/>)}</div></section>}
function MediaCard({item,openDetail,loadArtwork}:{item:MediaItem;openDetail:(item:MediaItem)=>void;loadArtwork:(id:string)=>Promise<string|null>}){const {attach,src}=useLazyRemoteImage(item.id,item.artworkAvailable,loadArtwork);return <button ref={attach} className="media-card" onClick={()=>openDetail(item)}><div className="poster">{src?<img src={src} alt=""/>:<span>{initials(displayTitle(item))}</span>}{item.progressPercent>0&&<i><b style={{width:`${item.progressPercent}%`}}/></i>}</div><strong>{displayTitle(item)}</strong><small>{item.year||'Sin año'}{item.kind==='episode'?` · T${item.seasonNumber||0} E${item.episodeNumber||0}`:''}</small></button>}
function SeriesRow({title,items,openSeries,loadArtwork}:{title:string;items:SeriesItem[];openSeries:(item:SeriesItem)=>void;loadArtwork:(id:string)=>Promise<string|null>}){return <section className="media-row"><div className="row-title"><h2>{title}</h2><span>{items.length}</span></div><div className="row-scroll">{items.map(item=><SeriesCard key={item.title} item={item} openSeries={openSeries} loadArtwork={loadArtwork}/>)}</div></section>}
function SeriesCard({item,openSeries,loadArtwork}:{item:SeriesItem;openSeries:(item:SeriesItem)=>void;loadArtwork:(id:string)=>Promise<string|null>}){const {attach,src}=useLazyRemoteImage(item.episodeId,item.artworkAvailable,loadArtwork);return <button ref={attach} className="media-card" onClick={()=>openSeries(item)}><div className="poster">{src?<img src={src} alt=""/>:<span>{initials(item.title)}</span>}</div><strong>{item.title}</strong><small>{item.seasons} {item.seasons===1?'temporada':'temporadas'} · {item.episodes} episodios</small></button>}
function EmptyState({search}:{search:boolean}){return <div className="empty-state">{search?<Search/>:<Film/>}<h2>{search?'Buscá una película o serie':'No hay contenido en esta sección'}</h2><p>{search?'Escribí un título o un año.':'Tu biblioteca se actualizará desde la computadora.'}</p></div>}

function SeriesSheet({series,close,play,openDetail,loadArtwork,loadBackdrop}:{series:SeriesItem;close:()=>void;play:(id:string)=>void;openDetail:(item:MediaItem)=>void;loadArtwork:(id:string)=>Promise<string|null>;loadBackdrop:(id:string)=>Promise<string|null>}){
  const [seasonNumber,setSeasonNumber]=useState(series.seasonItems[0]?.seasonNumber??0);
  const poster=useRemoteImage(series.episodeId,series.artworkAvailable,loadArtwork);
  const season=series.seasonItems.find(item=>item.seasonNumber===seasonNumber)??series.seasonItems[0];
  return <Sheet close={close}>
    <div className="series-heading">{poster?<img src={poster} alt=""/>:<div>{initials(series.title)}</div>}<div><span className="eyebrow">SERIE</span><h2>{series.title}</h2><p>{series.seasons} {series.seasons===1?'temporada':'temporadas'} · {series.episodes} episodios</p></div></div>
    <div className="season-tabs" aria-label="Temporadas">{series.seasonItems.map(item=><button key={item.seasonNumber} className={item.seasonNumber===seasonNumber?'active':''} onClick={()=>setSeasonNumber(item.seasonNumber)}>{item.title}</button>)}</div>
    <div className="episode-list">{season?.episodes.map(episode=><EpisodeRow key={episode.id} item={episode} play={play} openDetail={openDetail} loadArtwork={loadArtwork} loadBackdrop={loadBackdrop}/>)}</div>
  </Sheet>;
}

function EpisodeRow({item,play,openDetail,loadArtwork,loadBackdrop}:{item:MediaItem;play:(id:string)=>void;openDetail:(item:MediaItem)=>void;loadArtwork:(id:string)=>Promise<string|null>;loadBackdrop:(id:string)=>Promise<string|null>}){
  const backdrop=useRemoteImage(item.id,item.backdropAvailable,loadBackdrop);
  const poster=useRemoteImage(item.id,!item.backdropAvailable&&item.artworkAvailable,loadArtwork);
  return <article className="episode-row"><div className="episode-thumb">{backdrop||poster?<img src={backdrop||poster||''} alt=""/>:<span>{String(item.episodeNumber??0).padStart(2,'0')}</span>}<button aria-label="Reproducir episodio" onClick={()=>play(item.id)}><Play fill="currentColor"/></button></div><div className="episode-copy"><span>Episodio {item.episodeNumber??'—'}</span><h3>{item.title}</h3><p>{item.overview||'Sin descripción disponible.'}</p><button onClick={()=>openDetail(item)}>Ver información</button></div></article>;
}

function DetailSheet({detail,close,play,send,loadArtwork}:{detail:MediaDetail;close:()=>void;play:(id:string)=>void;send:ReturnType<typeof useRemote>['send'];loadArtwork:(id:string)=>Promise<string|null>}){const src=useArtwork(detail,loadArtwork);return <Sheet close={close}><div className="detail-hero">{src?<img src={src} alt=""/>:<div>{initials(displayTitle(detail))}</div>}<div><span className="eyebrow">{detail.kind==='movie'?'PELÍCULA':'SERIE'}</span><h2>{displayTitle(detail)}</h2><p>{[detail.year,detail.quality,formatDuration(detail.runtimeMs||detail.durationMs)].filter(Boolean).join(' · ')}</p></div></div>{detail.genres.length>0&&<div className="chips">{detail.genres.map(genre=><span key={genre}>{genre}</span>)}</div>}<p className="overview">{detail.overview||'Sin sinopsis disponible.'}</p><div className="detail-actions"><button className="primary" onClick={()=>play(detail.id)}><Play fill="currentColor"/>Reproducir en la computadora</button><button onClick={()=>send({type:'library_set_flag',media_id:detail.id,flag:'watchlist',value:!detail.inWatchlist})}><Bookmark fill={detail.inWatchlist?'currentColor':'none'}/>{detail.inWatchlist?'Quitar de Mi lista':'Agregar a Mi lista'}</button><button onClick={()=>send({type:'library_set_flag',media_id:detail.id,flag:'favorite',value:!detail.favorite})}><Heart fill={detail.favorite?'currentColor':'none'}/>Favorito</button></div></Sheet>}

function ImageSheet({settings,analyzing,percent,close,send}:{settings:ImageSetting[];analyzing:boolean;percent:number;close:()=>void;send:ReturnType<typeof useRemote>['send']}){return <Sheet close={close}><div className="sheet-heading"><div><span className="eyebrow">REPRODUCTOR</span><h2>Imagen</h2></div><button onClick={()=>send({type:'player_reset_image'})}><RotateCcw/>Restablecer</button></div>{analyzing&&<div className="image-analysis-status"><LoaderCircle className="spin"/><span>Escaneando escenas en la computadora…</span><b>{Math.round(percent)}%</b></div>}<div className="image-settings">{settings.map(setting=><label key={setting.id}><span><b>{setting.label}</b><output>{Math.round(setting.value)}</output></span><input type="range" min={setting.min} max={setting.max} step={setting.step} value={setting.value} onChange={event=>send({type:'player_set_image',setting_id:setting.id,value:Number(event.target.value)})}/></label>)}</div></Sheet>}
function TrackSheet({title,tracks,allowOff,close,select}:{title:string;tracks:Array<{id:string;label:string;language?:string;channels?:number;active:boolean}>;allowOff?:boolean;close:()=>void;select:(id:string|null)=>void}){return <Sheet close={close}><div className="sheet-heading"><div><span className="eyebrow">REPRODUCTOR</span><h2>{title}</h2></div></div><div className="track-list">{allowOff&&<button onClick={()=>select(null)}><span>Sin subtítulos</span></button>}{tracks.map(track=><button key={track.id} className={track.active?'active':''} onClick={()=>select(track.id)}><span><b>{track.label}</b><small>{[track.language,track.channels?`${track.channels} canales`:null].filter(Boolean).join(' · ')}</small></span>{track.active&&<Check/>}</button>)}</div></Sheet>}
function Sheet({children,close}:{children:React.ReactNode;close:()=>void}){return <div className="sheet-backdrop" onPointerDown={event=>{if(event.target===event.currentTarget)close();}}><section className="bottom-sheet"><button className="sheet-close" onClick={close}><X/></button><i className="sheet-handle"/>{children}</section></div>}

function useArtwork(item:MediaItem,loadArtwork:(id:string)=>Promise<string|null>){return useRemoteImage(item.id,item.artworkAvailable,loadArtwork);}

/* Descarga la imagen sólo cuando su tarjeta se acerca a la pantalla. Pedirlas todas al abrir
   saturaba el Wi‑Fi y el teléfono quedaba trabado hasta que terminaba: los controles del
   reproductor no respondían y la lista se movía sola mientras iban llegando. */
function useLazyRemoteImage(id:string,available:boolean,loadImage:(id:string)=>Promise<string|null>){
  const [node,setNode]=useState<HTMLElement|null>(null);
  const [visible,setVisible]=useState(false);
  const attach=useCallback((element:HTMLElement|null)=>setNode(element),[]);
  useEffect(()=>{
    if(!node||visible)return;
    if(typeof IntersectionObserver==='undefined'){setVisible(true);return;}
    const observer=new IntersectionObserver(entries=>{
      if(entries.some(entry=>entry.isIntersecting)){setVisible(true);observer.disconnect();}
    },{rootMargin:'320px'});
    observer.observe(node);
    return()=>observer.disconnect();
  },[node,visible]);
  return {attach,src:useRemoteImage(id,available&&visible,loadImage)};
}

function useRemoteImage(id:string,available:boolean,loadImage:(id:string)=>Promise<string|null>){const [src,setSrc]=useState<string|null>(null);useEffect(()=>{let mounted=true;let active:string|null=null;setSrc(null);if(available)void loadImage(id).then(url=>{if(!mounted){if(url)URL.revokeObjectURL(url);return;}active=url;setSrc(url);});return()=>{mounted=false;if(active)URL.revokeObjectURL(active);};},[available,id,loadImage]);return src;}
const displayTitle=(item:MediaItem)=>item.kind==='episode'?(item.seriesTitle||item.title):item.title;
const initials=(title:string)=>title.split(/\s+/).slice(0,2).map(part=>part[0]).join('').toUpperCase();
const formatTime=(seconds:number)=>{if(!Number.isFinite(seconds))return '0:00';const whole=Math.max(0,Math.floor(seconds));const h=Math.floor(whole/3600);const m=Math.floor((whole%3600)/60);const s=whole%60;return h?`${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`:`${m}:${String(s).padStart(2,'0')}`;};
const formatDuration=(ms?:number)=>ms?formatTime(ms/1000):'';
