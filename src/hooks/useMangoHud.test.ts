import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { api } from '../lib/api';
import { useMangoHud } from './useMangoHud';

afterEach(cleanup);

beforeEach(() => {
  vi.restoreAllMocks();
});

/** El hook lee la configuración al montar: se deja en un estado conocido. */
function mockInitialState(installed: boolean, duration: number) {
  vi.spyOn(api, 'checkMangohudInstalled').mockResolvedValue(installed);
  vi.spyOn(api, 'getMangohudLogDuration').mockResolvedValue(duration);
}

describe('useMangoHud: deshacer cambios', () => {
  it('restaura la configuración y relee la duración del archivo', async () => {
    mockInitialState(true, 60);
    // Tras restaurar, el archivo ya no tiene límite.
    vi.spyOn(api, 'restoreMangohudConf').mockResolvedValue(1);
    const getDuration = vi
      .spyOn(api, 'getMangohudLogDuration')
      .mockResolvedValueOnce(60)
      .mockResolvedValueOnce(0);
    const notify = vi.fn();

    const { result } = renderHook(() => useMangoHud(notify));
    await waitFor(() => expect(result.current.duration).toBe(60));

    let ok = false;
    await act(async () => {
      ok = await result.current.restoreConfig();
    });

    expect(ok).toBe(true);
    expect(api.restoreMangohudConf).toHaveBeenCalledTimes(1);
    // Se relee en vez de asumir: el archivo manda.
    expect(getDuration).toHaveBeenCalledTimes(2);
    expect(result.current.duration).toBe(0);
    expect(notify).toHaveBeenCalledWith('✓ Configuración de MangoHud restaurada');
  });

  it('si no hay copia de seguridad avisa y no miente sobre el estado', async () => {
    mockInitialState(true, 60);
    vi.spyOn(api, 'restoreMangohudConf').mockRejectedValue(
      'No hay ninguna copia de seguridad de MangoHud.conf que restaurar.'
    );
    const notify = vi.fn();

    const { result } = renderHook(() => useMangoHud(notify));
    await waitFor(() => expect(result.current.duration).toBe(60));

    let ok = true;
    await act(async () => {
      ok = await result.current.restoreConfig();
    });

    expect(ok).toBe(false);
    // La duración no cambia si la restauración falló.
    expect(result.current.duration).toBe(60);
    expect(notify).toHaveBeenCalledWith(
      expect.stringContaining('No hay ninguna copia de seguridad')
    );
  });
});

describe('useMangoHud: cambiar la duración', () => {
  it('revierte el valor en pantalla si el backend falla', async () => {
    mockInitialState(true, 60);
    vi.spyOn(api, 'setMangohudLogDuration').mockRejectedValue(
      '«MangoHud.conf» es un enlace simbólico.'
    );
    const notify = vi.fn();

    const { result } = renderHook(() => useMangoHud(notify));
    await waitFor(() => expect(result.current.duration).toBe(60));

    await act(async () => {
      await result.current.changeDuration(120);
    });

    // No debe quedarse mostrando 120 si no se guardó.
    expect(result.current.duration).toBe(60);
  });
});
