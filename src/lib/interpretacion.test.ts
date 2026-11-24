import { describe, expect, it } from 'vitest';
import { interpretar, nombreLegible } from './interpretacion';
import type { Benchmark, BenchmarkComparison } from './types';

/**
 * Estos tests fijan la política de interpretación: qué se puede afirmar y qué no.
 * Son la parte que un usuario nota como «la app me miente» cuando se rompe.
 */

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'Juego',
    appid: 440,
    timestamp: '2026-11-02 14:43:52',
    duration_seconds: 60,
    avg_fps: 100,
    fps_1_percent_low: 70,
    fps_0_1_percent_low: 60,
    max_fps: 140,
    min_fps: 40,
    frame_time_avg: 10,
    frame_time_1_percent_high: 14.3,
    total_frames: 600,
    notes: null,
    launch_command: '%command%',
    spikes_per_min: 2,
    pct_over_33ms: 1,
    jitter_ms: 1.5,
    worst_frame_ms: 25,
    warmup_seconds: 10,
    noise_percent: 3,
    log_path: '/x/a.csv',
    ...over,
  };
}

function cmp(
  antes: Partial<Benchmark>,
  despues: Partial<Benchmark>,
  noise: number | null = 5
): BenchmarkComparison {
  return {
    baseline: bench(antes),
    test: bench(despues),
    fps_change_percent: 0,
    fps_1_percent_change_percent: 0,
    fps_0_1_percent_change_percent: 0,
    frame_time_change_percent: 0,
    conclusion: '',
    noise_percent: noise,
    within_noise: false,
  } as BenchmarkComparison;
}

describe('interpretación: estados', () => {
  it('FPS prácticamente iguales → sin diferencia concluyente', () => {
    const r = interpretar(cmp({ avg_fps: 97.8 }, { avg_fps: 100.7 }));
    expect(r.estado).toBe('sin_cambio');
    expect(r.titulo).toBe('Sin diferencia concluyente');
    expect(r.chipConfianza).toBe('Dentro del margen estimado');
  });

  it('mejora de FPS dentro del margen no se llama mejora', () => {
    // +3,0 % con margen ±5,1 %: exactamente el caso del enunciado.
    const r = interpretar(
      cmp({ avg_fps: 97.8, fps_1_percent_low: 70, fps_0_1_percent_low: 60 },
          { avg_fps: 100.7, fps_1_percent_low: 72, fps_0_1_percent_low: 62 },
          5.1)
    );
    expect(r.estado).toBe('sin_cambio');
    expect(r.explicacion).toContain('prácticamente iguales');
  });

  it('mejora de FPS fuera del margen → mejora observada, no confirmada', () => {
    const r = interpretar(
      cmp({ avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60 },
          { avg_fps: 130, fps_1_percent_low: 91, fps_0_1_percent_low: 78 })
    );
    expect(r.estado).toBe('mejora');
    expect(r.confianza).toBe('provisional');
    expect(r.chipConfianza).toBe('Resultado provisional');
    // Nunca se dice "confirmada" ni se inventa una confianza.
    expect(r.chipConfianza).not.toMatch(/confirmad/i);
  });

  it('empeoramiento claro', () => {
    const r = interpretar(
      cmp({ avg_fps: 130, fps_1_percent_low: 91, fps_0_1_percent_low: 78 },
          { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60 })
    );
    expect(r.estado).toBe('empeora');
    expect(r.tono).toBe('mal');
  });

  it('mejora de estabilidad con FPS medios iguales → mejora', () => {
    const r = interpretar(
      cmp({ avg_fps: 60, fps_1_percent_low: 18, fps_0_1_percent_low: 12 },
          { avg_fps: 60, fps_1_percent_low: 42, fps_0_1_percent_low: 30 })
    );
    expect(r.estado).toBe('mejora');
    expect(r.explicacion).toMatch(/estabilidad/i);
    const est = r.metricas.find((m) => m.clave === 'estabilidad')!;
    expect(est.cambio).toBe('mejor');
    // Es la diferencia en puntos, no en %.
    expect(est.diferenciaPuntos).toBeGreaterThan(20);
    expect(est.diferenciaPct).toBeNull();
  });

  it('mejora de estabilidad con FPS medios distintos', () => {
    const r = interpretar(
      cmp({ avg_fps: 80, fps_1_percent_low: 20, fps_0_1_percent_low: 14 },
          { avg_fps: 120, fps_1_percent_low: 60, fps_0_1_percent_low: 45 })
    );
    expect(r.estado).toBe('mejora');
    expect(r.metricas[0].cambio).toBe('mejor');
  });

  it('más FPS pero peor estabilidad → mixto, no "gana el después"', () => {
    const r = interpretar(
      cmp({ avg_fps: 60, fps_1_percent_low: 45, fps_0_1_percent_low: 40 },
          { avg_fps: 90, fps_1_percent_low: 30, fps_0_1_percent_low: 20 })
    );
    expect(r.estado).toBe('mixto');
    expect(r.tono).toBe('atencion');
    expect(r.explicacion).toMatch(/suben|Empeora/);
  });

  it('menos FPS pero mejor estabilidad → mixto', () => {
    const r = interpretar(
      cmp({ avg_fps: 90, fps_1_percent_low: 30, fps_0_1_percent_low: 20 },
          { avg_fps: 60, fps_1_percent_low: 50, fps_0_1_percent_low: 45 })
    );
    expect(r.estado).toBe('mixto');
  });
});

describe('interpretación: datos que faltan o están corruptos', () => {
  it('FPS medios cero → datos insuficientes, no un 0 %', () => {
    const r = interpretar(cmp({ avg_fps: 0 }, { avg_fps: 0 }));
    expect(r.estado).toBe('datos_insuficientes');
    expect(r.confianza).toBe('insuficiente');
    // Los FPS son `null` (no medidos). Los percentiles sí existen en el fichero y se
    // muestran como dato: cero invención en ningún caso.
    const avg = r.metricas.find((m) => m.clave === 'avg_fps')!;
    expect(avg.antes).toBeNull();
    expect(avg.diferencia).toBeNull();
  });

  it('NaN en los FPS no se presenta como medida', () => {
    const r = interpretar(cmp({ avg_fps: NaN }, { avg_fps: 100 }));
    expect(r.estado).toBe('datos_insuficientes');
  });

  it('percentiles ausentes no inventan métricas', () => {
    const r = interpretar(
      cmp({ fps_1_percent_low: 0, fps_0_1_percent_low: 0 },
          { fps_1_percent_low: 0, fps_0_1_percent_low: 0 },
          20) // margen grande para que los FPS medios sean "sin cambio"
    );
    expect(r.metricas.find((m) => m.clave === 'low1')!.antes).toBeNull();
    expect(r.metricas.find((m) => m.clave === 'low1')!.cambio).toBe('sin_datos');
  });

  it('sin margen guardado se usa el mínimo y se avisa', () => {
    const r = interpretar(cmp({}, {}, null));
    expect(r.margenTexto).toBeNull();
    expect(r.avisos.some((a) => a.includes('versión anterior'))).toBe(true);
  });

  it('grabaciones cortas avisan de los valores extremos', () => {
    const r = interpretar(cmp({ duration_seconds: 12 }, { duration_seconds: 12 }));
    expect(r.avisos.some((a) => a.includes('menos de 20 s'))).toBe(true);
  });

  it('duraciones muy distintas avisan', () => {
    const r = interpretar(cmp({ duration_seconds: 30 }, { duration_seconds: 120 }));
    expect(r.avisos.some((a) => a.includes('duran muy distinto'))).toBe(true);
  });

  it('el 0,1 % low con pocas muestras genera aviso con cifras', () => {
    const r = interpretar(cmp({}, {}));
    expect(r.low01.fiable).toBe(false);
    expect(r.avisos.some((a) => a.includes('0,1 %'))).toBe(true);
  });

  it('juegos distintos se marcan como comparación inválida', () => {
    const r = interpretar(cmp({ game_name: 'A' }, { game_name: 'B' }));
    expect(r.avisos.some((a) => a.includes('juegos distintos'))).toBe(true);
  });
});

describe('interpretación: textos', () => {
  it('usa los nombres Antes/Después, no Inicial/Final', () => {
    const r = interpretar(cmp({ avg_fps: 100 }, { avg_fps: 130 }));
    const todo = [r.titulo, r.explicacion, ...r.avisos].join(' ');
    expect(todo).not.toMatch(/Inicial|Final/);
  });

  it('nunca dice "fluidez" ni "confirmada"', () => {
    for (const despues of [{ avg_fps: 130 }, { avg_fps: 70 }, { avg_fps: 100.5 }]) {
      const r = interpretar(cmp({}, despues));
      const todo = [r.titulo, r.explicacion, ...r.avisos].join(' ');
      expect(todo).not.toMatch(/fluidez/i);
      expect(todo).not.toMatch(/confirmad/i);
    }
  });

  it('la explicación incluye los números reales', () => {
    const r = interpretar(cmp({ avg_fps: 97.8 }, { avg_fps: 100.7 }, 5.1));
    expect(r.explicacion).toContain('97.8');
    expect(r.explicacion).toContain('100.7');
  });
});

describe('nombreLegible', () => {
  it('quita los sufijos técnicos', () => {
    expect(nombreLegible('Palworld-Win64-Shipping')).toBe('Palworld');
    expect(nombreLegible('7DaysToDie.x86_64')).toBe('7DaysToDie');
  });

  it('sin nombre devuelve un texto neutro', () => {
    expect(nombreLegible('')).toBe('Juego desconocido');
    expect(nombreLegible(null)).toBe('Juego desconocido');
  });
});
