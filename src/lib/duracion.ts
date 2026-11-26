/**
 * Qué significa la duración de una grabación.
 *
 * **Este es el único sitio donde se decide.** La regla es la misma que
 * `Benchmark::duracion` en Rust; si cambia, cambia en los dos lados.
 *
 * Existe porque hay dos campos (`duration_seconds` y `duracion_medida`) y el primero
 * vale `0` por compatibilidad cuando no hay reloj fiable. Leer ese cero como «cero
 * segundos de grabación» sería inventar un dato: una grabación sin eje temporal **no
 * dura cero**, simplemente **no consta** cuánto dura.
 */

import type { Benchmark } from './types';

/** De dónde sale —o por qué no se conoce— la duración. */
export type DuracionDeLaGrabacion =
  /** Anterior a `metodologia_calculo`: `duration_seconds` es lo medido entonces. */
  | { tipo: 'historica'; segundos: number }
  /** v2 con reloj fiable: duración medida. */
  | { tipo: 'medida'; segundos: number }
  /** No consta. **No** es «cero segundos». */
  | { tipo: 'desconocida' };

type CamposDeDuracion = Pick<Benchmark, 'duration_seconds' | 'duracion_medida' | 'metodologia_calculo'>;

function utilizable(v: number | null | undefined): v is number {
  return typeof v === 'number' && Number.isFinite(v) && v > 0;
}

/**
 * Duración de la grabación, con su procedencia.
 *
 * Orden de decisión (idéntico a Rust):
 *
 * 1. Si hay `duracion_medida` utilizable, manda ella.
 * 2. Si la grabación declara metodología v2 y no hay `duracion_medida`, la duración es
 *    **desconocida**: no se cae a `duration_seconds`, que en v2 vale 0 justo cuando no
 *    había eje.
 * 3. Si no declara metodología (histórica), `duration_seconds` es el valor medido
 *    entonces. Un valor no utilizable (≤0, no finito) sigue siendo desconocido.
 */
export function duracionDe(bench: CamposDeDuracion): DuracionDeLaGrabacion {
  if (utilizable(bench.duracion_medida)) {
    return { tipo: 'medida', segundos: bench.duracion_medida };
  }
  if (bench.metodologia_calculo != null) {
    return { tipo: 'desconocida' };
  }
  if (utilizable(bench.duration_seconds)) {
    return { tipo: 'historica', segundos: bench.duration_seconds };
  }
  return { tipo: 'desconocida' };
}

/**
 * Segundos de grabación **solo si constan**. `null` = no consta.
 *
 * Úsalo en lugar de leer `duration_seconds` directamente. Para mostrar, prefiere
 * `textoDeDuracion`, que además distingue los casos.
 */
export function duracionEnSegundos(bench: CamposDeDuracion): number | null {
  const d = duracionDe(bench);
  return d.tipo === 'desconocida' ? null : d.segundos;
}

/** ¿Se puede afirmar que la grabación dura menos de `segundos`? No si no consta. */
export function duraMenosDe(bench: CamposDeDuracion, segundos: number): boolean {
  const d = duracionEnSegundos(bench);
  return d != null && d < segundos;
}

/**
 * Texto para la interfaz. Cuando no consta **lo dice**, no escribe «0 s».
 */
export function textoDeDuracion(bench: CamposDeDuracion, formatear: (s: number) => string): string {
  const d = duracionEnSegundos(bench);
  return d == null ? 'duración no registrada' : formatear(d);
}
