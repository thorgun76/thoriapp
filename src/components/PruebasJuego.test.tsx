import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { Benchmark, GroupComparison } from '../lib/types';

const loadBenchmarksForGame = vi.fn();
const deleteBenchmark = vi.fn();
const deleteBenchmarksForGame = vi.fn();
// Borrado global: se espía para comprobar que desde esta ventana **no** se usa nunca.
const deleteAllBenchmarks = vi.fn();
const compareBenchmarkGroups = vi.fn();

vi.mock('../lib/api', () => ({
  api: {
    loadBenchmarksForGame: (...a: unknown[]) => loadBenchmarksForGame(...a),
    deleteBenchmark: (...a: unknown[]) => deleteBenchmark(...a),
    deleteBenchmarksForGame: (...a: unknown[]) => deleteBenchmarksForGame(...a),
    deleteAllBenchmarks: (...a: unknown[]) => deleteAllBenchmarks(...a),
    compareBenchmarkGroups: (...a: unknown[]) => compareBenchmarkGroups(...a),
  },
}));

import { PruebasJuego } from './PruebasJuego';

function bench(
  id: string,
  n: number,
  { low1 = 50, fps = 60, low01 = 40, ts = '2026-11-01T10:00:00' } = {}
): Benchmark {
  return {
    id,
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: ts,
    duration_seconds: 60,
    avg_fps: fps,
    fps_1_percent_low: low1,
    fps_0_1_percent_low: low01,
    max_fps: 120,
    min_fps: 10,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 33,
    total_frames: 3600,
    notes: null,
    launch_command: '',
    number: n,
  };
}

const grupo = (): GroupComparison[] => [
  {
    metric: 'avg_fps',
    antes_runs: 2,
    despues_runs: 2,
    antes_median: 55,
    despues_median: 60,
    change_percent: 9,
    ci_low_percent: 2,
    ci_high_percent: 16,
    required_margin_percent: 3,
    verdict: 'mejora',
    conclusion: 'mejora',
  },
];

const renderVista = (props: Partial<Parameters<typeof PruebasJuego>[0]> = {}) => {
  const onClose = vi.fn();
  const onMedir = vi.fn();
  render(
    <PruebasJuego
      appid={1623730}
      gameName="Palworld"
      onClose={onClose}
      onMedir={onMedir}
      {...props}
    />
  );
  return { onClose, onMedir };
};

beforeEach(() => {
  loadBenchmarksForGame.mockReset().mockResolvedValue([]);
  deleteBenchmark.mockReset().mockResolvedValue(undefined);
  deleteBenchmarksForGame.mockReset().mockResolvedValue(0);
  compareBenchmarkGroups.mockReset().mockResolvedValue(grupo());
});

afterEach(cleanup);

describe('PruebasJuego (historial del juego abierto)', () => {
  it('pide al backend solo las grabaciones de este juego, con appid y nombre', async () => {
    renderVista();
    // El nombre no es decorativo: sin él, las grabaciones antiguas (appid 0, porque
    // MangoHud no escribe el AppID) no se encontraban y la ventana salía vacía.
    await waitFor(() =>
      expect(loadBenchmarksForGame).toHaveBeenCalledWith({
        appid: 1623730,
        gameName: 'Palworld',
      })
    );
  });

  it('encuentra la prueba guardada antes, cuando aún no había appid', async () => {
    // Reproduce el fallo reportado: una grabación antigua de Palworld con appid 0.
    loadBenchmarksForGame.mockResolvedValue([{ ...bench('vieja', 1), appid: 0 }]);
    renderVista();

    // Singular correcto: «de tus 1 grabaciones» estaba mal escrito.
    expect(await screen.findByText('Tu única grabación de este juego')).toBeTruthy();
    expect(screen.queryByText(/1 grabaciones/)).toBeNull();
    // Y no se queda en el aviso de «todavía no hay pruebas».
    expect(screen.queryByText(/Todavía no hay pruebas/)).toBeNull();
  });

  it('titula con el nombre del juego, no de forma genérica', () => {
    renderVista();
    expect(screen.getByRole('heading', { name: 'Palworld' })).toBeTruthy();
  });

  it('avisa de cómo grabar cuando no hay pruebas, sin inventar datos', async () => {
    renderVista();
    expect(await screen.findByText(/Todavía no hay pruebas de Palworld/)).toBeTruthy();
    // Sin grabaciones no se pinta el ranking.
    expect(screen.queryByText(/Cuál va mejor/)).toBeNull();
    expect(screen.getByText(/Medir la primera/)).toBeTruthy();
  });

  it('enseña el ranking del juego cuando ya hay grabaciones', async () => {
    loadBenchmarksForGame.mockResolvedValue([bench('a', 1), bench('b', 2, { low1: 70 })]);
    renderVista();
    expect(await screen.findByText(/Cuál va mejor de tus 2 grabaciones/)).toBeTruthy();
    expect(screen.getByText(/Prueba 2/)).toBeTruthy();
  });

  it('ordena por 1 % low y no por FPS medios', async () => {
    loadBenchmarksForGame.mockResolvedValue([
      bench('muchos-fps', 1, { low1: 30, fps: 120 }),
      bench('pareja', 2, { low1: 55, fps: 70 }),
    ]);
    renderVista();

    await screen.findByText(/Cuál va mejor/);
    // El nombre de la prueba va dentro de la misma línea que la marca «Mejor · ».
    const filas = screen.getAllByText(/Prueba \d$/).map((n) => n.textContent);
    expect(filas).toEqual(['Mejor · Prueba 2', 'Prueba 1']);
  });

  it('borra una grabación individual pidiendo confirmación', async () => {
    loadBenchmarksForGame.mockResolvedValue([bench('a', 1), bench('b', 2, { low1: 70 })]);
    renderVista();

    fireEvent.click(await screen.findByLabelText('Borrar Prueba 1'));
    // Aún no se ha borrado: primero se confirma.
    expect(deleteBenchmark).not.toHaveBeenCalled();

    const dialogo = screen.getByRole('heading', { name: /¿Borrar Prueba 1\?/ }).closest('div')!
      .parentElement!;
    fireEvent.click(within(dialogo).getByRole('button', { name: 'Borrar' }));

    await waitFor(() => expect(deleteBenchmark).toHaveBeenCalledWith('a'));
  });

  it('borra solo el historial de este juego, no el de todos', async () => {
    loadBenchmarksForGame.mockResolvedValue([bench('a', 1), bench('b', 2, { low1: 70 })]);
    deleteBenchmarksForGame.mockResolvedValue(2);
    renderVista();

    fireEvent.click(await screen.findByText(/Borrar todas/));

    const titulo = screen.getByRole('heading', { name: /borrar todas las pruebas/i });
    const cuadro = titulo.closest('div')!.parentElement!;
    fireEvent.click(within(cuadro).getByRole('button', { name: 'Borrar todas' }));

    await waitFor(() =>
      expect(deleteBenchmarksForGame).toHaveBeenCalledWith({
        appid: 1623730,
        gameName: 'Palworld',
      })
    );
  });

  it('no ofrece «Borrar todas» con una sola grabación', async () => {
    loadBenchmarksForGame.mockResolvedValue([bench('a', 1)]);
    renderVista();
    // Con una grabación el título va en singular («Tu única grabación»).
    await screen.findByText('Tu única grabación de este juego');
    expect(screen.queryByText(/Borrar todas/)).toBeNull();
  });

  it('explica el límite de 10 por juego junto al borrado', async () => {
    loadBenchmarksForGame.mockResolvedValue([bench('a', 1), bench('b', 2, { low1: 70 })]);
    renderVista();
    expect(await screen.findByText(/como máximo 10 pruebas por juego/)).toBeTruthy();
  });

  it('compara los grupos solo con dos grabaciones o más', async () => {
    loadBenchmarksForGame.mockResolvedValue([bench('a', 1)]);
    renderVista();
    await screen.findByText('Tu única grabación de este juego');
    expect(compareBenchmarkGroups).not.toHaveBeenCalled();
  });

  it('muestra si el cambio es real cuando hay varias grabaciones', async () => {
    loadBenchmarksForGame.mockResolvedValue([
      bench('a', 1),
      bench('b', 2, { low1: 70 }),
      bench('c', 3),
      bench('d', 4, { low1: 65 }),
    ]);
    renderVista();
    expect(await screen.findByText(/El cambio es real o es ruido/i)).toBeTruthy();
  });

  it('«medir» lleva a la pestaña de benchmarks en vez de duplicar el formulario', async () => {
    const { onMedir } = renderVista();
    fireEvent.click(await screen.findByText(/Medir la primera/));
    expect(onMedir).toHaveBeenCalledTimes(1);
  });

  it('copia el comando y lo confirma con un aviso', async () => {
    // El portapapeles no da señal visible: sin toast el usuario no sabe si se copió.
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    loadBenchmarksForGame.mockResolvedValue([
      { ...bench('a', 1), launch_command: 'mangohud %command%' },
    ]);
    renderVista();

    fireEvent.click(await screen.findByLabelText('Copiar el comando de Prueba 1'));

    await waitFor(() => expect(writeText).toHaveBeenCalledWith('mangohud %command%'));
    expect(await screen.findByText('✓ Comando copiado')).toBeTruthy();
  });

  it('avisa si el portapapeles falla, sin romper la ventana', async () => {
    const writeText = vi.fn().mockRejectedValue(new Error('denegado'));
    Object.assign(navigator, { clipboard: { writeText } });
    loadBenchmarksForGame.mockResolvedValue([
      { ...bench('a', 1), launch_command: 'mangohud %command%' },
    ]);
    renderVista();

    fireEvent.click(await screen.findByLabelText('Copiar el comando de Prueba 1'));

    expect(await screen.findByText('❌ No se pudo copiar')).toBeTruthy();
  });

  it('Escape cierra la ventana', () => {
    const { onClose } = renderVista();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('el cierre no borra nada', async () => {
    const { onClose } = renderVista();
    fireEvent.click(screen.getByLabelText('Cerrar'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(deleteBenchmark).not.toHaveBeenCalled();
    expect(deleteBenchmarksForGame).not.toHaveBeenCalled();
  });
});

/**
 * Nombre comercial con subtítulo (defecto real de la Prueba 4).
 *
 * La Biblioteca llama al juego «Deskrawl: Idle ARPG»; MangoHud escribe «Deskrawl» y deja
 * `appid: 0`. La ventana decía «Todavía no hay pruebas» teniendo cuatro. La identidad la
 * decide Rust (`coinciden_por_identidad`); aquí se comprueba que la ventana **no** aplica
 * un segundo criterio por su cuenta: pasa appid y nombre tal cual y enseña lo que llega.
 */
describe('PruebasJuego · nombre comercial con subtítulo', () => {
  it('pasa el nombre con subtítulo al backend sin recortarlo ni normalizarlo en React', async () => {
    loadBenchmarksForGame.mockResolvedValue([]);
    renderVista({ appid: 0, gameName: 'Deskrawl: Idle ARPG' });

    await screen.findByText(/Todavía no hay pruebas de Deskrawl: Idle ARPG/);
    // Una sola política de identidad: la de Rust. React no toca el nombre.
    expect(loadBenchmarksForGame).toHaveBeenCalledWith({
      appid: 0,
      gameName: 'Deskrawl: Idle ARPG',
    });
  });

  it('enseña las cuatro grabaciones que devuelve el backend para ese juego', async () => {
    loadBenchmarksForGame.mockResolvedValue([
      bench('d1', 1, { fps: 60 }),
      bench('d2', 2, { fps: 70 }),
      bench('d3', 3, { fps: 55 }),
      bench('d4', 4, { fps: 65 }),
    ]);
    renderVista({ appid: 0, gameName: 'Deskrawl: Idle ARPG' });

    expect(await screen.findByText(/Cuál va mejor/)).toBeTruthy();
    expect(screen.queryByText(/Todavía no hay pruebas/)).toBeNull();
  });

  it('el borrado desde esta ventana usa el alcance del juego, con su nombre exacto', async () => {
    loadBenchmarksForGame.mockResolvedValue([
      bench('d1', 1),
      bench('d2', 2),
      bench('d3', 3),
      bench('d4', 4),
    ]);
    renderVista({ appid: 0, gameName: 'Deskrawl: Idle ARPG' });

    fireEvent.click(await screen.findByRole('button', { name: /Borrar todas/i }));
    // La ventana de pruebas **ya** es un `role="dialog"`, así que hay dos. Se busca el de
    // confirmación por su título, no por posición.
    await waitFor(() => expect(screen.getAllByRole('dialog').length).toBeGreaterThan(1));
    const confirmacion = screen
      .getAllByRole('dialog')
      .find((d) => /Se eliminarán/i.test(d.textContent ?? '')) as HTMLElement;
    expect(confirmacion).toBeTruthy();
    // El texto de la confirmación nombra el juego tal como se llama en la ficha.
    expect(confirmacion.textContent).toContain('Deskrawl: Idle ARPG');
    fireEvent.click(within(confirmacion).getByRole('button', { name: 'Borrar todas' }));

    await waitFor(() =>
      expect(deleteBenchmarksForGame).toHaveBeenCalledWith({
        appid: 0,
        gameName: 'Deskrawl: Idle ARPG',
      })
    );
    // Y nunca el borrado global.
    expect(deleteAllBenchmarks).not.toHaveBeenCalled();
  });
});
