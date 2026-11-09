import { describe, expect, it } from 'vitest';
import {
  NEUTRAL_IMAGE, clampImageValue, formatDelay, isNeutralImage, languageName, trackDetail, trackLabel,
  type EmbeddedTrack,
} from './embeddedPlayer';

const track = (partial: Partial<EmbeddedTrack>): EmbeddedTrack => ({
  id: 1, kind: 'audio', external: false, selected: false, defaultTrack: false, forced: false, hearingImpaired: false, ...partial,
});

describe('textos de pistas', () => {
  it('traduce los códigos de idioma al castellano', () => {
    expect(languageName('spa')).toBe('Español');
    expect(languageName('eng')).toBe('Inglés');
    expect(languageName('pt-BR')).toBe('Portugués de Brasil');
    expect(languageName('es-419')).toBe('Español latino');
    expect(languageName('und')).toBe('');
    expect(languageName(undefined)).toBe('');
  });

  it('arma el nombre de la pista sin repetir el idioma', () => {
    expect(trackLabel(track({ language: 'eng', title: 'English' }), 1)).toBe('Inglés');
    expect(trackLabel(track({ language: 'spa', title: 'Latino' }), 1)).toBe('Español · Latino');
    expect(trackLabel(track({ language: 'spa', title: 'Español latino' }), 1)).toBe('Español latino');
    expect(trackLabel(track({ kind: 'sub' }), 3)).toBe('Subtítulo 3');
    expect(trackLabel(track({}), 2)).toBe('Audio 2');
  });

  it('describe formato, canales y origen', () => {
    expect(trackDetail(track({ codec: 'ac3', channels: 6, defaultTrack: true }))).toBe('Dolby Digital · 5.1 · Principal');
    expect(trackDetail(track({ kind: 'sub', codec: 'subrip', external: true, forced: true }))).toBe('SRT · Forzados · Archivo aparte');
  });
});

describe('ajustes de imagen', () => {
  it('reconoce la imagen original y limita los valores', () => {
    expect(isNeutralImage(NEUTRAL_IMAGE)).toBe(true);
    expect(isNeutralImage({ ...NEUTRAL_IMAGE, deband: true })).toBe(false);
    expect(clampImageValue('brightness', 80)).toBe(50);
    expect(clampImageValue('sharpness', -5)).toBe(0);
    expect(clampImageValue('sharpness', 70.4)).toBe(70);
  });

  it('muestra el desfase de subtítulos con coma', () => {
    expect(formatDelay(0)).toBe('0 s');
    expect(formatDelay(0.5)).toBe('+0,5 s');
    expect(formatDelay(-1.25)).toBe('−1,3 s');
  });
});
