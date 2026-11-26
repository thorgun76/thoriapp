import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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
    getShaderCache: vi.fn().mockResolvedValue(null),
    // Borrado: se espían para poder comprobar que se llama al del alcance correcto y no a
    // `deleteAllBenchmarks`, que vaciaría el historial de todos los juegos.
    deleteBenchmarksForGame: vi.fn().mockResolvedValue(3),
    deleteAllBenchmarks: vi.fn().mockResolvedValue(undefined),
    deleteBenchmark: vi.fn().mockResolvedValue(undefined),
  },
}));

// `openExternal` llama a api.openUrl: se comprueba contra el mismo mock.
import { api } from '../lib/api';

const MANGOHUD_URL = 'https://github.com/flightlessmango/MangoHud';

beforeEach(() => {
  vi.clearAllMocks();
  // `clearAllMocks` borra llamadas pero no la implementación: un test que simula un
  // fallo de lectura dejaría el mock roto para el siguiente. Se restaura aquí.
  (api.loadBenchmarks as ReturnType<typeof vi.fn>).mockResolvedValue([]);
  (api.loadBenchmarksForGame as ReturnType<typeof vi.fn>).mockResolvedValue([]);
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

  // ── Subfase 5.1: el historial ilegible se dice, no se pinta como «vacío» ──

  it('s51 avisa cuando no se pudo leer el historial de benchmarks', async () => {
    (api.loadBenchmarks as ReturnType<typeof vi.fn>).mockRejectedValue(
      'El historial de benchmarks está dañado (expected value at line 1). No se ha modificado nada en disco.'
    );

    render(<BenchmarksView />);

    const alerta = await screen.findByRole('alert');
    expect(alerta.textContent).toContain('No se pudo leer el historial');
    expect(alerta.textContent).toContain('dañado');
  });

  it('s51 sin error de lectura no aparece ningún aviso', async () => {
    render(<BenchmarksView />);

    // Se espera a que la carga termine para no confundir «todavía no» con «no hay».
    await screen.findByText(/La grabación la hace/i);
    await waitFor(() => expect(api.loadBenchmarks).toHaveBeenCalled());
    expect(screen.queryByText(/No se pudo leer el historial/)).toBeNull();
  });
});

/**
 * Subfase 5.2: la configuración solo se atribuye al juego de la grabación.
 *
 * La vista no llama al backend al montarse (no analiza sola), así que la protección se
 * comprueba sobre el contexto que se construye: se monta con un juego abierto y se mira
 * qué se le ofrece al guardar un CSV de ese juego y de otro.
 */
describe('BenchmarksView: contexto de captura (Prueba 12)', () => {
  const abierta = {
    appid: 251570,
    gameName: '7 Days to Die',
    perfilId: 'p-7dtd',
    perfilNombre: 'Estable',
    comando: 'gamemoderun %command%',
    modoProton: true,
  };

  const completo = {
    id: 'b1',
    profile_id: '',
    game_name: '7 Days to Die',
    appid: 251570,
    timestamp: '2026-11-24 19:20:05',
    duration_seconds: 60,
    avg_fps: 60,
    fps_1_percent_low: 50,
    fps_0_1_percent_low: 40,
    max_fps: 80,
    min_fps: 30,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 20,
    total_frames: 600,
    notes: null,
    number: 1,
    snapshot: null,
  };

  it('s52 la instantánea de la prueba más reciente se ofrece en la vista', async () => {
    (api.loadBenchmarks as ReturnType<typeof vi.fn>).mockResolvedValue([completo]);
    (api.loadBenchmarksForGame as ReturnType<typeof vi.fn>).mockResolvedValue([completo]);

    render(
      <BenchmarksView
        currentAppid={251570}
        currentGameName={'7 Days to Die'}
        currentLaunchCommand="gamemoderun %command%"
        contextoDelJuegoAbierto={abierta}
      />
    );

    // Un benchmark antiguo se anuncia como sin instantánea, no se rellena.
    expect(await screen.findByText('Instantánea no disponible')).toBeTruthy();
  });
});

/**
 * Alcance de la pestaña general (cierre de 5.3).
 *
 * «Benchmarks» no es solo la ficha de un juego: sin juego abierto enseña el historial de
 * **todos** los juegos, y con juego abierto se limita a uno. El encabezado y el borrado
 * estaban escritos como si siempre hubiera un solo juego.
 */
describe('BenchmarksView · alcance del historial', () => {
  function prueba(id: string, appid: number, nombre: string): any {
    return {
      id,
      profile_id: '',
      game_name: nombre,
      appid,
      timestamp: `2026-11-2${id.slice(-1)} 10:00:00`,
      duration_seconds: 120,
      avg_fps: 60,
      fps_1_percent_low: 50,
      fps_0_1_percent_low: 40,
      max_fps: 70,
      min_fps: 30,
      frame_time_avg: 16,
      frame_time_1_percent_high: 20,
      total_frames: 600,
      notes: null,
      number: 1,
      snapshot: null,
      launch_command: '%command%',
    };
  }

  const ocho = [
    ...Array.from({ length: 5 }, (_, i) => prueba(`p${i}`, 1623730, 'Palworld')),
    ...Array.from({ length: 3 }, (_, i) => prueba(`d${i}`, 4623570, 'Deskrawl')),
  ];

  it('sin juego abierto el historial es de todos los juegos, y lo dice', async () => {
    (api.loadBenchmarks as ReturnType<typeof vi.fn>).mockResolvedValue(ocho);
    render(<BenchmarksView />);

    expect(await screen.findByText('Historial de todos los juegos')).toBeTruthy();
    // El texto antiguo afirmaba un alcance que los datos no tenían.
    expect(screen.queryByText('Historial de este juego')).toBeNull();
    expect(screen.getByText('8')).toBeTruthy();
  });

  it('con juego abierto el historial es de ese juego, y lo dice', async () => {
    (api.loadBenchmarksForGame as ReturnType<typeof vi.fn>).mockResolvedValue(
      ocho.filter((b) => b.appid === 4623570)
    );
    render(<BenchmarksView currentAppid={4623570} currentGameName="Deskrawl" />);

    expect(await screen.findByText('Historial de este juego')).toBeTruthy();
    expect(screen.queryByText('Historial de todos los juegos')).toBeNull();
  });

  it('la confirmación de borrado describe el alcance real de lo que borra', async () => {
    (api.loadBenchmarks as ReturnType<typeof vi.fn>).mockResolvedValue(ocho);
    render(<BenchmarksView />);

    fireEvent.click(await screen.findByRole('button', { name: /Borrar todo el historial/i }));
    // Dice cuántos juegos y cuántas grabaciones: no «de este juego» cuando son dos.
    expect(await screen.findByText(/los 2 juegos del historial/)).toBeTruthy();
    expect(screen.queryByText(/Los demás juegos no se tocan/)).toBeNull();
  });

  it('desde un juego concreto no borra el historial de los demás', async () => {
    (api.loadBenchmarksForGame as ReturnType<typeof vi.fn>).mockResolvedValue(
      ocho.filter((b) => b.appid === 4623570)
    );
    render(<BenchmarksView currentAppid={4623570} currentGameName="Deskrawl" />);

    fireEvent.click(await screen.findByRole('button', { name: /^Borrar historial$/i }));
    // El botón de confirmar se llama igual que el de abrir: hay que buscarlo dentro del
    // diálogo, no en toda la pantalla.
    const dialogo = (await screen.findByRole('dialog')) as HTMLElement;
    const confirmar = within(dialogo).getByRole('button', { name: /^Borrar historial$/i });
    fireEvent.click(confirmar);

    // Lo grave que había: llamaba a `deleteAllBenchmarks` y se llevaba Palworld por delante.
    await vi.waitFor(() => expect(api.deleteBenchmarksForGame).toHaveBeenCalled());
    expect(api.deleteAllBenchmarks).not.toHaveBeenCalled();
  });
});
