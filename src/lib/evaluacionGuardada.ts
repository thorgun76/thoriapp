/**
 * Qué se puede mostrar de una comparación **guardada**.
 *
 * Regla de la que cuelga todo este módulo: una comparación guardada **no se interpreta**.
 * Si se guardó sin evaluación, su veredicto original no se registró y **no se puede
 * recuperar**: volver a llamar a `interpretar()` produciría una conclusión nueva con las
 * reglas de hoy y la presentaría como si fuera la de entonces. Eso es inventar historia.
 *
 * `interpretar()` sigue siendo el único intérprete para las comparaciones **no guardadas**
 * mientras viva en TypeScript (bloque C lo llevará a Rust).
 *
 * Este módulo **no decide** nada: traduce el estado que ya calculó el backend
 * (`estado_de_la_evaluacion` en Rust) a lo que la interfaz necesita. No hay aquí ningún
 * umbral, margen ni regla de comparación.
 */

import type { EvaluacionPersistida, EvaluacionDeMetrica, SavedComparison } from './api';
import type { BenchmarkComparison } from './types';

/** Los tres estados posibles de la evaluación de una comparación guardada. */
export type EstadoDeEvaluacionGuardada =
  /** No se registró evaluación: comparación histórica. */
  | 'no_registrada'
  /** Registrada y comprensible por esta versión. */
  | 'soportada'
  /** Registrada, pero con códigos o metodología que esta versión no entiende. */
  | 'no_soportada'
  /** Hay una evaluación, pero no se puede leer. No es lo mismo que no tenerla. */
  | 'malformada';

/**
 * Clasifica una evaluación guardada.
 *
 * Se hace aquí, y no llamando al backend por cada fila, porque el dato ya viene en la
 * comparación: pedirlo aparte sería una ida y vuelta por un dato que ya está en memoria.
 * La regla es la misma que `EvaluacionPersistida::es_soportada` en Rust; si cambia, cambia
 * en los dos lados.
 */
export function estadoDeEvaluacionGuardada(
  c: Pick<SavedComparison, 'evaluacion' | 'estado_de_la_evaluacion'>
): EstadoDeEvaluacionGuardada {
  // El backend ya clasificó: se usa tal cual. Rust es la fuente de verdad y aquí no se
  // reimplementa la regla.
  if (c.estado_de_la_evaluacion) return c.estado_de_la_evaluacion;

  const ev = c.evaluacion;
  if (ev == null) return 'no_registrada';

  // Respaldo para listas antiguas sin clasificación del backend.
  if (ev.metodologia_interpretacion !== 'v1') return 'no_soportada';

  // Un estado que esta versión no conoce tampoco: presentarlo como «mejora» sería
  // inventar un veredicto.
  return ESTADOS_CONOCIDOS.has(ev.estado) ? 'soportada' : 'no_soportada';
}

/** Estados que esta versión sabe presentar. Cualquier otro se conserva, sin traducir. */
const ESTADOS_CONOCIDOS = new Set([
  'mejora',
  'empeora',
  'mixto',
  'sin_cambio',
  'datos_insuficientes',
  'solo_descriptiva',
]);

/** ¿La evaluación describe de verdad a esta comparación? */
export function evaluacionCorrespondeA(
  ev: EvaluacionPersistida,
  c: BenchmarkComparison
): boolean {
  return ev.referencia?.id === c.baseline.id && ev.candidata?.id === c.test.id;
}

/**
 * Los datos que la cabecera puede enseñar sin interpretar nada.
 *
 * Solo magnitudes **observadas** y **deltas ya almacenados**: no se calcula un veredicto.
 * `diferenciaPct` sale de `fps_change_percent`, que se guardó con la comparación.
 */
export interface DatosObservados {
  fpsAntes: number | null;
  fpsDespues: number | null;
  diferenciaPct: number | null;
  /** Deltas almacenados, tal cual. `null` si no constan. */
  deltaLow1: number | null;
  deltaLow01: number | null;
  deltaFrameTime: number | null;
}

const num = (v: number | null | undefined): number | null =>
  v != null && Number.isFinite(v) ? v : null;

export function datosObservados(c: BenchmarkComparison): DatosObservados {
  return {
    fpsAntes: num(c.baseline?.avg_fps),
    fpsDespues: num(c.test?.avg_fps),
    diferenciaPct: num(c.fps_change_percent),
    deltaLow1: num(c.fps_1_percent_change_percent),
    deltaLow01: num(c.fps_0_1_percent_change_percent),
    deltaFrameTime: num(c.frame_time_change_percent),
  };
}

/**
 * Una métrica de una evaluación **soportada**, ya lista para pintar.
 *
 * Nada se recalcula: cada cifra viene del registro guardado. `direccion` y `preferencia`
 * se guardan por separado a propósito (subir FPS es mejor; subir frametime no), y aquí se
 * muestran tal cual.
 */
export interface MetricaGuardada {
  clave: string;
  antes: number | null;
  despues: number | null;
  diferencia: number | null;
  diferenciaPct: number | null;
  direccion: string;
  preferencia: string;
  confianza: string | null;
}

export function metricasGuardadas(ev: EvaluacionPersistida): MetricaGuardada[] {
  return (ev.metricas ?? []).map((m: EvaluacionDeMetrica) => ({
    clave: m.clave,
    antes: num(m.antes),
    despues: num(m.despues),
    diferencia: num(m.diferencia),
    diferenciaPct: num(m.diferencia_pct),
    direccion: m.direccion,
    preferencia: m.preferencia,
    confianza: m.confianza ?? null,
  }));
}

/**
 * ¿Una evaluación «soportada» dice que hubo datos insuficientes?
 *
 * Es importante distinguirlo de no tener evaluación: aquí **sí** hubo una evaluación y su
 * conclusión fue que no alcanzaban los datos. Son cosas distintas y se dicen distinto.
 */
export function esDatosInsuficientes(ev: EvaluacionPersistida): boolean {
  return ev.estado === 'datos_insuficientes';
}

/** ¿El veredicto guardado es descriptivo (no comparables las metodologías)? */
export function esSoloDescriptiva(ev: EvaluacionPersistida): boolean {
  return ev.estado === 'solo_descriptiva';
}

/**
 * Lo que la cabecera de una prueba guardada puede decir, sin interpretar nada.
 *
 * `titulo` es un **código**, no una frase: la redacción vive en la interfaz, de modo que
 * traducir o reescribir un texto no cambia lo que se guardó ni obliga a subir la versión
 * de metodología.
 */
export type VeredictoGuardado =
  | { tipo: 'no_registrada' }
  | { tipo: 'no_soportada'; motivo: string }
  | { tipo: 'malformada' }
  | { tipo: 'datos_insuficientes'; evaluacion: EvaluacionPersistida }
  | { tipo: 'solo_descriptiva'; evaluacion: EvaluacionPersistida }
  | { tipo: 'veredicto'; evaluacion: EvaluacionPersistida }

export function veredictoGuardado(
  c: Pick<SavedComparison, 'evaluacion' | 'estado_de_la_evaluacion'>
): VeredictoGuardado {
  const estado = estadoDeEvaluacionGuardada(c);
  if (estado === 'no_registrada') return { tipo: 'no_registrada' };
  if (estado === 'malformada') return { tipo: 'malformada' };
  const ev = c.evaluacion;
  if (ev == null) return { tipo: 'malformada' };
  if (estado === 'no_soportada') {
    return {
      tipo: 'no_soportada',
      motivo: ev.metodologia_interpretacion === 'v1'
        ? ev.estado
        : ev.metodologia_interpretacion,
    };
  }
  if (esDatosInsuficientes(ev)) return { tipo: 'datos_insuficientes', evaluacion: ev };
  if (esSoloDescriptiva(ev)) return { tipo: 'solo_descriptiva', evaluacion: ev };
  return { tipo: 'veredicto', evaluacion: ev };
}

/** Estados que producen una ganadora. `sin_cambio` y mixto no la tienen. */
export function ganadorDeEstado(estado: string): 'antes' | 'despues' | 'ninguno' {
  if (estado === 'mejora') return 'despues';
  if (estado === 'empeora') return 'antes';
  return 'ninguno';
}

/**
 * Vocabularios de la evaluación. Se muestran **código a código**: uno desconocido se pinta
 * crudo, nunca se traduce a uno conocido.
 *
 * El motivo es el mismo que en Rust: si un valor futuro se convirtiera en «mejora» por un
 * default, la interfaz daría un veredicto que el archivo no dice.
 */
const TEXTO_DIRECCION: Record<string, string> = {
  sube: 'sube',
  baja: 'baja',
  igual: 'igual',
  no_consta: 'no consta',
};

const TEXTO_PREFERENCIA: Record<string, string> = {
  favor_de_candidata: 'a favor de la nueva',
  favor_de_referencia: 'a favor de la anterior',
  indistinto: 'indistinto',
  no_establecida: 'sin preferencia establecida',
};

const TEXTO_COMPATIBILIDAD: Record<string, string> = {
  establecida: 'Metodologías comparables',
  no_establecida: 'Metodologías no comparables',
};

export const textoDeDireccion = (codigo: string): string => TEXTO_DIRECCION[codigo] ?? codigo;
export const textoDePreferencia = (codigo: string): string =>
  TEXTO_PREFERENCIA[codigo] ?? codigo;
export const textoDeCompatibilidad = (codigo: string): string =>
  TEXTO_COMPATIBILIDAD[codigo] ?? codigo;
