import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import { ComparisonResultCard, summarizeComparison } from './BenchmarksView';
import { SavedComparisonDetail } from './SavedComparisonDetail';
import { titularDelCambio, etiquetasDe, interpretar, ANTES, DESPUES } from '../lib/interpretacion';

afterEach(cleanup);

/**
 * Consistencia entre las dos pantallas que muestran un resultado.
 *
 * Ya **no** tienen que decir lo mismo, y esa es la corrección: la pantalla de Benchmarks
 * interpreta una comparación recién hecha con las reglas de hoy, mientras que la de pruebas
 * guardadas muestra el veredicto que se **registró** al guardarla. Si se interpretara otra
 * vez, una prueba antigua cambiaría de conclusión al actualizar la app.
 *
 * Lo que sí deben compartir: las cifras medidas, y no inventar un ganador que no consta.
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

/** Casos que cubren los cinco estados del intérprete. */
const CASOS: Array<[string, ReturnType<typeof comparison>]> = [
  ['mejora clara', comparison({}, { avg_fps: 75, fps_1_percent_low: 55, fps_0_1_percent_low: 45 })],
  [
    'empeora clara',
    comparison({}, { avg_fps: 45, fps_1_percent_low: 30, fps_0_1_percent_low: 22 }),
  ],
  ['sin cambio', comparison({}, { avg_fps: 61 })],
  [
    'mixto',
    comparison({}, { avg_fps: 72, fps_1_percent_low: 28, fps_0_1_percent_low: 20, spikes_per_min: 14 }),
  ],
  [
    'datos insuficientes',
    comparison({ avg_fps: 0, fps_1_percent_low: 0 }, { avg_fps: 75, fps_1_percent_low: 55 }),
  ],
];

describe('las dos pantallas dan el mismo veredicto', () => {
  it.each(CASOS)('%s: la actual interpreta; la guardada NO', (_nombre, c) => {
    // La pantalla de lo recién medido sigue usando el intérprete.
    const esperado = titularDelCambio(interpretar(c), etiquetasDe(c));
    const { unmount } = render(<ComparisonResultCard comparison={c} />);
    expect(screen.getByText(esperado)).toBeDefined();
    unmount();

    // La guardada, sin evaluación registrada, dice justamente eso: no hay veredicto.
    const { container } = render(<SavedComparisonDetail comparison={c} {...propsVista} />);
    expect(container.textContent).toContain('Evaluación original no registrada');
    expect(container.textContent).not.toContain(esperado);
  });

  it('una guardada CON evaluación muestra la evaluación registrada, no una nueva', () => {
    // El intérprete diría «mejora» con estos datos; la evaluación guardada dice «empeora».
    // Debe mandar la guardada.
    const c = comparison({}, { avg_fps: 75, fps_1_percent_low: 55, fps_0_1_percent_low: 45 });
    expect(interpretar(c).estado).toBe('mejora');

    const evaluacion = {
      metodologia_interpretacion: 'v1',
      evaluada_en: '2026-01-02 11:00:00',
      referencia: { id: c.baseline.id },
      candidata: { id: c.test.id },
      estado: 'empeora',
      confianza: 'observada',
      condiciones: { compatibilidad_metodologica: 'establecida' as const },
      estado_de_la_evaluacion: 'soportada' as const,
    };

    const { container } = render(
      <SavedComparisonDetail comparison={c} evaluacion={evaluacion} {...propsVista} />
    );
    expect(container.textContent).toContain('Empeora');
    expect(container.textContent).not.toContain('Evaluación original no registrada');
  });

  it.each(CASOS)('%s: ninguna pantalla inventa un ganador', (_nombre, c) => {
    const g = interpretar(c).ganador;
    if (g === 'ninguno' || g === 'mixto') {
      // No debe aparecer un «Gana «…»» cuando el intérprete no da ganador.
      // Se busca en el texto renderizado entero, no por nodo: el titular puede partirse
      // entre nodos y `queryByText` no lo vería.
      const prohibido = new RegExp(`Gana «(${ANTES}|${DESPUES})»`);
      const { container, unmount } = render(<ComparisonResultCard comparison={c} />);
      expect(container.textContent ?? '').not.toMatch(prohibido);
      unmount();

      const { container: c2 } = render(
        <SavedComparisonDetail comparison={c} {...propsVista} />
      );
      expect(c2.textContent ?? '').not.toMatch(prohibido);
    }
  });

  it.each(CASOS)('%s: ninguna pantalla repite una recomendación', (_nombre, c) => {
    // El titular y «Mejor resultado observado» ya dicen lo que pasó, así que el consejo
    // final se eliminó del intérprete. Aquí se fija que no reaparezca en ninguna pantalla
    // y que el resumen siga diciendo lo mismo que el intérprete.
    const resumen = summarizeComparison(c);
    expect(resumen.title).toBe(interpretar(c).titulo);

    // Solo las frases que se eliminaron, no cualquier mención a «repetir la prueba»:
    // el aviso del margen de ruido dice «repite la prueba antes de decidir» y es legítimo.
    const recomendaciones =
      /Vuelve a «|Repite la prueba para asegurarte|Decide qué te importa|en la misma escena antes de decidir|Vuelve a analizar los ficheros/;

    const { container, unmount } = render(<ComparisonResultCard comparison={c} />);
    expect(container.textContent ?? '').not.toMatch(recomendaciones);
    unmount();

    const { container: c2 } = render(
      <SavedComparisonDetail comparison={c} {...propsVista} />
    );
    expect(c2.textContent ?? '').not.toMatch(recomendaciones);
  });

  it('las dos pantallas usan los mismos nombres para las grabaciones', () => {
    const c = CASOS[0][1];
    for (const [Componente, props] of [
      [ComparisonResultCard, { comparison: c }],
      [SavedComparisonDetail, { comparison: c, nombreLegible: 'CS2' }],
    ] as const) {
      const { container, unmount } = render(
        // @ts-expect-error: los props difieren entre los dos componentes a propósito
        <Componente {...props} />
      );
      const texto = container.textContent ?? '';
      expect(texto).toContain(ANTES);
      expect(texto).toContain(DESPUES);
      unmount();
    }
  });
});

describe('ninguna pantalla promete más de lo que los datos permiten', () => {
  const PROHIBIDO = /confirmad|demostrad|garantiz|\bfluidez\b/i;

  it.each(CASOS)('%s: ni la de Benchmarks ni la guardada', (_nombre, c) => {
    const { container, unmount } = render(<ComparisonResultCard comparison={c} />);
    expect(container.textContent ?? '').not.toMatch(PROHIBIDO);
    unmount();

    const { container: c2 } = render(
      <SavedComparisonDetail comparison={c} {...propsVista} />
    );
    expect(c2.textContent ?? '').not.toMatch(PROHIBIDO);
  });

  it('con dos grabaciones la confianza nunca pasa de provisional', () => {
    // El chip de la vista guardada sale de la confianza **registrada**, no de una
    // interpretación nueva. Sin evaluación registrada no hay chip: no hay nada que
    // prometer.
    const c = CASOS[0][1];
    expect(interpretar(c).confianza).toBe('provisional');

    const { container: sinEval } = render(
      <SavedComparisonDetail comparison={c} {...propsVista} />
    );
    expect(sinEval.textContent).not.toContain(interpretar(c).chipConfianza);

    const evaluacion = {
      metodologia_interpretacion: 'v1',
      evaluada_en: '2026-01-02 11:00:00',
      referencia: { id: c.baseline.id },
      candidata: { id: c.test.id },
      estado: 'mejora',
      confianza: 'provisional',
      condiciones: { compatibilidad_metodologica: 'establecida' as const },
      estado_de_la_evaluacion: 'soportada' as const,
    };
    const { container } = render(
      <SavedComparisonDetail comparison={c} evaluacion={evaluacion} {...propsVista} />
    );
    // La confianza guardada es la que se muestra.
    expect(container.textContent).toContain('Provisional');
  });
});
