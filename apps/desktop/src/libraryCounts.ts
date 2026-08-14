import type { MediaSummary, SeriesSummary } from './types';

export interface LibraryCounts {
  files: number;
  movies: number;
  series: number;
  chapters: number;
}

export function countLibrary(movies: readonly MediaSummary[], series: readonly SeriesSummary[]): LibraryCounts {
  const chapters = series.reduce((total, item) => total + item.episodes, 0);
  return {
    files: movies.length + chapters,
    movies: movies.length,
    series: series.length,
    chapters,
  };
}
