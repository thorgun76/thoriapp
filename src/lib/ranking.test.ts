import { describe, expect, it } from 'vitest';
import { clasificar, CRITERIO_RANKING, MAX_RANKING } from './ranking';
import type { Benchmark } from './types';

/** Grabación mínima: solo lo que mira el ranking. */
function bench(
  id: string,
  { low1 = 50, fps = 60, low01 = 40, timestamp = '2024-01-01T00:00:00' } = {}
): Benchmark {
  return {
    id,
    profile_id: '',
    game_name: 'TF2',
    appid: 440,
    timestamp,
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
  };
}

const ids = (lista: { bench: Benchmark }[]) => lista.map((e) => e.bench.id);

describe('clasificar (ranking de un juego)', () => {
  it('ordena por 1 % low, no por FPS medios', () => {
    // La de más FPS medios tiene peores lows: no puede ganar. Es el caso que un
    // promedio esconde (bajar ajustes sube el promedio y hunde los mínimos).
    const lista = clasificar([
      bench('promedio-alto', { low1: 30, fps: 120 }),
      bench('pareja', { low1: 55, fps: 70 }),
    ]);
    expect(ids(lista)).toEqual(['pareja', 'promedio-alto']);
    expect(lista[0].ganadora).toBe(true);
    expect(lista[1].ganadora).toBe(false);
  });

  it('desempata por FPS medios cuando el 1 % low es igual', () => {
    const lista = clasificar([
      bench('lenta', { low1: 50, fps: 60 }),
      bench('rapida', { low1: 50, fps: 90 }),
    ]);
    expect(ids(lista)).toEqual(['rapida', 'lenta']);
  });

  it('desempata por estabilidad cuando lows y FPS coinciden', () => {
    // Mismo 1 % low y mismos FPS medios; la estabilidad depende del 0,1 % low.
    const lista = clasificar([
      bench('inestable', { low1: 50, fps: 60, low01: 10 }),
      bench('estable', { low1: 50, fps: 60, low01: 45 }),
    ]);
    expect(ids(lista)).toEqual(['estable', 'inestable']);
  });

  it('no marca ganadora cuando solo hay una grabación', () => {
    const lista = clasificar([bench('unica')]);
    expect(lista).toHaveLength(1);
    expect(lista[0].ganadora).toBe(false);
    expect(lista[0].puesto).toBe(1);
  });

  it('devuelve lista vacía sin grabaciones', () => {
    expect(clasificar([])).toEqual([]);
  });

  it('asigna puestos consecutivos y diferencia frente a la ganadora', () => {
    const lista = clasificar([
      bench('a', { low1: 60 }),
      bench('b', { low1: 30 }),
      bench('c', { low1: 45 }),
    ]);
    expect(lista.map((e) => e.puesto)).toEqual([1, 2, 3]);
    expect(lista[0].pctVsGanadora).toBe(0);
    // 45 sobre 60 = -25 %
    expect(lista[1].pctVsGanadora).toBeCloseTo(-25, 5);
    // 30 sobre 60 = -50 %
    expect(lista[2].pctVsGanadora).toBeCloseTo(-50, 5);
  });

  it(`no clasifica más de ${MAX_RANKING} grabaciones`, () => {
    const muchas = Array.from({ length: 14 }, (_, i) => bench(`b${i}`, { low1: 20 + i }));
    expect(clasificar(muchas)).toHaveLength(MAX_RANKING);
  });

  it('descarta las grabaciones sin 1 % low en vez de ponerlas últimas con 0', () => {
    const lista = clasificar([
      bench('valida', { low1: 40 }),
      bench('sin-dato', { low1: 0 }),
    ]);
    expect(ids(lista)).toEqual(['valida']);
  });

  it('mantiene el orden de entrada entre iguales salvo por fecha', () => {
    const lista = clasificar([
      bench('vieja', { low1: 50, fps: 60, low01: 40, timestamp: '2024-01-01T00:00:00' }),
      bench('nueva', { low1: 50, fps: 60, low01: 40, timestamp: '2024-06-01T00:00:00' }),
    ]);
    // Todo idéntico: gana la más reciente, que es la que el usuario acaba de hacer.
    expect(ids(lista)).toEqual(['nueva', 'vieja']);
  });

  it('el criterio se explica sin mencionar FPS medios como principal', () => {
    expect(CRITERIO_RANKING).toContain('1 % low');
    expect(CRITERIO_RANKING.startsWith('Ordenadas por el 1 % low')).toBe(true);
  });
});
