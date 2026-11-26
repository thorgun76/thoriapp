/**
 * Interpretación de una comparación guardada.
 *
 * Antes de este módulo, la pantalla de detalle decidía el resultado con `buildVerdict`
 * (que suma porcentajes ponderados) y en paralelo el backend escribía su propio
 * `conclusion`. Los dos podían decir cosas distintas. Aquí se calcula **una sola vez**
 * un objeto con todo lo que la interfaz necesita, y la interfaz solo lo pinta.
 *
 * ## Qué significa "fiable" aquí, y qué no
 *
 * `noise_percent` mide el **error de estimación de la media** dentro de cada grabación
 * (variación entre bloques de ~6 s, ×1,96). Sirve para descartar diferencias que son
 * ruido de muestreo, y **no** mide la variación entre partidas distintas: otro enemigo,
 * otra zona, la GPU más caliente. Por eso:
 *
 * - una diferencia mayor que el margen → «observada», no «confirmada»;
 * - una diferencia menor → «sin cambio concluyente»;
 * - «confirmada» solo la da comparar **grupos** de grabaciones (ver `stats.rs`), y esta
 *   pantalla no tiene esos datos cuando solo hay dos grabaciones.
 *
 * No hay ningún porcentaje de confianza inventado en estas estructuras: solo lo que
 * dicen los números guardados.
 */

import {
  estabilidadDe,
  puntosDeDiferencia,
  low01Reliability,
  type Low01Reliability,
} from './estabilidad';
import { tieThreshold, marginLabel } from './noise';
import type { Benchmark, BenchmarkComparison } from './types';
import { duracionEnSegundos } from './duracion';

/**
 * Nombres por defecto de las dos grabaciones.
 *
 * Se conservan para las comparaciones guardadas cuyas grabaciones son anteriores a la
 * numeración: no se les inventa un número. Cuando la grabación sí tiene número, la
 * interfaz pasa `Prueba N` en su lugar (ver `etiquetasDe`).
 */
export const ANTES = 'Antes';
export const DESPUES = 'Después';

/**
 * Etiquetas a usar para las dos grabaciones de una comparación.
 *
 * `baseline` es la de referencia y `test` la que se probó después. Se derivan del número
 * persistente de cada grabación, que no cambia nunca; si falta, se queda «Antes»/«Después»,
 * que es lo único honesto que se puede decir.
 */
export interface Etiquetas {
  antes: string;
  despues: string;
}

export function etiquetasDe(c: {
  baseline?: { number?: number | null } | null;
  test?: { number?: number | null } | null;
}): Etiquetas {
  const nombre = (n: number | null | undefined, defecto: string) =>
    n != null && Number.isFinite(n) && n > 0 ? `Prueba ${n}` : defecto;
  return {
    antes: nombre(c.baseline?.number, ANTES),
    despues: nombre(c.test?.number, DESPUES),
  };
}

/**
 * Estado del resultado. Cinco valores, no tres: un resultado mixto (una métrica mejora
 * y otra empeora) no es «empate» ni «mejora», y forzarlo a uno de los dos era el
 * problema original.
 */
export type EstadoResultado =
  | 'mejora'           // las dos familias de métricas favorecen al después
  | 'empeora'          // las dos favorecen al antes
  | 'mixto'            // una mejora y otra empeora
  | 'sin_cambio'       // todo dentro del margen
  | 'datos_insuficientes';

export type Tono = 'bien' | 'mal' | 'atencion' | 'neutro';

/** Qué se puede afirmar con lo que hay. */
export type Confianza =
  | 'provisional'      // una grabación por lado: es una pista
  | 'observada'        // la diferencia supera el margen de estimación
  | 'no_concluyente'   // la diferencia cabe dentro del margen
  | 'insuficiente';    // faltan datos

/** Cómo cambia una métrica concreta. */
export type CambioMetrica = 'mejor' | 'peor' | 'sin_cambio' | 'sin_datos';

export interface MetricaInterpretada {
  clave: 'avg_fps' | 'estabilidad' | 'low1' | 'low01';
  etiqueta: string;
  antes: number | null;
  despues: number | null;
  /** Diferencia absoluta (después − antes). */
  diferencia: number | null;
  /** Diferencia porcentual sobre |antes|, si tiene sentido. */
  diferenciaPct: number | null;
  /** Puntos, solo para el índice de estabilidad (0-100). */
  diferenciaPuntos: number | null;
  cambio: CambioMetrica;
  /** Texto corto para la interfaz, ya redactado. */
  resumen: string;
}

export interface Interpretacion {
  estado: EstadoResultado;
  confianza: Confianza;
  tono: Tono;
  titulo: string;
  /** Una frase en lenguaje natural que explica qué pasó. */
  explicacion: string;
  /** Etiqueta corta de confianza, tipo chip. */
  chipConfianza: string;
  metricas: MetricaInterpretada[];
  /** Avisos sobre la calidad de los datos. */
  avisos: string[];
  /** Margen de estimación combinado, si existe (%). */
  margen: number | null;
  /** Texto del margen (`±5.1%`). */
  margenTexto: string | null;
  low01: Low01Reliability;
  /** Quién gana, derivado del estado. */
  ganador: Ganador;
}

/* ─────────────────────────── Saneado de números ─────────────────────────── */

/** Un número utilizable, o `null`. Evita que NaN/∞ lleguen a la interfaz. */
const num = (v: number | null | undefined): number | null =>
  v != null && Number.isFinite(v) ? v : null;

/** FPS válidos: finitos y positivos. */
const fps = (v: number | null | undefined): number | null => {
  const n = num(v);
  return n != null && n > 0 ? n : null;
};

const pct = (antes: number | null, despues: number | null): number | null => {
  if (antes == null || despues == null || antes === 0) return null;
  return ((despues - antes) / Math.abs(antes)) * 100;
};

/* ─────────────────────── Comparación de una métrica ─────────────────────── */

/**
 * Decide si una métrica mejoró, empeoró o no se movió, usando el mismo margen que el
 * resto de la aplicación (`tieThreshold`). Nunca compara con un umbral propio: dos
 * umbrales distintos para lo mismo es cómo se acaba con dos veredictos que se
 * contradicen.
 */
function cambioDe(
  antes: number | null,
  despues: number | null,
  masEsMejor: boolean,
  margen: number
): CambioMetrica {
  if (antes == null || despues == null) return 'sin_datos';
  const cambio = pct(antes, despues);
  if (cambio == null) return 'sin_datos';
  // Un cambio exactamente igual al margen se queda del lado conservador: no se afirma.
  if (Math.abs(cambio) <= margen) return 'sin_cambio';
  const mejor = masEsMejor ? cambio > 0 : cambio < 0;
  return mejor ? 'mejor' : 'peor';
}

const textoCambio = (c: CambioMetrica): string =>
  c === 'mejor' ? 'mejor'
  : c === 'peor' ? 'peor'
  : c === 'sin_cambio' ? 'sin cambio concluyente'
  : 'sin datos';

/* ────────────────────────── Utilidades de redacción ────────────────────────── */

const f1 = (n: number) => n.toFixed(1);
/* ────────────────────────────── El intérprete ────────────────────────────── */

/** Etiquetas por defecto: «Antes»/«Después», para no romper a quien no las pase. */
export function interpretar(c: BenchmarkComparison, etiquetas?: Etiquetas): Interpretacion {
  const et = etiquetas ?? etiquetasDe(c);
  const antes: Benchmark = c.baseline;
  const despues: Benchmark = c.test;

  const margen = tieThreshold(c.noise_percent);
  const margenTexto = marginLabel(c.noise_percent);

  // ── Métricas ────────────────────────────────────────────────────────────────
  const avgAntes = fps(antes.avg_fps);
  const avgDespues = fps(despues.avg_fps);
  const estAntes = estabilidadDe(antes);
  const estDespues = estabilidadDe(despues);
  const low1Antes = fps(antes.fps_1_percent_low);
  const low1Despues = fps(despues.fps_1_percent_low);
  const low01Antes = fps(antes.fps_0_1_percent_low);
  const low01Despues = fps(despues.fps_0_1_percent_low);

  const cambioAvg = cambioDe(avgAntes, avgDespues, true, margen);
  const cambioLow1 = cambioDe(low1Antes, low1Despues, true, margen);
  const cambioLow01 = cambioDe(low01Antes, low01Despues, true, margen);
  // La estabilidad se compara con el mismo margen (%): es un cociente, así que un
  // ±5 % sobre el índice significa lo mismo que un ±5 % sobre los FPS.
  const cambioEst = cambioDe(estAntes, estDespues, true, margen);

  const diffAvg = avgAntes != null && avgDespues != null ? avgDespues - avgAntes : null;
  const ptsEst =
    estAntes != null && estDespues != null
      ? puntosDeDiferencia(estAntes, estDespues)
      : null;

  const metricas: MetricaInterpretada[] = [
    {
      clave: 'avg_fps',
      etiqueta: 'FPS medios',
      antes: avgAntes,
      despues: avgDespues,
      diferencia: diffAvg,
      diferenciaPct: pct(avgAntes, avgDespues),
      diferenciaPuntos: null,
      cambio: cambioAvg,
      resumen:
        diffAvg == null
          ? 'No hay FPS medios guardados en alguna grabación.'
          : `${diffAvg >= 0 ? '+' : ''}${f1(diffAvg)} FPS · ${textoCambio(cambioAvg)}`,
    },
    {
      clave: 'estabilidad',
      etiqueta: 'Estabilidad de FPS',
      antes: estAntes,
      despues: estDespues,
      diferencia: estAntes != null && estDespues != null ? estDespues - estAntes : null,
      diferenciaPct: null, // en un índice 0-100, el % engaña: se habla de puntos
      diferenciaPuntos: ptsEst,
      cambio: cambioEst,
      resumen:
        ptsEst == null
          ? 'No se puede calcular con los datos guardados.'
          : `${ptsEst >= 0 ? '+' : ''}${f1(ptsEst)} puntos · ${textoCambio(cambioEst)}`,
    },
    {
      clave: 'low1',
      etiqueta: '1 % low',
      antes: low1Antes,
      despues: low1Despues,
      diferencia: low1Antes != null && low1Despues != null ? low1Despues - low1Antes : null,
      diferenciaPct: pct(low1Antes, low1Despues),
      diferenciaPuntos: null,
      cambio: cambioLow1,
      resumen:
        low1Antes == null || low1Despues == null
          ? 'No guardado en estas grabaciones.'
          : `${low1Despues - low1Antes >= 0 ? '+' : ''}${f1(low1Despues - low1Antes)} FPS · ${textoCambio(cambioLow1)}`,
    },
    {
      clave: 'low01',
      etiqueta: '0,1 % low',
      antes: low01Antes,
      despues: low01Despues,
      diferencia:
        low01Antes != null && low01Despues != null ? low01Despues - low01Antes : null,
      diferenciaPct: pct(low01Antes, low01Despues),
      diferenciaPuntos: null,
      cambio: cambioLow01,
      resumen:
        low01Antes == null || low01Despues == null
          ? 'No guardado en estas grabaciones.'
          : `${low01Despues - low01Antes >= 0 ? '+' : ''}${f1(low01Despues - low01Antes)} FPS · ${textoCambio(cambioLow01)}`,
    },
  ];

  const low01 = low01Reliability(
    Math.min(antes.total_frames || 0, despues.total_frames || 0) || null
  );

  // ── Avisos de calidad de datos ──────────────────────────────────────────────
  const avisos: string[] = [];

  // La duración puede no constar (grabación sin eje temporal): no se convierte en 0.
  const durAntes = duracionEnSegundos(antes);
  const durDespues = duracionEnSegundos(despues);
  if (durAntes != null && durDespues != null && durAntes > 0) {
    const ratio = Math.abs(durAntes - durDespues) / durAntes;
    if (ratio > 0.3) {
      avisos.push(
        `Las grabaciones duran muy distinto (${f1(durAntes)} s y ${f1(durDespues)} s). Repite la prueba con la misma duración.`
      );
    }
    if (Math.min(durAntes, durDespues) < 20) {
      avisos.push(
        'Alguna grabación dura menos de 20 s: los valores extremos (1 % y 0,1 % low) son poco fiables.'
      );
    }
  }
  if (antes.warmup_seconds === 0 && despues.warmup_seconds === 0) {
    avisos.push(
      'Se midió también el arranque de las dos grabaciones. Si empezaste a grabar en el menú o la carga, esa parte ensucia el resultado.'
    );
  }
  if (low01.aviso) avisos.push(low01.aviso);
  if (antes.game_name && despues.game_name &&
      antes.game_name.trim().toLowerCase() !== despues.game_name.trim().toLowerCase()) {
    avisos.push('Las grabaciones son de juegos distintos: la comparación no es válida.');
  }
  if (!c.noise_percent) {
    avisos.push(
      'Estas grabaciones no guardan el margen de estimación (son de una versión anterior). Se usa el mínimo de la aplicación (±5 %).'
    );
  }

  // ── Estado y confianza ──────────────────────────────────────────────────────
  const conDatos = [cambioAvg, cambioLow1].filter((x) => x !== 'sin_datos');

  if (avgAntes == null || avgDespues == null || conDatos.length === 0) {
    return {
      estado: 'datos_insuficientes',
      confianza: 'insuficiente',
      tono: 'neutro',
      titulo: 'No hay datos suficientes',
      explicacion:
        'A estas grabaciones les faltan los FPS medios, así que no se puede decir si hubo mejora.',
      chipConfianza: 'Datos insuficientes',
      metricas,
      avisos,
      margen,
      margenTexto,
      low01,
      ganador: 'ninguno',
    };
  }

  // Familia "velocidad" = FPS medios. Familia "regularidad" = lows + estabilidad.
  const velocidad: CambioMetrica = cambioAvg;
  const regularidad: CambioMetrica[] = [cambioLow1, cambioLow01, cambioEst].filter(
    (x) => x === 'mejor' || x === 'peor'
  );

  const hayMejoraVelocidad = velocidad === 'mejor';
  const hayPerdidaVelocidad = velocidad === 'peor';
  const regMejora = regularidad.filter((x) => x === 'mejor').length;
  const regEmpeora = regularidad.filter((x) => x === 'peor').length;

  let estado: EstadoResultado;
  if (regularidad.length === 0 && velocidad === 'sin_cambio') {
    estado = 'sin_cambio';
  } else if (hayMejoraVelocidad && regEmpeora > regMejora) {
    estado = 'mixto';
  } else if (hayPerdidaVelocidad && regMejora > regEmpeora) {
    estado = 'mixto';
  } else if (
    (hayMejoraVelocidad && regEmpeora === 0) ||
    (regMejora > 0 && regEmpeora === 0 && velocidad !== 'peor')
  ) {
    estado = 'mejora';
  } else if (
    (hayPerdidaVelocidad && regMejora === 0) ||
    (regEmpeora > 0 && regMejora === 0 && velocidad !== 'mejor')
  ) {
    estado = 'empeora';
  } else if (regMejora > 0 && regEmpeora > 0) {
    estado = 'mixto';
  } else {
    estado = 'sin_cambio';
  }

  // Confianza: una grabación por lado nunca confirma nada.
  const confianza: Confianza =
    estado === 'sin_cambio' ? 'no_concluyente' : 'provisional';

  // ── Textos ──────────────────────────────────────────────────────────────────
  const margenTxt = margenTexto ?? `±${Math.round(margen)} %`;
  const avgCambio = pct(avgAntes, avgDespues);

  // Frase de la velocidad, con el dato exacto.
  const fraseVelocidad =
    velocidad === 'sin_cambio'
      ? `Los FPS medios quedan prácticamente iguales (${f1(avgAntes)} → ${f1(avgDespues)}, ${margenTxt} de margen).`
      : velocidad === 'mejor'
      ? `Los FPS medios suben de ${f1(avgAntes)} a ${f1(avgDespues)}${avgCambio != null ? ` (${avgCambio >= 0 ? '+' : ''}${f1(avgCambio)} %)` : ''}.`
      : `Los FPS medios bajan de ${f1(avgAntes)} a ${f1(avgDespues)}${avgCambio != null ? ` (${f1(avgCambio)} %)` : ''}.`;

  // Frase de la regularidad, solo si hay algo que decir.
  const regMejores = [cambioLow1 === 'mejor' && 'el 1 % low',
                      cambioLow01 === 'mejor' && 'el 0,1 % low',
                      cambioEst === 'mejor' && 'la estabilidad'].filter(Boolean) as string[];
  const regPeores = [cambioLow1 === 'peor' && 'el 1 % low',
                     cambioLow01 === 'peor' && 'el 0,1 % low',
                     cambioEst === 'peor' && 'la estabilidad'].filter(Boolean) as string[];

  const listar = (xs: string[]) =>
    xs.length <= 1 ? xs.join('') : `${xs.slice(0, -1).join(', ')} y ${xs[xs.length - 1]}`;

  const fraseRegularidad =
    regMejores.length > 0 && regPeores.length === 0
      ? `Mejora también ${listar(regMejores)}.`
      : regPeores.length > 0 && regMejores.length === 0
      ? `Empeora ${listar(regPeores)}.`
      : regMejores.length > 0 && regPeores.length > 0
      ? `Mejora ${listar(regMejores)} pero empeora ${listar(regPeores)}.`
      : 'La regularidad no cambia.';

  const explicacion = `${fraseVelocidad} ${fraseRegularidad}`;

  let titulo: string;
  let tono: Tono;

  switch (estado) {
    case 'mejora':
      titulo = `Gana «${et.despues}»`;
      tono = 'bien';
      break;
    case 'empeora':
      titulo = `Gana «${et.antes}»`;
      tono = 'mal';
      break;
    case 'mixto':
      titulo = 'Resultado mixto';
      tono = 'atencion';
      break;
    case 'sin_cambio':
      titulo = 'Sin diferencia concluyente';
      tono = 'neutro';
      break;
    default:
      titulo = 'No hay datos suficientes';
      tono = 'neutro';
      break;
  }

  // El resultado de una sola sesión nunca es una confirmación. Se dice en el chip.
  const chipConfianza =
    confianza === 'provisional' ? 'Resultado provisional'
    : confianza === 'no_concluyente' ? 'Dentro del margen estimado'
    : confianza === 'insuficiente' ? 'Datos insuficientes'
    : 'Diferencia observada';

  // El aviso sobre la fiabilidad del 0,1 % low NO se añade aquí: la vista de detalle
  // tiene su propia caja técnica donde explicarlo una sola vez. Repetirlo arriba, junto al
  // veredicto, duplicaba el mismo aviso en la misma pantalla.

  return {
    estado,
    confianza,
    tono,
    titulo,
    explicacion,
    chipConfianza,
    metricas,
    avisos,
    margen,
    margenTexto,
    low01,
    ganador: ganadorDe(estado),
  };
}

/**
 * Quién gana, en términos de las dos grabaciones. Derivado **del estado**, no calculado
 * aparte: era la causa de que el badge dijera una cosa y el titular otra.
 */
export type Ganador = 'antes' | 'despues' | 'ninguno' | 'mixto';

/**
 * Titular del resultado, redactado sobre **el cambio**, no sobre quién gana.
 *
 * El titular anterior («Gana “Prueba 1”») se pintaba con el color del resultado, así que
 * un empeoramiento salía en rojo con pulgar abajo y el nombre de la prueba ganadora al
 * lado: se leía como «la Prueba 1 es la mala», justo al revés. Aquí el titular dice qué
 * pasó con la segunda prueba y la ganadora se menciona aparte, con la corona.
 *
 * Solo cambia la redacción: el estado sale de `interpretar()`, con los mismos criterios.
 */
export function titularDelCambio(
  r: Pick<Interpretacion, 'estado'>,
  et: Etiquetas
): string {
  switch (r.estado) {
    case 'mejora':
      return `${et.despues} rinde mejor`;
    case 'empeora':
      return `${et.despues} rinde peor`;
    case 'mixto':
      return 'Resultado mixto';
    case 'sin_cambio':
      return 'Sin diferencia concluyente';
    default:
      return 'No hay datos suficientes';
  }
}

/**
 * Qué grabación obtuvo el mejor resultado **observado**, según el intérprete.
 *
 * `null` cuando no hay ganadora clara (mixto, sin cambio, datos insuficientes). Nunca se
 * decide por los FPS medios: más promedio con peores 1 % lows no es mejor, y por eso manda
 * `ganador` y no una comparación de `avg_fps`.
 */
export function mejorObservada(
  r: Pick<Interpretacion, 'ganador'>,
  et: Etiquetas
): { lado: 'antes' | 'despues'; nombre: string } | null {
  if (r.ganador === 'antes') return { lado: 'antes', nombre: et.antes };
  if (r.ganador === 'despues') return { lado: 'despues', nombre: et.despues };
  return null;
}

export function ganadorDe(estado: EstadoResultado): Ganador {
  switch (estado) {
    case 'mejora':
      return 'despues';
    case 'empeora':
      return 'antes';
    case 'mixto':
      return 'mixto';
    default:
      return 'ninguno';
  }
}

/** Nombre de juego legible: `7DaysToDie` → `7 Days to Die` es imposible en general. */
export function nombreLegible(nombre: string | null | undefined): string {
  const n = (nombre ?? '').trim();
  if (!n) return 'Juego desconocido';
  // Solo se limpian los sufijos técnicos más comunes; no se intenta adivinar espacios.
  return n
    .replace(/\.(x86_64|x86|exe|bin|sh)$/i, '')
    .replace(/-Win64-Shipping$/i, '')
    .trim() || n;
}
