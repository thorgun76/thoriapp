import { describe, expect, it } from 'vitest';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import { NOMBRE_ANTES, NOMBRE_DESPUES, summarizeComparison } from './BenchmarksView';
import { interpretar, ANTES, DESPUES } from '../lib/interpretacion';

/**
 * El veredicto tiene que responder a la pregunta que trae el usuario («¿cuál es mejor?»)
 * nombrando al ganador con los mismos nombres que ven las tarjetas.
 *
 * Este fichero probaba antes `buildVerdict`, que ponderaba velocidad y estabilidad con
 * pesos 3/2/1. Ese camino ya no existe: todo pasa por `interpretar()`.
 */

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'Juego',
    appid: 440,
    timestamp: '2026-11-02 14:43:52',
    duration_seconds: 60,
    avg_fps: 60,
    fps_1_percent_low: 40,
    fps_0_1_percent_low: 30,
    max_fps: 90,
    min_fps: 20,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 25,
    total_frames: 3600,
    notes: null,
    launch_command: '%command%',
    spikes_per_min: 2,
    pct_over_33ms: 1,
    jitter_ms: 1.5,
    worst_frame_ms: 40,
    warmup_seconds: 0,
    noise_percent: null,
    ...over,
  };
}

function comparison(base: Partial<Benchmark>, test: Partial<Benchmark>): BenchmarkComparison {
  return {
    baseline: bench(base),
    test: bench(test),
    fps_change_percent: 0,
    noise_percent: null,
    within_noise: false,
  } as BenchmarkComparison;
}

const verdictOf = (c: BenchmarkComparison) => interpretar(c);

describe('veredicto: dice quién gana', () => {
  it('cuando la nueva va mejor, gana «Final»', () => {
    const v = verdictOf(comparison({}, { avg_fps: 75, fps_1_percent_low: 55 }));
    expect(v.estado).toBe('mejora');
    expect(v.titulo).toBe(`Gana «${NOMBRE_DESPUES}»`);
  });

  it('cuando la nueva va peor, gana «Inicial»', () => {
    // Los lows bajan en la misma proporción que la media: si cayeran menos que ella, la
    // estabilidad subiría y el resultado sería mixto (eso se prueba aparte).
    const v = verdictOf(comparison({}, { avg_fps: 45, fps_1_percent_low: 30, fps_0_1_percent_low: 22 }));
    expect(v.estado).toBe('empeora');
    expect(v.titulo).toBe(`Gana «${NOMBRE_ANTES}»`);
  });

  it('sin diferencia real no se inventa un ganador', () => {
    const v = verdictOf(comparison({}, { avg_fps: 60.5 }));
    expect(v.estado).toBe('sin_cambio');
    expect(v.titulo).toBe('Sin diferencia concluyente');
  });

  it('más FPS pero más tirones: resultado mixto', () => {
    const v = verdictOf(
      comparison({}, { avg_fps: 80, fps_1_percent_low: 20, spikes_per_min: 12, jitter_ms: 5 })
    );
    expect(v.estado).toBe('mixto');
  });

  it('el titular nunca habla de «la grabación nueva»', () => {
    // Era justo el texto que no se entendía junto a las tarjetas Antes/Después.
    const casos = [
      comparison({}, { avg_fps: 75, fps_1_percent_low: 55 }),
      comparison({}, { avg_fps: 45, fps_1_percent_low: 30, fps_0_1_percent_low: 22 }),
      comparison({}, { avg_fps: 80, fps_1_percent_low: 20, spikes_per_min: 12 }),
      comparison({}, { avg_fps: 60.5 }),
    ];
    for (const c of casos) {
      const v = verdictOf(c);
      expect(v.titulo).not.toMatch(/grabación nueva|grabación anterior/i);
    }
  });

  it('los nombres que exporta BenchmarksView son los del intérprete', () => {
    // Si alguien renombra las tarjetas en un sitio y no en el otro, esto falla.
    expect(NOMBRE_ANTES).toBe(ANTES);
    expect(NOMBRE_DESPUES).toBe(DESPUES);
  });
});

describe('summarizeComparison delega en interpretar()', () => {
  const casos: Array<[string, BenchmarkComparison]> = [
    ['mejora', comparison({}, { avg_fps: 75, fps_1_percent_low: 55 })],
    ['empeora', comparison({}, { avg_fps: 45, fps_1_percent_low: 30, fps_0_1_percent_low: 22 })],
    ['sin cambio', comparison({}, { avg_fps: 60.5 })],
    [
      'mixto',
      comparison({}, { avg_fps: 80, fps_1_percent_low: 20, spikes_per_min: 12, jitter_ms: 5 }),
    ],
  ];

  it.each(casos)('%s: mismo titular que el intérprete', (_n, c) => {
    // `summarizeComparison` ya no expone consejo: el intérprete dejó de producirlo para
    // no repetir en texto lo que ya dicen el titular y «Mejor resultado observado».
    const resumen = summarizeComparison(c);
    const r = interpretar(c);
    expect(resumen.title).toBe(r.titulo);
    expect(Object.keys(resumen).sort()).toEqual(['title', 'winner']);
  });

  it.each(casos)('%s: el ganador del resumen coincide con el del intérprete', (_n, c) => {
    const mapa = { antes: 'antes', despues: 'despues', ninguno: 'empate', mixto: 'depende' };
    expect(summarizeComparison(c).winner).toBe(mapa[interpretar(c).ganador]);
  });
});
