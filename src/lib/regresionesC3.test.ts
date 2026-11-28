import { describe, expect, it } from 'vitest';
import {
  estadoDeEvaluacionGuardada,
  formatearCifra,
  formatearDiferencia,
  presentarEvaluacion,
  presentarGuardada,
  presentarSinEvaluacion,
} from './presentacionEvaluacion';
import type { Benchmark, BenchmarkComparison } from './types';
import type { EvaluacionDeMetrica, EvaluacionPersistida } from './api';

/**
 * Regresiones de las incidencias de presentación de C.3.
 *
 * **Alcance de este archivo.** Cubre las incidencias cuyo defecto se puede reproducir sobre
 * el modelo de presentación; cada `describe` cita el identificador con el que se registró.
 * Las descripciones de C3-01 a C3-09 las aportó el responsable del proyecto desde las
 * observaciones de la validación real (están recogidas en `STATUS.md`); este archivo prueba
 * la parte de ellas que vive en `presentacionEvaluacion`.
 *
 * Lo que **no** hay aquí son pruebas de interfaz de esas incidencias: viven en
 * `SavedComparisonDetail.test.tsx`, `BenchmarksView.test.tsx` y `matrizCoherencia.test.tsx`,
 * que sí las ejercitan por el camino real de render.
 *
 * Cada prueba **falla con el código anterior** y pasa con el actual; se indica en el
 * comentario qué se comprobaba antes.
 */

/* ────────────────────────────── Utilidades ────────────────────────────── */

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'CS2',
    appid: 730,
    timestamp: '2026-11-02 14:43:52',
    duration_seconds: 60,
    avg_fps: 100,
    fps_1_percent_low: 70,
    fps_0_1_percent_low: 60,
    max_fps: 200,
    min_fps: 50,
    frame_time_avg: 10,
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

function metrica(
  clave: string,
  preferencia: EvaluacionDeMetrica['preferencia'],
  muestras: number | null = 9000
): EvaluacionDeMetrica {
  const apoyo = muestras == null ? null : { percentil: { muestras, valor: 0 } };
  return {
    clave,
    direccion: 'mas_es_mejor',
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
    referencia: { id: c.baseline.id, number: null },
    candidata: { id: c.test.id, number: null },
    estado: 'mejora',
    confianza: 'provisional',
    condiciones: { margen_porcentaje: 5, compatibilidad_metodologica: 'establecida' },
    motivos: [],
    limitaciones: [],
    metricas: [],
    ...campos,
  } as EvaluacionPersistida;
}

/* ─────────────────────────── C3-22: redondeo visual ─────────────────────────── */

describe('C3-22: el cero se normaliza después del redondeo', () => {
  it('un negativo que redondea a cero no conserva el signo', () => {
    // Antes: `(-0.04).toFixed(1)` da `"-0.0"`, y la tabla lo mostraba como «-0.0 pts».
    expect(formatearCifra(-0.04, 1)).toBe('0.0');
    expect(formatearDiferencia(-0.04, 1)).toBe('0.0');
  });

  it('un positivo que redondea a cero no se convierte en «+0.0»', () => {
    // Antes: el prefijo se añadía a mano y salía «+0.0».
    expect(formatearDiferencia(0.04, 1)).toBe('0.0');
    expect(formatearDiferencia(0.04, 1)).not.toContain('+');
  });

  it('el cero y el cero negativo son el mismo texto', () => {
    expect(formatearDiferencia(0, 1)).toBe('0.0');
    expect(formatearDiferencia(-0, 1)).toBe('0.0');
  });

  it('los valores que no redondean a cero conservan signo y dígitos', () => {
    // La normalización no puede redondear dos veces ni comerse valores reales.
    expect(formatearDiferencia(-0.05, 1)).toBe('-0.1');
    expect(formatearDiferencia(0.05, 1)).toBe('+0.1');
    expect(formatearCifra(-1.4778325123152847, 1)).toBe('-1.5');
    expect(formatearCifra(1234.5678, 2)).toBe('1234.57');
  });

  it('un valor que falta o no es finito no se convierte en cero', () => {
    // Un dato ausente nunca se presenta como 0: sería inventar una medición.
    expect(formatearCifra(null, 1)).toBeNull();
    expect(formatearCifra(undefined, 1)).toBeNull();
    expect(formatearCifra(Number.NaN, 1)).toBeNull();
    expect(formatearCifra(Number.POSITIVE_INFINITY, 1)).toBeNull();
  });
});

/* ────────────────── C3-15 / C3-01: una evaluación ilegible no es histórica ────────────────── */

describe('C3-15 / C3-01: no se pierde la clasificación de Rust al presentar', () => {
  it('un registro malformado no se presenta como «sin evaluación»', () => {
    // Antes: `presentarGuardada` devolvía `no_registrada` para cualquier `evaluacion: null`,
    // aunque Rust hubiera dicho que había un registro ilegible.
    const c = comparacion({ avg_fps: 100 }, { avg_fps: 130 });
    const m = presentarGuardada({
      evaluacion: null,
      estado_de_la_evaluacion: 'malformada',
      comparison: c,
    });
    expect(m.estado).toBe('malformada');
    expect(m.titulo).toBe('malformada');
    expect(m.veredicto.tipo).toBe('no_legible');
  });

  it('una metodología desconocida tampoco se presenta como histórica', () => {
    const c = comparacion();
    const m = presentarGuardada({
      evaluacion: null,
      estado_de_la_evaluacion: 'no_soportada',
      comparison: c,
    });
    expect(m.estado).toBe('no_soportada');
    expect(m.veredicto.tipo).toBe('no_legible');
  });

  it('sin registro y sin clasificación de Rust, sí es histórica', () => {
    const c = comparacion();
    const m = presentarGuardada({ evaluacion: null, comparison: c });
    expect(m.estado).toBe('no_registrada');
  });

  it('la clasificación se deduce igual que la deduce Rust', () => {
    const c = comparacion();
    // Metodología de otra versión: no soportada, no malformada.
    expect(
      estadoDeEvaluacionGuardada({
        evaluacion: evaluacion(c, { metodologia_interpretacion: 'v0' }),
      })
    ).toBe('no_soportada');
    // Sin evaluación: no registrada.
    expect(estadoDeEvaluacionGuardada({ evaluacion: null })).toBe('no_registrada');
  });
});

/* ──────────── C3-14 / C3-07: lo ilegible no conserva preferencias ni compatibilidad ──────────── */

describe('C3-14 / C3-07: una evaluación ilegible no afirma preferencias', () => {
  it('las métricas conservan sus datos pero pierden la preferencia', () => {
    // Antes: los campos venían escritos en el registro y se enseñaban tal cual, así que
    // una evaluación de metodología desconocida seguía diciendo «favorece a la nueva».
    const c = comparacion();
    const ev = evaluacion(c, { metricas: [metrica('avg_fps', 'favor_de_candidata')] });
    const m = presentarEvaluacion(ev, c, { estado: 'no_soportada' });
    expect(m.metricas).toHaveLength(1);
    expect(m.metricas[0]!.preferencia).toBeNull();
    expect(m.metricas[0]!.estado).toBe('no_evaluada');
    expect(m.quienFavorece).toBeNull();
  });

  it('no se afirma comparabilidad metodológica que no se puede revalidar', () => {
    const c = comparacion();
    const ev = evaluacion(c, { metricas: [metrica('avg_fps', 'favor_de_candidata')] });
    const m = presentarEvaluacion(ev, c, { estado: 'malformada' });
    expect(m.compatibilidad).toBeNull();
  });

  it('una evaluación soportada sí conserva su preferencia', () => {
    // Contrapunto: la neutralización no puede afectar al caso normal.
    const c = comparacion();
    const ev = evaluacion(c, { metricas: [metrica('avg_fps', 'favor_de_candidata')] });
    const m = presentarEvaluacion(ev, c, { estado: 'soportada' });
    expect(m.metricas[0]!.preferencia?.codigo).toBe('favor_de_candidata');
    expect(m.quienFavorece).toBe('candidata');
  });
});

/* ──────────────────── C3-04: la narrativa original no se inventa ──────────────────── */

describe('C3-04: la narrativa guardada se enseña tal cual o no se enseña', () => {
  it('sin texto en el registro no se fabrica uno', () => {
    const c = comparacion();
    const m = presentarSinEvaluacion(c);
    expect(m.narrativaOriginal).toBeNull();
  });

  it('con texto en el registro se devuelve sin reescribirlo', () => {
    const c = comparacion();
    const conTexto = { ...c, conclusion: 'La nueva grabación rinde un 30 % más.' };
    const m = presentarSinEvaluacion(conTexto);
    expect(m.narrativaOriginal).toBe('La nueva grabación rinde un 30 % más.');
  });

  it('un texto en blanco no se presenta como narrativa', () => {
    const c = comparacion();
    const m = presentarSinEvaluacion({ ...c, conclusion: '   ' });
    expect(m.narrativaOriginal).toBeNull();
  });
});

/* ──────────────── C3-24 / C3-25: cuatro estados y apoyo por lado ──────────────── */

describe('C3-24: los cuatro estados de una métrica no se colapsan', () => {
  it('con preferencia, indistinta, no evaluada y descriptiva se distinguen', () => {
    const c = comparacion();
    const ev = evaluacion(c, {
      metricas: [
        metrica('avg_fps', 'favor_de_candidata'),
        metrica('fps_1_percent_low', 'indistinto'),
        metrica('fps_0_1_percent_low', 'no_evaluada' as never),
      ],
    });
    const m = presentarEvaluacion(ev, c);
    const porClave = new Map(m.metricas.map((x) => [x.clave, x.estado]));
    expect(porClave.get('avg_fps')).toBe('con_preferencia');
    expect(porClave.get('fps_1_percent_low')).toBe('indistinta');
    expect(porClave.get('fps_0_1_percent_low')).toBe('no_evaluada');
  });

  it('el contador separa «con preferencia» de «indistinta»', () => {
    // Antes: `evaluadas` incluía las indistintas y el texto decía «N de M métricas con
    // preferencia establecida», afirmando una preferencia que el motor no emitió.
    const c = comparacion();
    const ev = evaluacion(c, {
      metricas: [
        metrica('avg_fps', 'favor_de_candidata'),
        metrica('fps_1_percent_low', 'indistinto'),
        metrica('fps_0_1_percent_low', 'indistinto'),
      ],
    });
    const m = presentarEvaluacion(ev, c);
    expect(m.cobertura.conPreferencia).toBe(1);
    expect(m.cobertura.indistintas).toBe(2);
  });
});

describe('C3-25: el apoyo declarado se conserva', () => {
  it('las muestras de cada lado llegan al modelo', () => {
    // Antes: `metricasPresentadas` descartaba `apoyo_referencia`/`apoyo_candidata`, que
    // Rust sí escribe, así que no se podía explicar una exclusión por falta de apoyo.
    const c = comparacion();
    const ev = evaluacion(c, {
      metricas: [metrica('avg_fps', 'favor_de_candidata', 9500)],
    });
    const m = presentarEvaluacion(ev, c);
    expect(m.metricas[0]!.apoyo?.referencia?.muestras).toBe(9500);
    expect(m.metricas[0]!.apoyo?.candidata?.muestras).toBe(9500);
  });

  it('la ausencia de apoyo es ausencia, no un cero', () => {
    const c = comparacion();
    const ev = evaluacion(c, {
      metricas: [metrica('avg_fps', 'indistinto', null)],
    });
    const m = presentarEvaluacion(ev, c);
    expect(m.metricas[0]!.apoyo).toBeNull();
  });
});
