import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { api } from '../lib/api';
import { useLibrary } from './useLibrary';

afterEach(cleanup);

beforeEach(() => {
  vi.restoreAllMocks();
  vi.spyOn(api, 'scanLibrary').mockResolvedValue([
    { appid: 2646460, name: 'Soulmask', installed: true, launcher: 'steam' },
  ]);
  vi.spyOn(api, 'scanExternalLaunchers').mockResolvedValue([
    // El mismo juego desde otro launcher: debe quedar oculto.
    { name: 'Soulmask', launcher: 'lutris', install_path: '', appid: 2646460, cover_key: 'soulmask' },
    { name: 'Star Valor', launcher: 'lutris', install_path: '', appid: null, cover_key: 'star-valor' },
  ]);
});

describe('useLibrary', () => {
  it('oculta los duplicados de Steam y los cuenta', async () => {
    const { result } = renderHook(() => useLibrary({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.loadInitial();
    });

    await waitFor(() => expect(result.current.externalGames).toHaveLength(1));
    expect(result.current.externalGames[0].name).toBe('Star Valor');
    expect(result.current.hiddenDuplicates).toBe(1);
  });

  it('mantiene estable la identidad de loadInitial', async () => {
    // Regresión: cuando `scanExternal` dependía de `library`, `loadInitial`
    // cambiaba en cada escaneo y el efecto de arranque de App.tsx se volvía a
    // ejecutar: la app escaneaba en bucle y parpadeaba.
    const { result, rerender } = renderHook(() => useLibrary({ getHwTags: () => [] }));

    const firstLoad = result.current.loadInitial;
    const firstScan = result.current.rescanExternal;

    await act(async () => {
      await result.current.loadInitial();
    });

    expect(result.current.loadInitial).toBe(firstLoad);
    expect(result.current.rescanExternal).toBe(firstScan);

    rerender();
    expect(result.current.loadInitial).toBe(firstLoad);
  });

  it('un solo escaneo al cargar', async () => {
    const { result } = renderHook(() => useLibrary({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.loadInitial();
    });

    expect(api.scanExternalLaunchers).toHaveBeenCalledTimes(1);
  });

  it('el re-escaneo manual usa la biblioteca de Steam ya cargada', async () => {
    const { result } = renderHook(() => useLibrary({ getHwTags: () => [] }));

    await act(async () => {
      await result.current.loadInitial();
    });
    await act(async () => {
      await result.current.rescanExternal();
    });

    // Sin el ref, el segundo escaneo compararía contra el estado del primer
    // render (vacío) y volvería a mostrar Soulmask duplicado.
    expect(result.current.externalGames.map((g) => g.name)).toEqual(['Star Valor']);
  });

  it('expone cuántos juegos de Heronite se ocultaron por desinstalados', async () => {
    // Heronite deja los juegos desinstalados en su library.json. Rust los
    // filtra; aquí solo se comprueba que el conteo llega a la UI, para poder
    // avisar al usuario en vez de que parezca que faltan juegos.
    vi.spyOn(api, 'heroniteUninstalledCount').mockResolvedValue(3);

    const { result } = renderHook(() => useLibrary({ getHwTags: () => [] }));
    await act(async () => {
      await result.current.loadInitial();
    });

    await waitFor(() => expect(result.current.heroniteUninstalled).toBe(3));
  });

  it('si falla el conteo de desinstalados, vale 0 y no rompe el escaneo', async () => {
    vi.spyOn(api, 'heroniteUninstalledCount').mockRejectedValue(new Error('boom'));

    const { result } = renderHook(() => useLibrary({ getHwTags: () => [] }));
    await act(async () => {
      await result.current.loadInitial();
    });

    await waitFor(() => expect(result.current.externalGames).toHaveLength(1));
    expect(result.current.heroniteUninstalled).toBe(0);
  });

  it('menciona los desinstalados en el aviso del re-escaneo', async () => {
    vi.spyOn(api, 'heroniteUninstalledCount').mockResolvedValue(2);
    const onNotify = vi.fn();

    const { result } = renderHook(() => useLibrary({ getHwTags: () => [], onNotify }));
    await act(async () => {
      await result.current.loadInitial();
    });
    await act(async () => {
      await result.current.rescanExternal();
    });

    expect(onNotify).toHaveBeenCalledWith(expect.stringContaining('2 desinstalados de Heronite'));
  });
});
