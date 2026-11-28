import { describe, expect, it } from 'vitest';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import { NOMBRE_ANTES, NOMBRE_DESPUES, summarizeComparison, tituloDeCabecera } from './BenchmarksView';
import { ANTES, DESPUES, etiquetasDe } from '../lib/etiquetas';
import { presentarEvaluacion, presentarSinEvaluacion } from '../lib/presentacionEvaluacion';
import type { EvaluacionPersistida } from '../lib/api';

/**
 * El veredicto tiene que responder a la pregunta que trae el usuario («¿cuál es mejor?»)
 * nombrando al ganador con los mismos nombres que ven las tarjetas.
 *
 * **Reescrito en C.3.** Antes este fichero llamaba a `interpretar()`, un intérprete de
 * frontend que deducía un veredicto mirando cifras. Ese módulo ya no existe: la única
 * fuente de veredicto es la evaluación de Rust, y lo que aquí se prueba es que la
 * presentación **copia** ese veredicto en vez de construirlo.
 *
 * Lo que se conserva de la versión anterior, porque sigue siendo una garantía de producto:
 * el titular nombra al ganador con el nombre de su tarjeta y nunca habla de «la grabación
 * nueva».
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

/**
 * Evaluación con la forma que produce Rust.
 *
 * Los campos que no importan para cada caso se rellenan con valores neutros: la
 * presentación no los deriva, los copia, así que no hace falta que sean coherentes con el
 * estado salvo en lo que cada prueba mira.
 */
function evaluacion(over: Partial<EvaluacionPersistida> = {}): EvaluacionPersistida {
  return {
    version: 1,
    metodologia: 'v1',
    metodologia_interpretacion: 'v1',
    estado: 'mejora',
    confianza: 'observada',
    referencia: { id: 'b', number: 1 },
    candidata: { id: 'b', number: 2 },
    condiciones: {
      margen_porcentaje: 5,
      compatibilidad_metodologica: 'comparable',
      ruido_referencia_porcentaje: 1,
      ruido_candidata_porcentaje: 1,
    },
    ruido_combinado_porcentaje: 2,
    motivos: [],
    limitaciones: [],
    metricas: [],
    ...over,
  } as EvaluacionPersistida;
}

const tituloDe = (ev: EvaluacionPersistida, c: BenchmarkComparison) =>
  tituloDeCabecera(presentarEvaluacion(ev, c, { estado: 'soportada' }), etiquetasDe(c));

describe('veredicto: dice quién gana', () => {
  it('cuando la evaluación dice mejora, gana «Después»', () => {
    const c = comparison({}, { avg_fps: 75, fps_1_percent_low: 55 });
    expect(tituloDe(evaluacion({ estado: 'mejora' }), c)).toBe(`Mejor con ${NOMBRE_DESPUES}`);
  });

  it('cuando la evaluación dice empeora, gana «Antes»', () => {
    const c = comparison({}, { avg_fps: 45, fps_1_percent_low: 30, fps_0_1_percent_low: 22 });
    expect(tituloDe(evaluacion({ estado: 'empeora' }), c)).toBe(`Mejor con ${NOMBRE_ANTES}`);
  });

  it('sin cambio evaluado no se inventa un ganador', () => {
    const c = comparison({}, { avg_fps: 60.5 });
    const modelo = presentarEvaluacion(evaluacion({ estado: 'sin_cambio' }), c, {
      estado: 'soportada',
    });
    expect(modelo.titulo).toBe('sin_cambio');
    expect(modelo.quienFavorece).toBeNull();
    expect(tituloDeCabecera(modelo)).toBe('Sin un cambio que se pueda sostener');
  });

  it('resultado mixto no tiene ganadora', () => {
    const c = comparison({}, { avg_fps: 80, fps_1_percent_low: 20 });
    const modelo = presentarEvaluacion(evaluacion({ estado: 'mixto' }), c, {
      estado: 'soportada',
    });
    expect(modelo.quienFavorece).toBeNull();
  });

  it('el titular nunca habla de «la grabación nueva»', () => {
    const casos: Array<[string, Partial<EvaluacionPersistida>]> = [
      ['mejora', { estado: 'mejora' }],
      ['empeora', { estado: 'empeora' }],
      ['mixto', { estado: 'mixto' }],
      ['sin_cambio', { estado: 'sin_cambio' }],
      ['datos_insuficientes', { estado: 'datos_insuficientes' }],
      ['solo_descriptiva', { estado: 'solo_descriptiva' }],
    ];
    const c = comparison({}, { avg_fps: 75, fps_1_percent_low: 55 });
    for (const [nombre, over] of casos) {
      const titulo = tituloDe(evaluacion(over), c);
      expect(titulo, nombre).not.toMatch(/grabación nueva|grabación anterior/i);
    }
  });

  it('los nombres que exporta BenchmarksView son los de las etiquetas', () => {
    // Si alguien renombra las tarjetas en un sitio y no en el otro, esto falla.
    expect(NOMBRE_ANTES).toBe(ANTES);
    expect(NOMBRE_DESPUES).toBe(DESPUES);
  });
});

/**
 * `summarizeComparison` es el resumen de una comparación **sin evaluación**.
 *
 * Antes delegaba en `interpretar()`. Ahora devuelve siempre la respuesta honesta: sin
 * evaluación no hay ganadora, así que `depende` — no un empate ni un ganador deducido.
 */
describe('summarizeComparison: sin evaluación no hay ganadora', () => {
  const casos: Array<[string, BenchmarkComparison]> = [
    ['sube los FPS', comparison({}, { avg_fps: 75, fps_1_percent_low: 55 })],
    ['baja los FPS', comparison({}, { avg_fps: 45, fps_1_percent_low: 30, fps_0_1_percent_low: 22 })],
    ['casi igual', comparison({}, { avg_fps: 60.5 })],
    ['mixto por tirones', comparison({}, { avg_fps: 80, fps_1_percent_low: 20, spikes_per_min: 12 })],
  ];

  it.each(casos)('%s: no nombra ganadora', (_n, c) => {
    expect(summarizeComparison(c).winner).toBe('depende');
  });

  it.each(casos)('%s: el titular dice que no hay evaluación', (_n, c) => {
    expect(summarizeComparison(c).title).toBe(tituloDeCabecera(presentarSinEvaluacion(c)));
    expect(summarizeComparison(c).title).toMatch(/sin evaluación registrada/i);
  });

  it('la forma del resumen no ha cambiado para quien lo consume', () => {
    const resumen = summarizeComparison(comparison({}, {}));
    expect(Object.keys(resumen).sort()).toEqual(['title', 'winner']);
  });
});

/**
 * Cobertura parcial: el cambio se afirma, la victoria global no [C.3, sexto defecto].
 *
 * Caso real del motor (FPS 100→120, 1 % low 80→96, 0,1 % low 60→30 no elegible):
 * `estado=mejora` con `evaluacion_parcial`. El titular no puede decir «Mejor con…».
 */
describe('cobertura parcial: no hay corona ni titular de ganadora', () => {
  const limitacionesParciales = [
    'no_elegible',
    'apoyo_insuficiente',
    'perdida_observada_sin_evaluar',
    'evaluacion_parcial',
  ];

  it('el titular no declara ganadora y dice que es parcial', () => {
    const c = comparison({ avg_fps: 100 }, { avg_fps: 120 });
    const ev = evaluacion({ estado: 'mejora', limitaciones: limitacionesParciales });
    const titulo = tituloDe(ev, c);
    expect(titulo).not.toContain('Mejor con');
    expect(titulo).toContain('solo en parte de las métricas');
  });

  it('`summarizeComparison` no corona con cobertura parcial', () => {
    const c = comparison({ avg_fps: 100 }, { avg_fps: 120 });
    // El resumen sin evaluación ya devuelve «depende»; el modelo parcial tampoco corona.
    const modelo = presentarEvaluacion(
      evaluacion({ estado: 'mejora', limitaciones: limitacionesParciales }),
      c,
      { estado: 'soportada' }
    );
    expect(modelo.quienFavorece).toBeNull();
    expect(summarizeComparison(c).winner).toBe('depende');
  });

  it('con cobertura completa el titular sí corona', () => {
    const c = comparison({ avg_fps: 100 }, { avg_fps: 120 });
    const ev = evaluacion({
      estado: 'mejora',
      limitaciones: [],
      metricas: [
        {
          clave: 'avg_fps',
          antes: 100,
          despues: 120,
          diferencia: 20,
          diferencia_pct: 20,
          direccion: 'sube',
          preferencia: 'favor_de_candidata',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
      ],
    } as Partial<EvaluacionPersistida>);
    expect(tituloDe(ev, c)).toBe(`Mejor con ${NOMBRE_DESPUES}`);
  });
});
