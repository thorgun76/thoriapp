import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { api } from '../lib/api';
import { clearCoverCache, useLocalCover } from './useLocalCover';

afterEach(cleanup);

beforeEach(() => {
  clearCoverCache();
  vi.restoreAllMocks();
});

const lutris = { appid: 0, name: 'Juego', launcher: 'lutris', cover_key: 'slug' };

describe('useLocalCover', () => {
  it('devuelve la carátula del launcher como data URI', async () => {
    vi.spyOn(api, 'getLocalCover').mockResolvedValue('data:image/jpeg;base64,AAAA');

    const { result } = renderHook(() => useLocalCover(lutris));

    await waitFor(() => expect(result.current).toBe('data:image/jpeg;base64,AAAA'));
  });

  it('mientras consulta devuelve undefined, no null', () => {
    // La tarjeta distingue ambos: con undefined espera (no pinta el CDN de
    // Steam y evita un parpadeo), con null cae al icono de mando.
    vi.spyOn(api, 'getLocalCover').mockReturnValue(new Promise(() => {}));

    const { result } = renderHook(() => useLocalCover(lutris));
    expect(result.current).toBeUndefined();
  });

  it('null cuando el launcher no tiene carátula en disco', async () => {
    vi.spyOn(api, 'getLocalCover').mockResolvedValue(null);

    const { result } = renderHook(() => useLocalCover(lutris));
    await waitFor(() => expect(result.current).toBeNull());
  });

  it('un error del comando se trata como "sin carátula"', async () => {
    vi.spyOn(api, 'getLocalCover').mockRejectedValue(new Error('EACCES'));

    const { result } = renderHook(() => useLocalCover(lutris));
    await waitFor(() => expect(result.current).toBeNull());
  });

  it('no repite la consulta para el mismo juego', async () => {
    vi.spyOn(api, 'getLocalCover').mockResolvedValue(null);

    const { result, rerender } = renderHook(() => useLocalCover({ ...lutris }));
    await waitFor(() => expect(result.current).toBeNull());
    rerender();
    rerender();

    expect(api.getLocalCover).toHaveBeenCalledTimes(1);
  });

  it('al cambiar de juego devuelve undefined en lugar de la carátula anterior', async () => {
    vi.spyOn(api, 'getLocalCover').mockImplementation((_l, _a, key) =>
      Promise.resolve(key === 'uno' ? 'data:image/jpeg;base64,UNO' : 'data:image/jpeg;base64,DOS')
    );

    const { result, rerender } = renderHook(({ game }) => useLocalCover(game), {
      initialProps: { game: { ...lutris, cover_key: 'uno' } },
    });
    await waitFor(() => expect(result.current).toBe('data:image/jpeg;base64,UNO'));

    rerender({ game: { ...lutris, cover_key: 'dos' } });
    // No debe verse la carátula de "uno" en la tarjeta de "dos".
    expect(result.current).toBeUndefined();
    await waitFor(() => expect(result.current).toBe('data:image/jpeg;base64,DOS'));
  });

  it('sin juego no consulta nada', async () => {
    const spy = vi.spyOn(api, 'getLocalCover').mockResolvedValue(null);

    const { result } = renderHook(() => useLocalCover(null));
    expect(result.current).toBeNull();
    await act(async () => {});
    expect(spy).not.toHaveBeenCalled();
  });

  it('clearCoverCache fuerza a consultar de nuevo', async () => {
    const spy = vi.spyOn(api, 'getLocalCover').mockResolvedValue(null);

    const { result, rerender } = renderHook(() => useLocalCover(lutris));
    await waitFor(() => expect(result.current).toBeNull());

    clearCoverCache();
    rerender();
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
  });

  it('envía el runner del juego (Heronite indexa el arte por runner:appName)', async () => {
    // Regresión: sin el runner, la clave `sideload:<appName>` de art.json no se
    // encontraba y los juegos de Heronite salían sin carátula.
    const spy = vi.spyOn(api, 'getLocalCover').mockResolvedValue(null);

    const { result } = renderHook(() =>
      useLocalCover({
        appid: 0,
        name: 'Soulmask',
        launcher: 'heronite',
        cover_key: 'steam_2646460|Soulmask',
        runner: 'sideload',
      })
    );
    await waitFor(() => expect(result.current).toBeNull());

    expect(spy).toHaveBeenCalledWith('heronite', 0, 'steam_2646460|Soulmask', 'sideload');
  });

  it('un juego de Steam no envía runner', async () => {
    const spy = vi.spyOn(api, 'getLocalCover').mockResolvedValue(null);

    const { result } = renderHook(() => useLocalCover({ appid: 1623730, name: 'Palworld' }));
    await waitFor(() => expect(result.current).toBeNull());

    expect(spy).toHaveBeenCalledWith('steam', 1623730, undefined, undefined);
  });
});
