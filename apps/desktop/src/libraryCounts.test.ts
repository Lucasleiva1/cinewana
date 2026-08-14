import { describe, expect, it } from 'vitest';
import type { MediaSummary, SeriesSummary } from './types';
import { countLibrary } from './libraryCounts';

describe('library counts', () => {
  it('separates movies, distinct series, and all chapters', () => {
    const movies = [{ id: 'movie-1' }, { id: 'movie-2' }] as MediaSummary[];
    const series = [
      { title: 'Serie A', episodes: 18 },
      { title: 'Serie B', episodes: 7 },
    ] as SeriesSummary[];

    expect(countLibrary(movies, series)).toEqual({
      files: 27,
      movies: 2,
      series: 2,
      chapters: 25,
    });
  });
});
