import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import type { EvaluacionDeMetrica, EvaluacionPersistida } from '../lib/api';
import { ComparisonResultCard } from './BenchmarksView';
import { SavedComparisonDetail } from './SavedComparisonDetail';
import { DashboardView } from './DashboardView';
import {
  estadoDeEvaluacionGuardada,
  presentarEvaluacion,
  presentarGuardada,
} from '../lib/presentacionEvaluacion';

/**
 * El Dashboard lee sus guardadas de un hook, no de props. Se sustituye por esta lista,
 * que cada escenario rellena antes de renderizar.
 */
const guardadas: unknown[] = [];
vi.mock('../hooks/useSavedComparisons', () => ({
  useSavedComparisons: () => ({
    items: guardadas,
    loading: false,
    remove: vi.fn(),
    removeAll: vi.fn(),
    error: null,
  }),
}));

afterEach(cleanup);

/**
 * Matriz de coherencia entre las tres superficies que muestran un resultado.
 *
 * **Qué es coherencia aquí.** No es que las tres digan lo mismo: el Dashboard resume,
 * Benchmarks juzga una vista previa y el detalle muestra lo guardado. Cada una tiene su
 * responsabilidad y su nivel de detalle. Lo que la matriz persigue es la **ausencia de
 * contradicciones semánticas**: que ninguna afirme una preferencia que el motor no emitió,
 * que ninguna esconda una pérdida observada, y que ninguna convierta un dato descriptivo
 * en un veredicto.
 *
 * **Qué no se comprueba.** No se busca la ausencia de clases verdes o rojas en el HTML:
 * eso confundiría el color con el significado y pasaría en verde con una vista muda. Se
 * miran los **elementos concretos** —la fila de cada métrica, el titular, el contador— y
 * el **texto** que se afirma, con una lista negra contextual: en un resultado MIXTO es
 * legítimo que una métrica diga «mejora»; lo que no es legítimo es que el conjunto se
 * anuncie como ganador.
 */

/* ─────────────────────────────── Utilidades ─────────────────────────────── */

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'Counter-Strike 2',
    appid: 730,
    timestamp: '2026-11-02 14:43:52',
    duration_seconds: 60,
    avg_fps: 120,
    fps_1_percent_low: 90,
    fps_0_1_percent_low: 80,
    max_fps: 200,
    min_fps: 50,
    frame_time_avg: 8.3,
    frame_time_1_percent_high: 12,
    total_frames: 1800,
    notes: null,
    launch_command: 'mangohud %command%',
    spikes_per_min: 1,
    pct_over_33ms: 0.5,
    jitter_ms: 1.1,
    worst_frame_ms: 20,
    warmup_seconds: 10,
    noise_percent: 3,
    log_path: '/tmp/a.csv',
    ...over,
  };
}

function comparacion(
  antes: Partial<Benchmark> = {},
  despues: Partial<Benchmark> = {}
): BenchmarkComparison {
  return {
    baseline: bench({ id: 'ref', ...antes }),
    test: bench({ id: 'cand', ...despues }),
    fps_change_percent: 0,
    fps_1_percent_change_percent: 0,
    fps_0_1_percent_change_percent: 0,
    frame_time_change_percent: 0,
    conclusion: '',
    noise_percent: 3,
    within_noise: false,
  } as BenchmarkComparison;
}

/** Métrica con la forma que escribe Rust. `preferencia: null` = no la juzgó. */
function metrica(
  clave: string,
  preferencia: EvaluacionDeMetrica['preferencia'],
  muestras: number | null = 9000
): EvaluacionDeMetrica {
  const apoyo = muestras == null ? null : { percentil: { muestras, valor: 0 } };
  return {
    clave,
    direccion: clave === 'frame_time_avg' ? 'menos_es_mejor' : 'mas_es_mejor',
    preferencia,
    apoyo_referencia: apoyo,
    apoyo_candidata: apoyo,
  } as unknown as EvaluacionDeMetrica;
}

function evaluacion(
  c: BenchmarkComparison,
  campos: Partial<EvaluacionPersistida> = {}
): EvaluacionPersistida {
  return {
    version: 1,
    metodologia: 'v1',
    metodologia_interpretacion: 'v1',
    evaluada_en: '2026-01-02 11:00:00',
    referencia: { id: c.baseline.id, number: c.baseline.number ?? null },
    candidata: { id: c.test.id, number: c.test.number ?? null },
    estado: 'mejora',
    confianza: 'provisional',
    condiciones: {
      margen_porcentaje: 5,
      ruido_combinado_porcentaje: 3,
      compatibilidad_metodologica: 'establecida',
    },
    ruido_combinado_porcentaje: 3,
    motivos: [],
    limitaciones: [],
    metricas: [],
    ...campos,
  } as EvaluacionPersistida;
}

/** Frases que ninguna superficie puede decir cuando el motor no dio un ganador. */
const SIN_GANADOR = [
  /gana la nueva/i,
  /gana la anterior/i,
  /mejor configuración/i,
  /más rápido que/i,
  /conviene cambiar/i,
];

const textoDe = (n: Element | DocumentFragment) => (n.textContent ?? '').replace(/\s+/g, ' ');

/**
 * Bloque de una métrica cuyo texto contiene su nombre.
 *
 * La tabla de detalle de Benchmarks no es un `<table>`: son filas `div` con una rejilla
 * (ver `DetailsTable`), mientras que la del detalle guardado sí es una tabla real. Se
 * buscan las dos formas, porque la matriz tiene que funcionar en las dos superficies.
 */
function filaDe(container: HTMLElement, nombre: RegExp): HTMLElement | null {
  const candidatas = [
    ...Array.from(container.querySelectorAll('tbody tr')),
    ...Array.from(container.querySelectorAll('div.grid')),
  ];
  return (candidatas.find((f) => nombre.test(textoDe(f))) as HTMLElement | undefined) ?? null;
}

/* ─────────────────────── Los ocho escenarios de la matriz ─────────────────────── */

/**
 * Cada escenario declara lo que el motor dijo y lo que **debe** sostenerse en las tres
 * vistas. `prohibido` son afirmaciones prohibidas en ese escenario concreto.
 */
const ESCENARIOS = [
  {
    nombre: 'mejora completa',
    comparacion: () =>
      comparacion(
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60, frame_time_avg: 10 },
        { avg_fps: 130, fps_1_percent_low: 100, fps_0_1_percent_low: 90, frame_time_avg: 7.7 }
      ),
    evaluacion: (c: BenchmarkComparison) =>
      evaluacion(c, {
        estado: 'mejora',
        metricas: [
          metrica('avg_fps', 'favor_de_candidata'),
          metrica('fps_1_percent_low', 'favor_de_candidata'),
          metrica('fps_0_1_percent_low', 'favor_de_candidata'),
          metrica('frame_time_avg', 'favor_de_candidata'),
        ],
      }),
    // Con el motor afirmando mejora en todo, el ganador es la candidata.
    esperaGanador: 'candidata' as const,
    prohibido: [/favor de la anterior/i, /dentro del ruido/i],
  },
  {
    nombre: 'empeoramiento completo',
    comparacion: () =>
      comparacion(
        { avg_fps: 130, fps_1_percent_low: 100, fps_0_1_percent_low: 90, frame_time_avg: 7.7 },
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60, frame_time_avg: 10 }
      ),
    evaluacion: (c: BenchmarkComparison) =>
      evaluacion(c, {
        estado: 'empeora',
        metricas: [
          metrica('avg_fps', 'favor_de_referencia'),
          metrica('fps_1_percent_low', 'favor_de_referencia'),
          metrica('fps_0_1_percent_low', 'favor_de_referencia'),
          metrica('frame_time_avg', 'favor_de_referencia'),
        ],
      }),
    esperaGanador: 'referencia' as const,
    prohibido: [/favor de la nueva/i, /dentro del ruido/i],
  },
  {
    nombre: 'mejora parcial con pérdida en métrica excluida',
    comparacion: () =>
      comparacion(
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60, frame_time_avg: 10 },
        { avg_fps: 130, fps_1_percent_low: 70, fps_0_1_percent_low: 40, frame_time_avg: 7.7 }
      ),
    evaluacion: (c: BenchmarkComparison) =>
      evaluacion(c, {
        estado: 'mejora',
        // 0,1 % low empeoró de 60 a 40, pero el motor no la juzgó: sin apoyo declarado.
        metricas: [
          metrica('avg_fps', 'favor_de_candidata'),
          metrica('frame_time_avg', 'favor_de_candidata'),
          metrica('fps_1_percent_low', 'indistinto'),
          metrica('fps_0_1_percent_low', 'indistinto', null),
        ],
      }),
    esperaGanador: 'candidata' as const,
    // La pérdida en la métrica no juzgada tiene que seguir visible: no se convierte en
    // «todo mejoró» ni desaparece de la tabla.
    prohibido: [/todas las métricas mejor/i],
    exigePerdidaVisible: true,
  },
  {
    nombre: 'mixto',
    comparacion: () =>
      comparacion(
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60, frame_time_avg: 10 },
        { avg_fps: 130, fps_1_percent_low: 60, fps_0_1_percent_low: 50, frame_time_avg: 7.7 }
      ),
    evaluacion: (c: BenchmarkComparison) =>
      evaluacion(c, {
        estado: 'mixto',
        metricas: [
          metrica('avg_fps', 'favor_de_candidata'),
          metrica('frame_time_avg', 'favor_de_candidata'),
          metrica('fps_1_percent_low', 'favor_de_referencia'),
          metrica('fps_0_1_percent_low', 'favor_de_referencia'),
        ],
      }),
    // En mixto no hay ganador global, pero **sí** hay preferencias individuales válidas y
    // opuestas: la matriz exige que se conserven las dos.
    esperaGanador: null,
    prohibido: [...SIN_GANADOR, /todas las métricas mejor/i],
    exigePreferenciasOpuestas: true,
  },
  {
    nombre: 'sin cambio (indistinto)',
    comparacion: () =>
      comparacion(
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60, frame_time_avg: 10 },
        { avg_fps: 100.4, fps_1_percent_low: 70.2, fps_0_1_percent_low: 60.1, frame_time_avg: 9.98 }
      ),
    evaluacion: (c: BenchmarkComparison) =>
      evaluacion(c, {
        estado: 'sin_cambio',
        metricas: [
          metrica('avg_fps', 'indistinto'),
          metrica('fps_1_percent_low', 'indistinto'),
          metrica('fps_0_1_percent_low', 'indistinto'),
          metrica('frame_time_avg', 'indistinto'),
        ],
      }),
    esperaGanador: null,
    // Ninguna preferencia: tampoco «mejora» ni «empeora» por métrica.
    prohibido: [...SIN_GANADOR, /favorece a/i, /mejor que/i],
  },
  {
    nombre: 'histórica (sin evaluación registrada)',
    comparacion: () =>
      comparacion(
        { avg_fps: 100, fps_1_percent_low: 70 },
        { avg_fps: 130, fps_1_percent_low: 100 }
      ),
    evaluacion: () => null,
    esperaGanador: null,
    // Sin evaluación no se reinterpreta con las reglas de hoy. «Sin evaluación
    // registrada» es la forma correcta de decirlo, así que no está prohibida: lo que se
    // prohíbe es inventar un juicio.
    prohibido: [...SIN_GANADOR, /favorece a/i],
  },
  {
    nombre: 'malformada',
    comparacion: () =>
      comparacion(
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60 },
        { avg_fps: 130, fps_1_percent_low: 100, fps_0_1_percent_low: 90 }
      ),
    // Registro ilegible: Rust conservó la clasificación, pero no hay contenido utilizable.
    evaluacion: () => null,
    estadoDeclarado: 'malformada' as const,
    esperaGanador: null,
    prohibido: [...SIN_GANADOR, /favorece a/i],
  },
  {
    nombre: 'no soportada (metodología de otra versión)',
    comparacion: () =>
      comparacion(
        { avg_fps: 100, fps_1_percent_low: 70 },
        { avg_fps: 130, fps_1_percent_low: 100 }
      ),
    evaluacion: (c: BenchmarkComparison) =>
      evaluacion(c, {
        metodologia_interpretacion: 'v0',
        estado: 'mejora',
        metricas: [metrica('avg_fps', 'favor_de_candidata')],
      }),
    esperaGanador: null,
    prohibido: [...SIN_GANADOR, /favorece a/i],
  },
];

/* ─────────────────────────────── La matriz ─────────────────────────────── */

describe('matriz de coherencia: BenchmarksView × SavedComparisonDetail × DashboardView', () => {
  for (const esc of ESCENARIOS) {
    describe(esc.nombre, () => {
      const c = esc.comparacion();
      const ev = esc.evaluacion(c);

      /**
       * Modelo compartido, igual que lo construye cada vista.
       *
       * Importante: cuando la evaluación existe pero su metodología no es la de esta
       * versión, la app **no** llama a `presentarEvaluacion` como si fuera válida; pasa
       * por `estadoDeEvaluacionGuardada`, que la clasifica como `no_soportada`. Se replica
       * aquí ese camino porque es el real: si la matriz llamara directamente, probaría una
       * ruta que la aplicación no usa.
       */
      const modeloVista = () =>
        ev == null
          ? presentarGuardada({
              evaluacion: null,
              estado_de_la_evaluacion: (esc as { estadoDeclarado?: string }).estadoDeclarado as never,
              comparison: c,
            })
          : presentarEvaluacion(ev, c, {
              origen: 'evaluada',
              estado: estadoDeEvaluacionGuardada({ evaluacion: ev }),
            });

      it('el modelo compartido coincide con lo que declara el escenario', () => {
        const m = modeloVista();
        const ganadora = m.quienFavorece;
        expect(ganadora).toBe(esc.esperaGanador);

        if (esc.esperaGanador == null) {
          for (const p of esc.prohibido) {
            expect(textoDe(document.body)).not.toMatch(p);
          }
        }

        // Las preferencias individuales se conservan aunque no haya ganador global.
        if (esc.exigePreferenciasOpuestas) {
          const codigos = new Set(m.metricas.map((x) => x.preferencia?.codigo).filter(Boolean));
          expect(codigos.has('favor_de_candidata')).toBe(true);
          expect(codigos.has('favor_de_referencia')).toBe(true);
        }
      });

      it('BenchmarksView: vista previa sin contradicciones', () => {
        // `ComparisonResultCard` exige evaluación por contrato: **no hay vista previa
        // sin evaluación**. Los escenarios sin ella (histórica, malformada) no se pintan
        // por esta vía justamente para no inventar una: su superficie es el detalle.
        if (ev == null) {
          expect(ev).toBeNull();
          return;
        }
        const { container } = render(
          <ComparisonResultCard comparison={c} evaluacion={ev} />
        );
        const texto = textoDe(container);
        for (const p of esc.prohibido) expect(texto).not.toMatch(p);

        if (esc.exigePerdidaVisible) {
          // La tabla de detalle está plegada por defecto: se despliega como haría el
          // usuario. Sin desplegarla, la aserción pasaría por no haber mirado nada.
          fireEvent.click(screen.getByRole('button', { name: /Ver detalles técnicos|detalles/i }));
          // La métrica no juzgada sigue en la tabla, con su valor observado.
          const fila = filaDe(container, /0[.,]1 ?% Low|0[.,]1 % low/i);
          expect(fila).not.toBeNull();
          expect(textoDe(fila!)).toMatch(/40/);
        }
      });

      it('SavedComparisonDetail: lo guardado no se reinterpreta', () => {
        const { container } = render(
          <SavedComparisonDetail
            comparison={c}
            evaluacion={ev}
            onBack={() => {}}
            onDelete={() => {}}
            onCopy={() => {}}
          />
        );
        const texto = textoDe(container);
        for (const p of esc.prohibido) expect(texto).not.toMatch(p);

        if (esc.exigePerdidaVisible) {
          const fila = filaDe(container, /0[.,]1 ?% Low|0[.,]1 % low/i);
          expect(fila).not.toBeNull();
          expect(textoDe(fila!)).toMatch(/40/);
          // Y no lleva preferencia: el motor no la juzgó.
          expect(textoDe(fila!)).not.toMatch(/Favorece a/);
        }
      });

      it('DashboardView: el resumen de una guardada no contradice al modelo', () => {
        // El Dashboard no recibe props: lee la lista con `useSavedComparisons`, que aquí
        // se sustituye por una única guardada con el modelo del escenario.
        const guardada = {
          id: 'g1',
          saved_at: '2026-01-02 11:00:00',
          game_name: c.baseline.game_name,
          before_fps: c.baseline.avg_fps,
          after_fps: c.test.avg_fps,
          fps_change_percent: 0,
          comparison: c,
          evaluacion: ev,
          estado_de_la_evaluacion: (esc as { estadoDeclarado?: string }).estadoDeclarado,
        };
        guardadas.length = 0;
        guardadas.push(guardada as never);

        const { container } = render(<DashboardView />);
        const texto = textoDe(container);
        for (const p of esc.prohibido) expect(texto).not.toMatch(p);
      });
    });
  }
});
