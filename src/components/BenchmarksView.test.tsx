import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { BenchmarksView } from './BenchmarksView';

// El backend no existe en los tests: se simula lo que devuelven los comandos.
vi.mock('../lib/api', () => ({
  api: {
    checkMangohudInstalled: vi.fn().mockResolvedValue(true),
    getMangohudLogDuration: vi.fn().mockResolvedValue(60),
    loadBenchmarks: vi.fn().mockResolvedValue([]),
    loadBenchmarksForGame: vi.fn().mockResolvedValue([]),
    loadComparisons: vi.fn().mockResolvedValue([]),
    detectMangohudLogPath: vi.fn().mockResolvedValue('/tmp/MangoHud'),
    openUrl: vi.fn().mockResolvedValue(undefined),
  },
}));

// `openExternal` llama a api.openUrl: se comprueba contra el mismo mock.
import { api } from '../lib/api';

const MANGOHUD_URL = 'https://github.com/flightlessmango/MangoHud';

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
});

afterEach(cleanup);

describe('BenchmarksView: requisito de MangoHud', () => {
  it('dice que la grabación la hace MangoHud y enlaza a su instalación', async () => {
    // Sin este aviso, la pantalla pedía "pulsa Shift + F2" a gente que no tiene
    // MangoHud instalado, y el atajo simplemente no hacía nada.
    render(<BenchmarksView />);

    expect(await screen.findByText(/La grabación la hace/i)).toBeTruthy();
    expect(screen.getByRole('button', { name: /Cómo instalar MangoHud/i })).toBeTruthy();
  });

  it('abre la documentación de MangoHud al pulsar el enlace', async () => {
    render(<BenchmarksView />);

    fireEvent.click(await screen.findByRole('button', { name: /Cómo instalar MangoHud/i }));

    await waitFor(() => {
      expect(api.openUrl).toHaveBeenCalledWith(MANGOHUD_URL);
    });
  });

  it('enseña los comandos de instalación cuando MangoHud no está presente', async () => {
    vi.mocked(api.checkMangohudInstalled).mockResolvedValue(false);

    render(<BenchmarksView />);

    expect(await screen.findByText('MangoHud no está instalado')).toBeTruthy();
    expect(screen.getByText('sudo apt install mangohud')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Guía paso a paso/i })).toBeTruthy();
  });

  it('no enseña el aviso de instalación cuando MangoHud sí está', async () => {
    // Se fija explícitamente: el test anterior cambió la respuesta del mock.
    vi.mocked(api.checkMangohudInstalled).mockResolvedValue(true);

    render(<BenchmarksView />);

    // Se espera a que la detección termine antes de asegurar que no aparece.
    await waitFor(() => expect(api.checkMangohudInstalled).toHaveBeenCalled());
    expect(screen.queryByText('MangoHud no está instalado')).toBeNull();
  });
});
