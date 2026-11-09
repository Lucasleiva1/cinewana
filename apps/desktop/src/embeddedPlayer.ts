/* Contrato con el reproductor incrustado (libmpv dentro de la ventana) y textos en castellano para
   las pistas de audio y subtítulos. La lógica de reproducción vive en Rust; acá solo se traduce. */

export interface EmbeddedTrack {
  id: number;
  kind: 'video' | 'audio' | 'sub' | string;
  language?: string | null;
  title?: string | null;
  codec?: string | null;
  channels?: number | null;
  external: boolean;
  selected: boolean;
  defaultTrack: boolean;
  forced: boolean;
  hearingImpaired: boolean;
}

export interface EmbeddedState {
  session: number;
  loaded: boolean;
  videoReady: boolean;
  position: number;
  duration: number;
  paused: boolean;
  ended: boolean;
  buffering: boolean;
  volume: number;
  muted: boolean;
  speed: number;
  audioId?: number | null;
  subtitleId?: number | null;
  subtitleScale: number;
  subtitleDelay: number;
  subtitleStyle: SubtitleStyle;
  hardwareDecoder?: string | null;
  tracks: EmbeddedTrack[];
  error?: string | null;
}

export type SubtitleColor = 'blanco' | 'amarillo';
export type SubtitleFont = 'clasica' | 'montserrat' | 'segoe' | 'verdana';
/* position: 100 es abajo de todo; cuanto menor, más arriba (en % de la pantalla). */
export interface SubtitleStyle { color: SubtitleColor; font: SubtitleFont; position: number }
export const DEFAULT_SUBTITLE_STYLE: SubtitleStyle = { color: 'blanco', font: 'clasica', position: 100 };
export const SUBTITLE_COLORS: Array<{ value: SubtitleColor; label: string }> = [
  { value: 'blanco', label: 'Blanco' }, { value: 'amarillo', label: 'Amarillo' },
];
export const SUBTITLE_FONTS: Array<{ value: SubtitleFont; label: string }> = [
  { value: 'clasica', label: 'Clásica' }, { value: 'montserrat', label: 'Montserrat' },
  { value: 'segoe', label: 'Segoe UI' }, { value: 'verdana', label: 'Verdana' },
];
/* La altura elegida, en la escala del control (0 = abajo de todo, 60 = bastante más arriba). */
export const SUBTITLE_HEIGHT_MAX = 60;

export interface EmbeddedStatus { available: boolean; message?: string | null }

export type MpvScaling = 'normal' | 'alta' | 'suave';
export type MpvAspect = 'original' | '16:9' | '4:3' | '2.35:1' | '1.85:1';
export type MpvToneMapping = 'auto' | 'bt.2390' | 'hable' | 'mobius';

export interface MpvImage {
  brightness: number;
  contrast: number;
  saturation: number;
  gamma: number;
  hue: number;
  shadows: number;
  highlights: number;
  temperature: number;
  sharpness: number;
  scaling: MpvScaling;
  antiring: boolean;
  deband: boolean;
  debandStrength: number;
  deinterlace: boolean;
  smoothMotion: boolean;
  fillScreen: boolean;
  aspect: MpvAspect;
  zoom: number;
  rotate: number;
  toneMapping: MpvToneMapping;
  hdrAutoPeak: boolean;
}

export type MpvImageNumber = 'brightness' | 'contrast' | 'saturation' | 'gamma' | 'hue' | 'shadows' | 'highlights' | 'temperature' | 'sharpness' | 'debandStrength' | 'zoom';
export type MpvImageSwitch = 'antiring' | 'deband' | 'deinterlace' | 'smoothMotion' | 'fillScreen' | 'hdrAutoPeak';

/* La imagen tal cual viene en el archivo: lo mismo que mpv hace de fábrica. */
export const NEUTRAL_IMAGE: MpvImage = {
  brightness: 0, contrast: 0, saturation: 0, gamma: 0, hue: 0, shadows: 0, highlights: 0, temperature: 0, sharpness: 0,
  scaling: 'normal', antiring: false, deband: false, debandStrength: 35, deinterlace: false, smoothMotion: false,
  fillScreen: false, aspect: 'original', zoom: 0, rotate: 0, toneMapping: 'auto', hdrAutoPeak: true,
};

export interface ImageSliderSpec { key: MpvImageNumber; label: string; min: number; max: number; unit?: string }
export interface ImageSwitchSpec { key: MpvImageSwitch; label: string; detail: string }
export interface ImageChoiceSpec<T extends string | number> { value: T; label: string }

export const LIGHT_SLIDERS: ImageSliderSpec[] = [
  { key: 'brightness', label: 'Brillo', min: -50, max: 50 },
  { key: 'contrast', label: 'Contraste', min: -50, max: 50 },
  { key: 'saturation', label: 'Saturación', min: -50, max: 50 },
  { key: 'gamma', label: 'Gamma', min: -50, max: 50 },
  { key: 'hue', label: 'Tono del color', min: -50, max: 50 },
  { key: 'temperature', label: 'Temperatura', min: -50, max: 50 },
];

export const SHADING_SLIDERS: ImageSliderSpec[] = [
  { key: 'shadows', label: 'Sombras', min: -50, max: 50 },
  { key: 'highlights', label: 'Luces', min: -50, max: 50 },
];

export const SHARPNESS_SLIDER: ImageSliderSpec = { key: 'sharpness', label: 'Nitidez', min: 0, max: 100 };
export const DEBAND_SLIDER: ImageSliderSpec = { key: 'debandStrength', label: 'Intensidad', min: 0, max: 100 };
export const ZOOM_SLIDER: ImageSliderSpec = { key: 'zoom', label: 'Zoom', min: 0, max: 100, unit: '%' };

export const SCALING_CHOICES: ImageChoiceSpec<MpvScaling>[] = [
  { value: 'normal', label: 'Normal' }, { value: 'alta', label: 'Alta calidad' }, { value: 'suave', label: 'Suave' },
];
export const ASPECT_CHOICES: ImageChoiceSpec<MpvAspect>[] = [
  { value: 'original', label: 'Original' }, { value: '16:9', label: '16:9' }, { value: '4:3', label: '4:3' },
  { value: '2.35:1', label: '2.35:1' }, { value: '1.85:1', label: '1.85:1' },
];
export const ROTATE_CHOICES: ImageChoiceSpec<number>[] = [
  { value: 0, label: '0°' }, { value: 90, label: '90°' }, { value: 180, label: '180°' }, { value: 270, label: '270°' },
];
export const TONE_MAPPING_CHOICES: ImageChoiceSpec<MpvToneMapping>[] = [
  { value: 'auto', label: 'Automática' }, { value: 'bt.2390', label: 'Natural' }, { value: 'hable', label: 'Cine' }, { value: 'mobius', label: 'Suave' },
];

export const SWITCHES: Record<MpvImageSwitch, ImageSwitchSpec> = {
  antiring: { key: 'antiring', label: 'Reducir halos', detail: 'Quita el borde claro que aparece alrededor de las figuras al agrandar la imagen.' },
  deband: { key: 'deband', label: 'Quitar bandas', detail: 'Suaviza los escalones de color en cielos, sombras y paredes lisas.' },
  deinterlace: { key: 'deinterlace', label: 'Desentrelazar', detail: 'Para grabaciones de TV o DVD que se ven con rayitas horizontales al moverse.' },
  smoothMotion: { key: 'smoothMotion', label: 'Movimiento suave', detail: 'Menos saltitos en los paneos de cámara. Usa más la placa de video.' },
  fillScreen: { key: 'fillScreen', label: 'Llenar la pantalla', detail: 'Quita las franjas negras recortando un poco los bordes.' },
  hdrAutoPeak: { key: 'hdrAutoPeak', label: 'Brillo automático por escena', detail: 'Mide cada escena para que las partes muy brillantes no se quemen.' },
};

/* Los controles deslizables, para el control remoto del celular. */
export const ALL_IMAGE_SLIDERS = [...LIGHT_SLIDERS, ...SHADING_SLIDERS, SHARPNESS_SLIDER];

export function isNeutralImage(image: MpvImage) {
  return (Object.keys(NEUTRAL_IMAGE) as Array<keyof MpvImage>).every(key => image[key] === NEUTRAL_IMAGE[key]);
}

export function clampImageValue(key: MpvImageNumber, value: number) {
  const spec = [...ALL_IMAGE_SLIDERS, DEBAND_SLIDER, ZOOM_SLIDER].find(item => item.key === key);
  if (!spec || !Number.isFinite(value)) return 0;
  return Math.max(spec.min, Math.min(spec.max, Math.round(value)));
}

const LANGUAGE_NAMES: Record<string, string> = {
  spa: 'Español', es: 'Español', esp: 'Español', 'es-es': 'Español de España', 'es-mx': 'Español de México',
  lat: 'Español latino', 'es-419': 'Español latino', 'es-la': 'Español latino', 'es-us': 'Español latino',
  eng: 'Inglés', en: 'Inglés', 'en-us': 'Inglés', 'en-gb': 'Inglés británico',
  por: 'Portugués', pt: 'Portugués', 'pt-br': 'Portugués de Brasil', 'pt-pt': 'Portugués de Portugal',
  fre: 'Francés', fra: 'Francés', fr: 'Francés', ita: 'Italiano', it: 'Italiano',
  ger: 'Alemán', deu: 'Alemán', de: 'Alemán', jpn: 'Japonés', ja: 'Japonés', kor: 'Coreano', ko: 'Coreano',
  chi: 'Chino', zho: 'Chino', zh: 'Chino', rus: 'Ruso', ru: 'Ruso', cat: 'Catalán', ca: 'Catalán',
  glg: 'Gallego', gl: 'Gallego', eus: 'Euskera', baq: 'Euskera', ara: 'Árabe', ar: 'Árabe', hin: 'Hindi', hi: 'Hindi',
  tur: 'Turco', tr: 'Turco', pol: 'Polaco', pl: 'Polaco', dut: 'Neerlandés', nld: 'Neerlandés', nl: 'Neerlandés',
  swe: 'Sueco', sv: 'Sueco', nor: 'Noruego', nob: 'Noruego', no: 'Noruego', dan: 'Danés', da: 'Danés',
  fin: 'Finlandés', fi: 'Finlandés', gre: 'Griego', ell: 'Griego', el: 'Griego', heb: 'Hebreo', he: 'Hebreo',
  tha: 'Tailandés', th: 'Tailandés', vie: 'Vietnamita', vi: 'Vietnamita', ukr: 'Ucraniano', uk: 'Ucraniano',
  cze: 'Checo', ces: 'Checo', cs: 'Checo', hun: 'Húngaro', hu: 'Húngaro', rum: 'Rumano', ron: 'Rumano', ro: 'Rumano',
  ind: 'Indonesio', id: 'Indonesio',
};

/* Títulos que solo repiten el idioma ("English", "Spanish"): no suman nada al lado del nombre. */
const PLAIN_LANGUAGE_TITLES = new Set([
  'english', 'ingles', 'spanish', 'espanol', 'portuguese', 'portugues', 'french', 'francais', 'frances',
  'italian', 'italiano', 'german', 'deutsch', 'aleman', 'japanese', 'japones', 'russian', 'ruso', 'chinese', 'chino',
  'korean', 'coreano', 'catalan', 'polish', 'polaco', 'dutch', 'turkish', 'turco',
]);

export function languageName(code?: string | null) {
  if (!code) return '';
  const key = code.trim().toLowerCase().replace('_', '-');
  return LANGUAGE_NAMES[key] ?? LANGUAGE_NAMES[key.split('-')[0]] ?? (key === 'und' ? '' : code.toUpperCase());
}

function normalize(text: string) {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').trim().toLowerCase();
}

export function trackLabel(track: EmbeddedTrack, position: number) {
  const language = languageName(track.language);
  const title = track.title?.trim();
  if (title && !PLAIN_LANGUAGE_TITLES.has(normalize(title))) {
    if (!language || normalize(title).includes(normalize(language))) return title;
    return `${language} · ${title}`;
  }
  if (language) return language;
  return track.kind === 'sub' ? `Subtítulo ${position}` : `Audio ${position}`;
}

const CODEC_NAMES: Record<string, string> = {
  ac3: 'Dolby Digital', eac3: 'Dolby Digital Plus', truehd: 'Dolby TrueHD', dts: 'DTS', 'dts-hd': 'DTS-HD',
  aac: 'AAC', mp3: 'MP3', mp2: 'MP2', opus: 'Opus', vorbis: 'Vorbis', flac: 'FLAC',
  subrip: 'SRT', srt: 'SRT', ass: 'ASS', ssa: 'SSA', webvtt: 'WebVTT', mov_text: 'Texto',
  hdmv_pgs_subtitle: 'Imagen (PGS)', dvd_subtitle: 'Imagen (DVD)', dvb_subtitle: 'Imagen (DVB)',
};

export function channelLabel(channels?: number | null) {
  if (!channels) return '';
  if (channels === 1) return 'Mono';
  if (channels === 2) return 'Estéreo';
  if (channels === 6) return '5.1';
  if (channels === 8) return '7.1';
  return `${channels} canales`;
}

export function trackDetail(track: EmbeddedTrack) {
  const codec = track.codec ? (CODEC_NAMES[track.codec.toLowerCase()] ?? (track.codec.startsWith('pcm') ? 'PCM' : track.codec.toUpperCase())) : '';
  return [
    codec,
    track.kind === 'audio' ? channelLabel(track.channels) : '',
    track.forced ? 'Forzados' : '',
    track.hearingImpaired ? 'Para sordos' : '',
    track.external ? 'Archivo aparte' : '',
    track.defaultTrack ? 'Principal' : '',
  ].filter(Boolean).join(' · ');
}

export function tracksOf(state: EmbeddedState | null, kind: 'audio' | 'sub') {
  return (state?.tracks ?? []).filter(track => track.kind === kind);
}

export function formatDelay(seconds: number) {
  if (Math.abs(seconds) < 0.05) return '0 s';
  const text = Math.abs(seconds).toFixed(1).replace('.', ',');
  return `${seconds > 0 ? '+' : '−'}${text} s`;
}
