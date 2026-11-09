import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

// El hook llama a Tauri; se simula el módulo entero antes de importarlo.
const mocks = vi.hoisted(() => ({
  getRules: vi.fn(),
  gameInfo: vi.fn(),
  detectEngine: vi.fn(),
  getEngineRecommendations: vi.fn(),
  getCommunityRecommendations: vi.fn(),
  getAnticheatInfo: vi.fn(),
  getProtonPulseReports: vi.fn(),
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
    mocks.getAnticheatInfo.mockResolvedValue({ info: null, catalog_size: 0, source: 'cache' });
    mocks.getProtonPulseReports.mockResolvedValue(null);
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

describe('useGameDetails: anti-cheat', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  it('expone el anti-cheat del juego abierto', async () => {
    mocks.gameInfo.mockResolvedValue({ name: 'ARK', native: true, errors: [] });
    mocks.getAnticheatInfo.mockResolvedValue({
      info: { status: 'Supported', anticheats: ['BattlEye'] },
      catalog_size: 900,
      source: 'network',
    });

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.openGame(storeGame);
    });

    expect(result.current.antiCheat).toEqual({ status: 'Supported', anticheats: ['BattlEye'] });
  });

  it('si falla la consulta de anti-cheat el juego se abre igual', async () => {
    mocks.gameInfo.mockResolvedValue({ name: 'ARK', native: true, errors: [] });
    mocks.getAnticheatInfo.mockRejectedValue('sin conexión');

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.openGame(storeGame);
    });

    expect(result.current.antiCheat).toBeNull();
    expect(result.current.rules).not.toBeNull();
  });

  it('no pide anti-cheat para juegos sin appid (Lutris/Heroic)', async () => {
    mocks.getAnticheatInfo.mockClear();
    mocks.getRules.mockResolvedValue({ ...rules, appid: 0 });

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.openGame({ appid: 0, name: 'Juego de Lutris', launcher: 'lutris' });
    });

    expect(mocks.getAnticheatInfo).not.toHaveBeenCalled();
    expect(result.current.antiCheat).toBeNull();
  });
});

describe('useGameDetails: Proton Pulse filtrado por GPU', () => {
  const summary = {
    vendor: 'amd',
    total: 40,
    matching: 12,
    good: 9,
    borked: 1,
    same_family: 5,
    reports: [
      { gpu: 'AMD Radeon RX 6800 XT (RADV NAVI21)', rating: 'platinum', proton_version: '10.0-3' },
    ],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getRules.mockResolvedValue(rules);
    mocks.detectEngine.mockResolvedValue(null);
    mocks.getEngineRecommendations.mockResolvedValue([]);
    mocks.getCommunityRecommendations.mockResolvedValue([]);
    mocks.getAnticheatInfo.mockResolvedValue({ info: null, catalog_size: 0, source: 'cache' });
    mocks.gameInfo.mockResolvedValue({ name: 'ARK', native: true, errors: [] });
  });

  it('pide los reportes con la GPU del equipo, no sin filtro', async () => {
    mocks.getProtonPulseReports.mockResolvedValue(summary);

    const { result } = renderHook(() =>
      useGameDetails({ getHwTags: () => [], getGpuName: () => 'AMD Radeon RX 6800 XT' })
    );

    await act(async () => {
      await result.current.openGame(storeGame);
    });

    // Sin la GPU, el backend devolvería todos los vendors mezclados: es el bug
    // que esta llamada evita.
    expect(mocks.getProtonPulseReports).toHaveBeenCalledWith(346110, 'AMD Radeon RX 6800 XT');
    await waitFor(() => expect(result.current.protonPulse).toEqual(summary));
  });

  it('un fallo en Proton Pulse no rompe la ficha del juego', async () => {
    mocks.getProtonPulseReports.mockRejectedValue('sin conexión');

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [], getGpuName: () => 'x' }));

    await act(async () => {
      await result.current.openGame(storeGame);
    });

    expect(result.current.protonPulse).toBeNull();
    expect(result.current.protonPulseLoading).toBe(false);
    expect(result.current.rules).not.toBeNull();
  });

  it('no pide reportes para juegos de otro launcher (appid 0)', async () => {
    mocks.getRules.mockResolvedValue({ ...rules, appid: 0 });

    const { result } = renderHook(() => useGameDetails({ getHwTags: () => [], getGpuName: () => 'x' }));

    await act(async () => {
      await result.current.openGame({ appid: 0, name: 'Juego de Lutris', launcher: 'lutris' });
    });

    expect(mocks.getProtonPulseReports).not.toHaveBeenCalled();
    expect(result.current.protonPulse).toBeNull();
  });
});
