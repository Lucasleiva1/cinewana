// @vitest-environment jsdom

import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IdentificationReviewCard } from './App';
import type { IdentificationReview, MediaMetadataCandidate } from './types';

const candidate: MediaMetadataCandidate = {
  id: 'tmdb:movie:348',
  language: 'es-AR',
  pageId: 348,
  title: 'Alien: El octavo pasajero',
  year: 1979,
  description: 'La tripulación encuentra una forma de vida desconocida.',
  sourceUrl: 'https://www.themoviedb.org/movie/348',
  posterUrl: 'https://image.tmdb.org/t/p/w342/poster.jpg',
};

const review: IdentificationReview = {
  mediaId: 'media-1',
  fileName: 'Alien.1979.mkv',
  kind: 'movie',
  title: 'Alien',
  reason: 'TMDB encontró varias posibilidades.',
  identificationPending: true,
  metadataStatus: 'ambiguous',
  metadataCandidates: [candidate],
};

describe('identification review candidate selection', () => {
  it('applies the clicked poster and confirms the change inside Settings', async () => {
    const onApplyCandidate = vi.fn(async () => undefined);

    render(
      <IdentificationReviewCard
        review={review}
        onResolve={vi.fn(async () => undefined)}
        onApplyCandidate={onApplyCandidate}
        onRetryMetadata={vi.fn(async () => undefined)}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Usar portada Alien: El octavo pasajero' }));

    expect(onApplyCandidate).toHaveBeenCalledWith('media-1', candidate);
    expect(await screen.findByText('Portada aplicada: Alien: El octavo pasajero.')).toBeTruthy();
  });
});
