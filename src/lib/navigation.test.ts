import { describe, expect, it } from 'vitest';
import { GAME_VIEW_SECTION, viewForOpenedGame } from './navigation';
import type { SidebarView } from './types';

const SECTIONS: SidebarView[] = ['library', 'tests', 'profiles', 'benchmarks'];

describe('viewForOpenedGame', () => {
  // Regresión: buscar en el buscador estando en pruebas/perfiles/benchmarks
  // cargaba el juego pero no se veía nada, porque la vista de juego solo se
  // pinta en la biblioteca.
  it('lleva a la biblioteca desde cualquier sección', () => {
    for (const from of SECTIONS) {
      expect(viewForOpenedGame(from)).toBe('library');
    }
  });

  it('la sección de la vista de juego es la biblioteca', () => {
    expect(GAME_VIEW_SECTION).toBe('library');
  });

  it('desde la biblioteca se queda en la biblioteca', () => {
    expect(viewForOpenedGame('library')).toBe('library');
  });
});
