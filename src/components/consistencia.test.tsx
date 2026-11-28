import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import { ComparisonResultCard, summarizeComparison } from './BenchmarksView';
import { SavedComparisonDetail } from './SavedComparisonDetail';
import { etiquetasDe, ANTES, DESPUES } from '../lib/etiquetas';
import {
  presentarEvaluacion,
  presentarGuardada,
  presentarSinEvaluacion,
} from '../lib/presentacionEvaluacion';
import { tituloDeCabecera } from './BenchmarksView';
import type { EvaluacionPersistida, SavedComparison } from '../lib/api';

afterEach(cleanup);

/**
 * Consistencia entre las dos pantallas que muestran un resultado.
 *
 * **Reescrito en C.3.** La versión anterior comparaba la pantalla de Benchmarks (que
 * interpretaba en el frontend con `interpretar()`) contra la guardada, y comprobaba que
 * dijeran cosas distintas. Ahora **las dos leen el mismo modelo**: la única diferencia
 * legítima es la evaluación que cada una tiene delante, no la pantalla.
 *
 * Lo que se fija aquí:
 *  - con la **misma** evaluación, las dos pantallas coinciden en titular y en ganadora;
 *  - ninguna inventa un ganador cuando el motor no lo dio;
 *  - una guardada sin evaluación no se reinterpreta;
 *  - las cifras y los nombres de las grabaciones son los mismos en las dos.
 */

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'Counter-Strike 2',
    appid: 730,
    timestamp: '2026-11-02 14:43:52',
    duration_seconds: 60,
    avg_fps: 60,
    fps_1_percent_low: 40,
    fps_0_1_percent_low: 30,
    max_fps: 90,
    min_fps: 20,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 25,
    total_frames: 600,
    notes: null,
    launch_command: 'mangohud %command%',
    spikes_per_min: 2,
    pct_over_33ms: 1,
    jitter_ms: 1.5,
    worst_frame_ms: 40,
    warmup_seconds: 0,
    noise_percent: 4,
    ...over,
  };
}

function comparison(base: Partial<Benchmark>, test: Partial<Benchmark>): BenchmarkComparison {
  return {
    baseline: bench({ id: 'antes', ...base }),
    test: bench({ id: 'despues', ...test }),
    fps_change_percent: 0,
    noise_percent: 4,
    within_noise: false,
  } as BenchmarkComparison;
}

/** Props obligatorias de la vista guardada que no afectan al resultado. */
const propsVista = { onBack: () => {}, onDelete: () => {}, onCopy: () => {} };

/**
 * Estados del motor, uno por cada rama de la presentación.
 *
 * Ya no se clasifican mirando las cifras: el motor es quien decide, y aquí lo único que se
 * varía es el estado que registró.
 */
const ESTADOS = [
  'mejora',
  'empeora',
  'mixto',
  'sin_cambio',
  'datos_insuficientes',
  'solo_descriptiva',
] as const;

const CASOS: Array<[string, BenchmarkComparison, (typeof ESTADOS)[number]]> = [
  ['mejora', comparison({}, { avg_fps: 75, fps_1_percent_low: 55 }), 'mejora'],
  ['empeora', comparison({}, { avg_fps: 45, fps_1_percent_low: 30 }), 'empeora'],
  ['sin cambio', comparison({}, { avg_fps: 61 }), 'sin_cambio'],
  ['mixto', comparison({}, { avg_fps: 72, fps_1_percent_low: 28 }), 'mixto'],
  [
    'datos insuficientes',
    comparison({ avg_fps: 0, fps_1_percent_low: 0 }, { avg_fps: 75, fps_1_percent_low: 55 }),
    'datos_insuficientes',
  ],
  [
    'solo descriptiva',
    comparison({}, { avg_fps: 70, fps_1_percent_low: 50 }),
    'solo_descriptiva',
  ],
];

/** Evaluación con la forma que produce Rust, coherente con la comparación que se le pase. */
function evaluacion(
  c: BenchmarkComparison,
  estado: (typeof ESTADOS)[number],
  over: Partial<EvaluacionPersistida> = {}
): EvaluacionPersistida {
  return {
    version: 1,
    metodologia: 'v1',
    metodologia_interpretacion: 'v1',
    evaluada_en: '2026-01-02 11:00:00',
    estado,
    confianza: 'provisional',
    referencia: { id: c.baseline.id, number: 1 },
    candidata: { id: c.test.id, number: 2 },
    condiciones: {
      margen_porcentaje: 5,
      ruido_combinado_porcentaje: 4,
      compatibilidad_metodologica: 'comparable',
    },
    ruido_combinado_porcentaje: 4,
    motivos: [],
    limitaciones: [],
    metricas: [],
    ...over,
  } as EvaluacionPersistida;
}

/**
 * Una prueba guardada.
 *
 * `estadoDeLaEvaluacion` se deja sin fijar por defecto **a propósito**: solo Rust lo
 * escribe, y estos casos prueban el respaldo del frontend para listas que no lo traen.
 */
const guardada = (
  c: BenchmarkComparison,
  ev: EvaluacionPersistida | null,
  estadoDeLaEvaluacion?: SavedComparison['estado_de_la_evaluacion']
): SavedComparison =>
  ({
    id: 'c1',
    comparison: c,
    evaluacion: ev,
    ...(estadoDeLaEvaluacion ? { estado_de_la_evaluacion: estadoDeLaEvaluacion } : {}),
  }) as SavedComparison;

describe('las dos pantallas leen el mismo modelo', () => {
  it.each(CASOS)('%s: mismo titular en Benchmarks y en la guardada', (_nombre, c, estado) => {
    const ev = evaluacion(c, estado);
    const modelo = presentarEvaluacion(ev, c, { estado: 'soportada' });
    const esperado = tituloDeCabecera(modelo, etiquetasDe(c));

    const { container: bench1, unmount } = render(
      <ComparisonResultCard comparison={c} evaluacion={ev} />
    );
    expect(bench1.textContent).toContain(esperado);
    unmount();

    // El detalle pinta su propia redacción de cabecera, pero la **ganadora** y las cifras
    // son las mismas: se comprueba que no aparece la ganadora contraria.
    const { container: detalle } = render(
      <SavedComparisonDetail comparison={c} evaluacion={ev} {...propsVista} />
    );
    const favorable = modelo.quienFavorece;
    const esperada = favorable === 'candidata' ? etiquetasDe(c).despues
      : favorable === 'referencia' ? etiquetasDe(c).antes
      : null;
    if (esperada) {
      expect(detalle.textContent).toContain(`Mejor resultado observado: ${esperada}`);
    } else {
      expect(detalle.textContent).not.toContain('Mejor resultado observado');
    }
  });

  it.each(CASOS)('%s: ninguna pantalla inventa un ganador', (_nombre, c, estado) => {
    const ev = evaluacion(c, estado);
    const modelo = presentarEvaluacion(ev, c, { estado: 'soportada' });
    if (modelo.quienFavorece != null) return;

    const prohibido = new RegExp(`Gana «(${ANTES}|${DESPUES})»|Mejor resultado observado`);

    const { container, unmount } = render(
      <ComparisonResultCard comparison={c} evaluacion={ev} />
    );
    expect(container.textContent ?? '').not.toMatch(prohibido);
    unmount();

    const { container: c2 } = render(
      <SavedComparisonDetail comparison={c} evaluacion={ev} {...propsVista} />
    );
    expect(c2.textContent ?? '').not.toMatch(prohibido);
  });

  it('la guardada muestra la evaluación registrada, no una calculada ahora', () => {
    // Los datos parecen una mejora clara; la evaluación guardada dice «empeora». Manda la
    // guardada, porque es lo que se concluyó entonces.
    const c = comparison({}, { avg_fps: 75, fps_1_percent_low: 55, fps_0_1_percent_low: 45 });
    const ev = evaluacion(c, 'empeora');

    const { container } = render(
      <SavedComparisonDetail comparison={c} evaluacion={ev} {...propsVista} />
    );
    expect(container.textContent).toContain('Empeora');
    expect(container.textContent).not.toContain('Evaluación original no registrada');
    // Y la ganadora es la anterior, no la nueva.
    expect(container.textContent).toContain(`Mejor resultado observado: ${ANTES}`);
  });

  it('sin evaluación registrada no se reinterpreta nada', () => {
    const c = comparison({}, { avg_fps: 120, fps_1_percent_low: 100 });

    const { container } = render(<SavedComparisonDetail comparison={c} {...propsVista} />);
    expect(container.textContent).toContain('Evaluación original no registrada');
    expect(container.textContent).not.toContain('Mejor resultado observado');
    expect(container.textContent).not.toContain('Empeora');
  });

  it('una evaluación malformada se presenta como no legible, sin lanzar', () => {
    // Antes esto rompía la pantalla entera (H5): un registro imposible tumbaba la vista.
    const c = comparison({}, {});
    const rota = { metodologia_interpretacion: 'v1', estado: 'mejora' } as EvaluacionPersistida;

    expect(() =>
      render(<SavedComparisonDetail comparison={c} evaluacion={rota} {...propsVista} />)
    ).not.toThrow();

    const { container } = render(
      <SavedComparisonDetail comparison={c} evaluacion={rota} {...propsVista} />
    );
    expect(container.textContent).toContain('Evaluación no legible');
    expect(container.textContent).not.toContain('Mejor resultado observado');
  });

  it('las dos pantallas usan los mismos nombres para las grabaciones', () => {
    const c = CASOS[0][1];
    const ev = evaluacion(c, 'mejora');
    for (const nodo of [
      <ComparisonResultCard comparison={c} evaluacion={ev} />,
      <SavedComparisonDetail comparison={c} evaluacion={ev} {...propsVista} />,
    ]) {
      const { container, unmount } = render(nodo);
      const texto = container.textContent ?? '';
      expect(texto).toContain(ANTES);
      expect(texto).toContain(DESPUES);
      unmount();
    }
  });
});

describe('ninguna pantalla promete más de lo que los datos permiten', () => {
  const PROHIBIDO = /confirmad|demostrad|garantiz|\bfluidez\b/i;

  it.each(CASOS)('%s: ni la de Benchmarks ni la guardada', (_nombre, c, estado) => {
    const ev = evaluacion(c, estado);
    const { container, unmount } = render(
      <ComparisonResultCard comparison={c} evaluacion={ev} />
    );
    expect(container.textContent ?? '').not.toMatch(PROHIBIDO);
    unmount();

    const { container: c2 } = render(
      <SavedComparisonDetail comparison={c} evaluacion={ev} {...propsVista} />
    );
    expect(c2.textContent ?? '').not.toMatch(PROHIBIDO);
  });

  it('la confianza que se muestra es la registrada', () => {
    const c = CASOS[0][1];

    // Sin evaluación no hay chip de confianza: no hay nada que prometer.
    const modeloHistorica = presentarSinEvaluacion(c);
    expect(modeloHistorica.confianza).toBeNull();

    // Con evaluación, la que trae el registro.
    const ev = evaluacion(c, 'mejora', { confianza: 'provisional' });
    expect(presentarEvaluacion(ev, c, { estado: 'soportada' }).confianza?.texto).toBe(
      'Provisional'
    );

    const { container } = render(
      <SavedComparisonDetail comparison={c} evaluacion={ev} {...propsVista} />
    );
    expect(container.textContent).toContain('Provisional');
  });

  it('una guardada malformada en el listado no pierde el estado', () => {
    // `presentarGuardada` no lanza: devuelve `malformada` para que la lista siga viva.
    const c = comparison({}, {});
    const m = presentarGuardada(
      guardada(c, { metodologia_interpretacion: 'v1' } as EvaluacionPersistida)
    );
    expect(m.titulo).toBe('malformada');
    expect(m.quienFavorece).toBeNull();
  });
});

describe('summarizeComparison sigue siendo el resumen de lo no evaluado', () => {
  it.each(CASOS)('%s: sin evaluación no nombra ganadora', (_nombre, c) => {
    const resumen = summarizeComparison(c);
    expect(resumen.winner).toBe('depende');
    expect(resumen.title).toBe(tituloDeCabecera(presentarSinEvaluacion(c)));
  });
});
