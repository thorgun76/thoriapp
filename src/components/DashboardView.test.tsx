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
/** Error de lectura simulado (subfase 5.1). `null` = lectura correcta. */
let errorLectura: string | null = null;

vi.mock('../hooks/useSavedComparisons', () => ({
  useSavedComparisons: () => ({
    items: guardadas,
    loading: false,
    remove: vi.fn(),
    removeAll: vi.fn(),
    error: errorLectura,
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
  errorLectura = null;
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

  it('una prueba guardada sin evaluación no inventa un veredicto', () => {
    // Aunque los tres valores suban y el intérprete la llamaría mejora, la prueba se
    // guardó sin evaluación registrada: el listado lo dice y no concluye nada.
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
    expect(fila.getAttribute('aria-label')).toContain('Sin evaluación registrada');
    expect(fila.getAttribute('aria-label')).not.toMatch(/rinde mejor|rinde peor/i);
  });

  it('una prueba guardada con evaluación muestra la registrada, no una nueva', () => {
    // Los datos dirían «mejora», pero lo registrado fue «resultado mixto»: manda lo
    // guardado.
    const c = guardada(
      '1',
      { avg_fps: 55.1, fps_1_percent_low: 48, number: 7 },
      { avg_fps: 59.4, id: 'b2', fps_1_percent_low: 30, number: 8 },
      7.8
    );
    c.evaluacion = {
      metodologia_interpretacion: 'v1',
      evaluada_en: '2026-01-02 11:00:00',
      referencia: { id: c.comparison.baseline.id },
      candidata: { id: c.comparison.test.id },
      estado: 'mixto',
      confianza: 'provisional',
      condiciones: { compatibilidad_metodologica: 'establecida' },
    };
    c.estado_de_la_evaluacion = 'soportada';
    guardadas.push(c);

    render(<DashboardView />);
    const fila = filaDe();
    expect(fila.getAttribute('aria-label')).toContain('Resultado mixto');
    expect(fila.getAttribute('aria-label')).not.toMatch(/rinde mejor/i);
  });

  it('una mejora con cobertura parcial no se anuncia como victoria global [C.3]', () => {
    // Caso real del motor: FPS y 1 % low suben, el 0,1 % low cae un 50 % pero no es
    // elegible, así que el motor decide `mejora` con `evaluacion_parcial`.
    const c = guardada(
      '1',
      { avg_fps: 100, fps_1_percent_low: 80, fps_0_1_percent_low: 60, number: 1 },
      { avg_fps: 120, id: 'b2', fps_1_percent_low: 96, fps_0_1_percent_low: 30, number: 2 },
      20
    );
    c.evaluacion = {
      metodologia_interpretacion: 'v1',
      evaluada_en: '2026-01-02 11:00:00',
      referencia: { id: c.comparison.baseline.id },
      candidata: { id: c.comparison.test.id },
      estado: 'mejora',
      confianza: 'observada',
      condiciones: { compatibilidad_metodologica: 'establecida', margen_porcentaje: 5 },
      motivos: [],
      limitaciones: [
        'no_elegible',
        'apoyo_insuficiente',
        'perdida_observada_sin_evaluar',
        'evaluacion_parcial',
      ],
      metricas: [],
    } as never;
    c.estado_de_la_evaluacion = 'soportada';
    guardadas.push(c);

    render(<DashboardView />);
    const etiqueta = filaDe().getAttribute('aria-label') ?? '';
    // Se dice que el cambio es parcial, y no se anuncia una mejora sin matices.
    expect(etiqueta).toContain('en parte de las métricas');
    expect(etiqueta).not.toMatch(/\brinde mejor\b/i);
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

  // ── Subfase 5.1: un fallo de lectura no se disfraza de «no hay pruebas» ──

  it('s51 avisa cuando no se pudo leer el archivo en vez de decir que no hay pruebas', () => {
    errorLectura =
      'Tus pruebas guardadas está dañado (expected value). No se ha modificado nada en disco.';
    render(<DashboardView />);

    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/No se pudieron leer tus pruebas guardadas/)).toBeTruthy();
    // La pantalla vacía afirmaría lo contrario de lo que sabemos: que no hay ninguna.
    expect(screen.queryByText(/No hay pruebas guardadas aún/)).toBeNull();
  });

  it('s51 el aviso dice dónde siguen los datos, para no asustar sin motivo', () => {
    errorLectura = 'Tus pruebas guardadas está dañado.';
    render(<DashboardView />);

    const alerta = screen.getByRole('alert');
    expect(alerta.textContent).toContain('saved_comparisons.json');
    expect(alerta.textContent).toContain('no se han borrado');
  });

  it('s51 sin error sigue diciendo «no hay pruebas» cuando de verdad no hay', () => {
    render(<DashboardView />);

    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByText(/No hay pruebas guardadas aún/)).toBeTruthy();
  });
});
