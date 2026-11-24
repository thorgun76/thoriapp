import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import type { SavedComparison } from '../hooks/useSavedComparisons';

/**
 * Pruebas de «Mis pruebas» (lista de comparaciones guardadas).
 *
 * Se sustituye el hook por datos fijos: aquí lo que se comprueba es la JERARQUÍA de la
 * pantalla (qué se ve y cuántas veces), no la carga desde el backend.
 */

const guardadas: SavedComparison[] = [];

vi.mock('../hooks/useSavedComparisons', () => ({
  useSavedComparisons: () => ({
    items: guardadas,
    loading: false,
    remove: vi.fn(),
    removeAll: vi.fn(),
  }),
}));

const { DashboardView } = await import('./DashboardView');

/**
 * Fila de la lista (el `<div role="button">` de una prueba).
 *
 * Se busca por `role` y no por texto porque el nombre del juego también aparece en el
 * `<button>` de borrar de su propia fila.
 */
function filaDe(indice = 0) {
  return screen.getAllByRole('button', { name: /FPS\./ })[indice]!;
}

afterEach(cleanup);
beforeEach(() => {
  guardadas.length = 0;
});

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-24 19:20:05',
    duration_seconds: 60,
    avg_fps: 68.2,
    fps_1_percent_low: 55,
    fps_0_1_percent_low: 41,
    max_fps: 90,
    min_fps: 20,
    frame_time_avg: 13.3,
    frame_time_1_percent_high: 25,
    total_frames: 600,
    notes: null,
    launch_command: 'mangohud %command%',
    spikes_per_min: 2,
    pct_over_33ms: 1,
    jitter_ms: 1.5,
    worst_frame_ms: 25,
    warmup_seconds: 10,
    noise_percent: 4.2,
    log_path: '',
    ...over,
  };
}

function guardada(
  id: string,
  antes: Partial<Benchmark>,
  despues: Partial<Benchmark>,
  fpsChange: number
): SavedComparison {
  return {
    id,
    game_name: despues.game_name ?? antes.game_name ?? 'Palworld',
    before_fps: antes.avg_fps ?? 68.2,
    after_fps: despues.avg_fps ?? 84.6,
    fps_change_percent: fpsChange,
    saved_at: '2026-11-24 19:20:05',
    comparison: {
      baseline: bench(antes),
      test: bench(despues),
      fps_change_percent: fpsChange,
      fps_1_percent_change_percent: 0,
      fps_0_1_percent_change_percent: 0,
      frame_time_change_percent: 0,
      conclusion: '',
      noise_percent: 4.2,
      within_noise: false,
    } as BenchmarkComparison,
  } as SavedComparison;
}

describe('Mis pruebas: resumen de cabecera', () => {
  it('no hay tarjetas de estadísticas sueltas', () => {
    guardadas.push(
      guardada('1', { avg_fps: 68.2 }, { avg_fps: 84.6, id: 'b2', timestamp: '2026-11-24 19:20:05' }, 24)
    );
    const { container } = render(<DashboardView />);
    // Las tres tarjetas antiguas tenían estos rótulos; ahora todo va en una línea.
    expect(container.textContent).not.toMatch(/Total de pruebas|Juegos probados|Última prueba/i);
  });

  it('resume en una línea cuántas pruebas, cuántos juegos y cuándo fue la última', () => {
    guardadas.push(
      guardada('1', { avg_fps: 68.2 }, { avg_fps: 84.6, id: 'b2' }, 24),
      guardada('2', { avg_fps: 75, game_name: 'Elden Ring' }, { avg_fps: 68.5, id: 'b3', game_name: 'Elden Ring' }, -8.7)
    );
    render(<DashboardView />);
    const texto = document.body.textContent ?? '';
    expect(texto).toMatch(/2 comparaciones guardadas/);
    expect(texto).toMatch(/2 juegos/);
    expect(texto).toMatch(/última/i);
  });

  it('en singular conjuga bien', () => {
    guardadas.push(guardada('1', {}, { id: 'b2' }, 24));
    render(<DashboardView />);
    const texto = document.body.textContent ?? '';
    expect(texto).toMatch(/1 comparación guardada/);
    expect(texto).toMatch(/1 juego/);
  });
});

describe('Mis pruebas: filas de la lista', () => {
  it('hay un solo contenedor con borde, no una tarjeta por prueba', () => {
    guardadas.push(
      guardada('1', { avg_fps: 68.2 }, { avg_fps: 84.6, id: 'b2' }, 24),
      guardada('2', { avg_fps: 75 }, { avg_fps: 68.5, id: 'b3' }, -8.7)
    );
    const { container } = render(<DashboardView />);
    expect(container.querySelectorAll('ul > li')).toHaveLength(2);
    // Una única lista con separadores, en vez de N cajas con su borde.
    expect(container.querySelectorAll('ul.divide-y')).toHaveLength(1);
  });

  it('el veredicto lo pone el intérprete, no la resta de FPS', () => {
    // Mejora general: los tres valores suben, así que el intérprete la llama mejora.
    guardadas.push(
      guardada(
        '1',
        { avg_fps: 68.2, number: 3, fps_1_percent_low: 55, fps_0_1_percent_low: 41 },
        { avg_fps: 84.6, id: 'b2', number: 4, fps_1_percent_low: 72, fps_0_1_percent_low: 60 },
        24
      )
    );
    render(<DashboardView />);
    const fila = filaDe();
    expect(fila.getAttribute('aria-label')).toMatch(/Prueba 4 rinde mejor/);
  });

  it('un caso mixto se anuncia como «Resultado mixto», aunque suba el FPS medio', () => {
    guardadas.push(
      guardada(
        '1',
        { avg_fps: 55.1, fps_1_percent_low: 48, number: 7 },
        { avg_fps: 59.4, id: 'b2', fps_1_percent_low: 30, number: 8 },
        7.8
      )
    );
    render(<DashboardView />);
    const fila = filaDe();
    expect(fila.getAttribute('aria-label')).toMatch(/Resultado mixto/i);
    expect(fila.getAttribute('aria-label')).not.toMatch(/rinde mejor/i);
  });

  it('las grabaciones antiguas se anuncian sin inventarles un número', () => {
    guardadas.push(
      guardada(
        '1',
        { avg_fps: 61.4, launch_command: '' },
        { avg_fps: 63.0, id: 'b2', launch_command: '', timestamp: '2026-10-02 21:40:00' },
        2.6
      )
    );
    render(<DashboardView />);
    const texto = document.body.textContent ?? '';
    // El intérprete las llama «Antes»/«Después»; la lista usa la misma etiqueta que el
    // resto de la app para grabaciones sin número.
    expect(texto).toMatch(/Prueba sin número/);
    expect(texto).not.toMatch(/Prueba 1\b/);
    expect(texto).not.toMatch(/%command%/);
  });

  it('el nombre del juego no se repite cuando la fila anterior es del mismo juego', () => {
    guardadas.push(
      guardada('1', { avg_fps: 68.2 }, { avg_fps: 84.6, id: 'b2' }, 24),
      guardada('2', { avg_fps: 75 }, { avg_fps: 68.5, id: 'b3', timestamp: '2026-11-23 15:10:13' }, -8.7)
    );
    const { container } = render(<DashboardView />);
    const filas = Array.from(container.querySelectorAll('ul > li'));
    const visibles = filas.map((li) =>
      Array.from(li.querySelectorAll('p')).some(
        (p) => !p.className.includes('sr-only') && p.textContent === 'Palworld'
      )
    );
    expect(visibles[0]).toBe(true);
    expect(visibles[1]).toBe(false);
  });

  it('cada fila se puede abrir con el teclado', () => {
    guardadas.push(guardada('1', { avg_fps: 68.2 }, { avg_fps: 84.6, id: 'b2' }, 24));
    render(<DashboardView />);
    const fila = filaDe();
    expect(fila.getAttribute('role')).toBe('button');
    expect(fila.getAttribute('tabindex')).toBe('0');
    fireEvent.keyDown(fila, { key: 'Enter' });
    // Al abrir se entra en el detalle de la prueba guardada.
    return waitFor(() => expect(screen.getByText(/Prueba guardada/i)).toBeDefined());
  });

  it('la lista va de la más reciente a la más antigua', () => {
    guardadas.push(
      guardada('1', { avg_fps: 68.2 }, { avg_fps: 84.6, id: 'b2', timestamp: '2026-11-24 19:20:05' }, 24),
      guardada('2', { avg_fps: 75 }, { avg_fps: 68.5, id: 'b3', timestamp: '2026-11-23 15:10:13' }, -8.7)
    );
    render(<DashboardView />);
    const fechas = screen.getAllByRole('button', { name: /FPS\./ }).map((el) => el.textContent ?? '');
    expect(fechas[0]).toContain('24/11/2026');
    expect(fechas[1]).toContain('23/11/2026');
  });
});
