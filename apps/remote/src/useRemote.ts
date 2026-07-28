import { useCallback, useEffect, useRef, useState } from 'react';
import type { ConnectionState, MediaDetail, MediaItem, PlayerSnapshot, RemoteCommand, SeriesItem, StoredCredentials } from './types';

const CREDENTIALS_KEY = 'cine-wana.remote-device.v1';
const emptyPlayer: PlayerSnapshot = { active:false,positionSeconds:0,durationSeconds:0,playing:false,volume:0.8,muted:false,fullscreen:false,imageAnalyzing:false,imageAnalysisPercent:0,nextUp:null,imageSettings:[],audioTracks:[],subtitleTracks:[] };

function loadCredentials(): StoredCredentials | null {
  try { return JSON.parse(localStorage.getItem(CREDENTIALS_KEY) || 'null') as StoredCredentials | null; } catch { return null; }
}

export function useRemote() {
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectRef = useRef<number | null>(null);
  const attemptsRef = useRef(0);
  const [connection,setConnection] = useState<ConnectionState>('disconnected');
  const [player,setPlayer] = useState<PlayerSnapshot>(emptyPlayer);
  const [items,setItems] = useState<MediaItem[]>([]);
  const [movies,setMovies] = useState<MediaItem[]>([]);
  const [recentlyAdded,setRecentlyAdded] = useState<MediaItem[]>([]);
  const [continueWatching,setContinueWatching] = useState<MediaItem[]>([]);
  const [series,setSeries] = useState<SeriesItem[]>([]);
  const [error,setError] = useState<string | null>(null);
  const [pairRequestId,setPairRequestId] = useState<string | null>(null);

  const connect = useCallback(() => {
    if (reconnectRef.current) window.clearTimeout(reconnectRef.current);
    socketRef.current?.close();
    const url = new URL(window.location.href);
    const pairToken = url.searchParams.get('pair');
    const credentials = loadCredentials();
    if (!pairToken && !credentials) { setConnection('unpaired'); return; }
    setConnection(pairToken ? 'pairing' : attemptsRef.current ? 'reconnecting' : 'disconnected');
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const socket = new WebSocket(`${protocol}//${window.location.host}/ws`);
    socketRef.current = socket;
    socket.addEventListener('open',()=>{
      const hello = pairToken
        ? { type:'pair',pairToken,deviceName:deviceName() }
        : { type:'auth',deviceId:credentials?.deviceId,token:credentials?.token };
      socket.send(JSON.stringify(hello));
    });
    socket.addEventListener('message',event=>{
      let message: Record<string,unknown>;
      try { message=JSON.parse(String(event.data)) as Record<string,unknown>; } catch { return; }
      if(message.type==='paired'){
        const next={deviceId:String(message.deviceId),token:String(message.token)};
        localStorage.setItem(CREDENTIALS_KEY,JSON.stringify(next));
        url.searchParams.delete('pair'); window.history.replaceState({},'',url.pathname);
        setPairRequestId(null);
      } else if(message.type==='authenticated'){
        attemptsRef.current=0;setConnection('connected');setError(null);
      } else if(message.type==='pair_pending'){
        setConnection('pairing');setPairRequestId(String(message.requestId));
      } else if(message.type==='pair_invalid'){
        setConnection('unpaired');setError('El código de vinculación venció. Generá uno nuevo en la computadora.');
      } else if(message.type==='player'){
        setPlayer(message.player as PlayerSnapshot);
      } else if(message.type==='library'){
        setItems((message.items as MediaItem[])||[]);
        setMovies((message.movies as MediaItem[])||[]);
        setRecentlyAdded((message.recentlyAdded as MediaItem[])||[]);
        setContinueWatching((message.continueWatching as MediaItem[])||[]);
        setSeries((message.series as SeriesItem[])||[]);
      } else if(message.type==='error'){
        setError(String(message.message||'No se pudo completar la acción.'));
      } else if(message.type==='session_revoked'){
        localStorage.removeItem(CREDENTIALS_KEY);socketRef.current=null;socket.close();setConnection('unpaired');setError('La computadora revocó esta vinculación.');
      }
    });
    socket.addEventListener('close',()=>{
      if(socketRef.current!==socket)return;
      attemptsRef.current+=1;
      const stillPairing=Boolean(new URL(window.location.href).searchParams.get('pair'));
      setConnection(stillPairing?'pairing':'reconnecting');
      const delay=stillPairing?1800:Math.min(1000*2**Math.min(attemptsRef.current,5),15000);
      reconnectRef.current=window.setTimeout(connect,delay);
    });
    socket.addEventListener('error',()=>setError('No se pudo conectar con CINE WANA. Comprobá que la computadora siga encendida y en la misma red.'));
  },[]);

  useEffect(()=>{connect();return()=>{if(reconnectRef.current)window.clearTimeout(reconnectRef.current);const socket=socketRef.current;socketRef.current=null;socket?.close();};},[connect]);

  const send=useCallback((command:RemoteCommand)=>{
    if(socketRef.current?.readyState!==WebSocket.OPEN){setError('El control está desconectado. Reintentando…');return false;}
    socketRef.current.send(JSON.stringify(command));
    if(navigator.vibrate)navigator.vibrate(10);
    return true;
  },[]);

  useEffect(()=>{if(connection!=='connected')return;let timer:number;const schedule=()=>{const now=new Date();const next=new Date(now);next.setHours(24,0,1,0);timer=window.setTimeout(()=>{send({type:'library_refresh'});schedule();},next.getTime()-now.getTime());};schedule();return()=>window.clearTimeout(timer);},[connection,send]);

  const unlink=useCallback(()=>{
    localStorage.removeItem(CREDENTIALS_KEY);const socket=socketRef.current;socketRef.current=null;socket?.close();setItems([]);setMovies([]);setRecentlyAdded([]);setContinueWatching([]);setSeries([]);setPlayer(emptyPlayer);setConnection('unpaired');
  },[]);

  const authorizedFetch=useCallback(async<T,>(path:string):Promise<T>=>{
    const credentials=loadCredentials();
    if(!credentials)throw new Error('El teléfono no está vinculado.');
    const response=await fetch(path,{headers:{Authorization:`Bearer ${credentials.token}`}});
    if(!response.ok)throw new Error(response.status===401?'La vinculación fue revocada.':'No se pudo cargar la información.');
    return response.json() as Promise<T>;
  },[]);

  const loadDetail=useCallback((id:string)=>authorizedFetch<MediaDetail>(`/api/media/${encodeURIComponent(id)}`),[authorizedFetch]);
  const loadArtwork=useCallback(async(id:string)=>{
    const credentials=loadCredentials();if(!credentials)return null;
    const response=await fetch(`/api/artwork/${encodeURIComponent(id)}`,{headers:{Authorization:`Bearer ${credentials.token}`}});
    return response.ok?URL.createObjectURL(await response.blob()):null;
  },[]);
  const loadBackdrop=useCallback(async(id:string)=>{
    const credentials=loadCredentials();if(!credentials)return null;
    const response=await fetch(`/api/backdrop/${encodeURIComponent(id)}`,{headers:{Authorization:`Bearer ${credentials.token}`}});
    return response.ok?URL.createObjectURL(await response.blob()):null;
  },[]);

  return {connection,player,items,movies,recentlyAdded,continueWatching,series,error,setError,pairRequestId,send,unlink,retry:connect,loadDetail,loadArtwork,loadBackdrop};
}

function deviceName(){
  const ua=navigator.userAgent;
  if(/Android/i.test(ua))return 'Android · Chrome';
  if(/iPhone|iPad/i.test(ua))return 'iPhone · Safari';
  return 'Navegador móvil';
}
