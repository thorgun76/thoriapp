import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

// El hook llama a Tauri; se simula el módulo entero antes de importarlo.
const mocks = vi.hoisted(() => ({
  getRules: vi.fn(),
  gameInfo: vi.fn(),
  detectEngine: vi.fn(),
  getEngineRecommendations: vi.fn(),
  getCommunityRecommendations: vi.fn(),
}));

vi.mock('../lib/api', () => ({ api: mocks }));

import { useGameDetails } from './useGameDetails';

/** Juego de la Steam Store: no instalado y con appid real. */
const storeGame = { appid: 346110, name: 'ARK: Survival Evolved' };

const rules = { appid: 346110, name: 'ARK', params: [], has_rules: true };

describe('useGameDetails', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getRules.mockResolvedValue(rules);
    mocks.detectEngine.mockResolvedValue(null);
    mocks.getEngineRecommendations.mockResolvedValue([]);
    mocks.getCommunityRecommendations.mockResolvedValue([]);
  });

  it('abre un juego de la Store aunque ProtonDB falle', async () => {
    // Regresión: game_info lanza → info quedaba null y App.tsx nunca pintaba
    // la vista (la condición es `current && rules && info`). El usuario veía
    // el desplegable pero al hacer clic no pasaba nada.
    mocks.gameInfo.mockRejectedValue('ProtonDB: timeout');

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.openGame(storeGame);
    });

    expect(result.current.current).toEqual(storeGame);
    expect(result.current.rules).not.toBeNull();
    // Lo importante: `info` debe existir con un valor por defecto, no null.
    expect(result.current.info).not.toBeNull();
    expect(result.current.info?.errors?.length).toBeGreaterThan(0);
    expect(result.current.loading).toBe(false);
  });

  it('abre un juego de la Store cuando todo responde bien', async () => {
    mocks.gameInfo.mockResolvedValue({ name: 'ARK', native: false, errors: [] });

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.openGame(storeGame);
    });

    expect(result.current.current?.name).toBe('ARK: Survival Evolved');
    expect(result.current.info?.native).toBe(false);
    expect(result.current.loading).toBe(false);
  });

  it('si las reglas fallan, avisa en lugar de quedarse cargando', async () => {
    mocks.getRules.mockRejectedValue('backend caído');
    mocks.gameInfo.mockResolvedValue({ name: 'ARK', native: false, errors: [] });

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.openGame(storeGame);
    });

    // Nunca debe quedarse en loading infinito.
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeTruthy();
  });

  it('descarta la respuesta de un juego que ya se abandonó', async () => {
    let resolveFirst: (v: unknown) => void = () => {};
    mocks.getRules.mockImplementationOnce(
      () => new Promise((res) => { resolveFirst = res; })
    );
    mocks.gameInfo.mockResolvedValue({ name: 'lento', native: false, errors: [] });

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [] }));

    let first: Promise<void>;
    await act(async () => {
      first = result.current.openGame({ appid: 1, name: 'Lento' });
    });

    // El usuario cambia de juego antes de que responda el primero.
    await act(async () => {
      await result.current.openGame({ appid: 2, name: 'Rápido' });
    });

    await act(async () => {
      resolveFirst({ appid: 1, name: 'Lento', params: [], has_rules: true });
      await first!;
    });

    // Debe seguir mostrando el segundo juego, no el primero.
    expect(result.current.current?.name).toBe('Rápido');
  });
});
