import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { FinishedLog } from '../lib/types';

const scanNewMangohudLogs = vi.fn();
const markLogsProcessed = vi.fn();

vi.mock('../lib/api', () => ({
  api: {
    scanNewMangohudLogs: (...args: unknown[]) => scanNewMangohudLogs(...args),
    markLogsProcessed: (...args: unknown[]) => markLogsProcessed(...args),
  },
}));

import { POLL_MS, useLogWatcher } from './useLogWatcher';

function log(path: string, program = 'Juego', timestamp = '2026-11-02_14-43-52'): FinishedLog {
  return { path, program, timestamp };
}

beforeEach(() => {
  scanNewMangohudLogs.mockReset();
  markLogsProcessed.mockReset();
  scanNewMangohudLogs.mockResolvedValue([]);
  markLogsProcessed.mockResolvedValue(undefined);
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useLogWatcher', () => {
  it('analiza una grabación nueva y la marca como procesada', async () => {
    const nueva = log('/home/u/goverlay/Juego_2026-11-02_14-43-52.csv');
    scanNewMangohudLogs.mockResolvedValue([nueva]);
    const process = vi.fn().mockResolvedValue(true);
    const onFound = vi.fn();

    const { result } = renderHook(() =>
      useLogWatcher({ folder: '/home/u/goverlay', process, onFound })
    );

    await waitFor(() => expect(process).toHaveBeenCalledWith(nueva, null));
    await waitFor(() => expect(onFound).toHaveBeenCalledWith(nueva));
    expect(markLogsProcessed).toHaveBeenCalledWith([nueva.path]);
    await waitFor(() => expect(result.current.lastFound).toEqual(nueva));
  });

  it('no marca como procesada una grabación que no se pudo guardar', async () => {
    // Lo importante: si el análisis falla, la grabación se reintenta en la próxima
    // pasada en vez de perderse para siempre.
    const nueva = log('/x/Juego_2026-11-02_14-43-52.csv');
    scanNewMangohudLogs.mockResolvedValue([nueva]);
    const process = vi.fn().mockResolvedValue(false);

    const { result } = renderHook(() =>
      useLogWatcher({ folder: '/x', process })
    );

    await waitFor(() => expect(process).toHaveBeenCalled());
    expect(markLogsProcessed).not.toHaveBeenCalled();
    expect(result.current.lastFound).toBeNull();
  });

  it('sigue con las demás aunque una falle', async () => {
    const a = log('/x/a.csv', 'A');
    const b = log('/x/b.csv', 'B');
    scanNewMangohudLogs.mockResolvedValue([a, b]);
    const process = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);

    const { result } = renderHook(() => useLogWatcher({ folder: '/x', process }));

    await waitFor(() => expect(process).toHaveBeenCalledTimes(2));
    expect(markLogsProcessed).toHaveBeenCalledTimes(1);
    expect(markLogsProcessed).toHaveBeenCalledWith([b.path]);
    expect(result.current.lastFound).toEqual(b);
  });

  it('sin carpeta no consulta nada', async () => {
    const process = vi.fn();
    renderHook(() => useLogWatcher({ folder: null, process }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_MS * 3);
    });
    expect(scanNewMangohudLogs).not.toHaveBeenCalled();
    expect(process).not.toHaveBeenCalled();
  });

  it('desactivado no consulta nada aunque haya carpeta', async () => {
    const process = vi.fn();
    renderHook(() => useLogWatcher({ folder: '/x', process, enabled: false }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_MS * 3);
    });
    expect(scanNewMangohudLogs).not.toHaveBeenCalled();
  });

  it('vuelve a mirar cada pocos segundos', async () => {
    renderHook(() => useLogWatcher({ folder: '/x', process: async () => true }));
    await waitFor(() => expect(scanNewMangohudLogs).toHaveBeenCalledTimes(1));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_MS);
    });
    await waitFor(() => expect(scanNewMangohudLogs).toHaveBeenCalledTimes(2));
  });

  it('un error al leer la carpeta se cuenta, no rompe la pantalla', async () => {
    scanNewMangohudLogs.mockRejectedValue('no existe');
    const onError = vi.fn();
    renderHook(() => useLogWatcher({ folder: '/nada', process: async () => true, onError }));
    await waitFor(() => expect(onError).toHaveBeenCalled());
    expect(String(onError.mock.calls[0][0])).toContain('No se pudo revisar');
  });

  it('no se solapan dos pasadas si una tarda más que el intervalo', async () => {
    let resolve: (v: FinishedLog[]) => void = () => {};
    scanNewMangohudLogs.mockImplementation(
      () => new Promise<FinishedLog[]>((r) => { resolve = r; })
    );
    renderHook(() => useLogWatcher({ folder: '/x', process: async () => true }));

    await waitFor(() => expect(scanNewMangohudLogs).toHaveBeenCalledTimes(1));
    // Pasan tres intervalos mientras la primera sigue en marcha.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_MS * 3);
    });
    expect(scanNewMangohudLogs).toHaveBeenCalledTimes(1);
    await act(async () => {
      resolve([]);
    });
  });

  it('pasa la configuración del momento a cada grabación', async () => {
    const nueva = log('/x/a.csv');
    scanNewMangohudLogs.mockResolvedValue([nueva]);
    const setup = { command: 'mangohud %command%', at: '2026-11-02T14:43:52Z' };
    const captureSetup = vi.fn().mockReturnValue(setup);
    const process = vi.fn().mockResolvedValue(true);

    renderHook(() => useLogWatcher({ folder: '/x', process, captureSetup }));

    await waitFor(() => expect(process).toHaveBeenCalledWith(nueva, setup));
  });

  it('una grabación que no se pudo guardar no se reintenta en bucle', async () => {
    // Este era el fallo: el aviso salía cada 5 s para siempre y la pantalla se movía.
    const nueva = log('/x/a.csv');
    scanNewMangohudLogs.mockResolvedValue([nueva]);
    const process = vi.fn().mockResolvedValue(false);
    const onError = vi.fn();

    renderHook(() => useLogWatcher({ folder: '/x', process, onError }));

    await waitFor(() => expect(process).toHaveBeenCalledTimes(1));

    // Pasan tres sondeos más: no se vuelve a intentar ni se repite el aviso.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_MS * 3);
    });
    expect(process).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledTimes(1);
  });

  it('avisa una sola vez aunque fallen varias grabaciones a la vez', async () => {
    scanNewMangohudLogs.mockResolvedValue([log('/x/a.csv', 'A'), log('/x/b.csv', 'B')]);
    const onError = vi.fn();
    renderHook(() =>
      useLogWatcher({ folder: '/x', process: async () => false, onError })
    );

    await waitFor(() => expect(onError).toHaveBeenCalled());
    expect(onError).toHaveBeenCalledTimes(1);
    expect(String(onError.mock.calls[0][0])).toContain('2 grabaciones');
  });

  it('checkNow fuerza una revisión inmediata', async () => {
    const { result } = renderHook(() =>
      useLogWatcher({ folder: '/x', process: async () => true })
    );
    await waitFor(() => expect(scanNewMangohudLogs).toHaveBeenCalledTimes(1));
    await act(async () => {
      await result.current.checkNow();
    });
    expect(scanNewMangohudLogs).toHaveBeenCalledTimes(2);
  });
});
