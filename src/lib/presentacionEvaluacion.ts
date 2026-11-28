/**
 * El **único** contrato de presentación de una comparación individual.
 *
 * Lo usan `BenchmarksView`, `SavedComparisonDetail` y `DashboardView`. No hay otro lugar
 * donde se decida qué se enseña de una evaluación: si una vista quiere saber si la nueva
 * configuración favorece a alguien, lo pregunta aquí, y aquí se contesta copiando
 * `estado` de Rust, nunca mirando el signo de una cifra.
 *
 * Reglas que la firma de este módulo hace imposibles de incumplir:
 *
 *  - **Preferencia solo desde `estado`**: `quienFavorece` es `null` salvo que el motor
 *    haya dicho `mejora` o `empeora`. Una evaluación parcial (`mixto`), descriptiva o sin
 *    datos no tiene ganadora, por mucho que alguna métrica mejore.
 *  - **Magnitud, dirección y preferencia son campos distintos**. `MetricaPresentada` no
 *    tiene `mejor`/`peor`/`esMejora`: quien pinte no puede deducir un veredicto de un
 *    número porque el número no está en la forma que produce veredictos.
 *  - **Sin umbrales aquí**. Ni márgenes, ni percentiles, ni ruido combinado se recalculan:
 *    llegan ya decididos en `evaluacion.condiciones`.
 *  - **Ruido combinado ≠ margen aplicado**: se presentan como dos datos distintos, porque
 *    el margen es `max(5 %, ruido)` y confundirlos hace creer que 3 % de ruido basta para
 *    afirmar algo.
 *  - **Estabilidad es descriptiva** y solo puede venir de los datos medidos: Rust no la
 *    evalúa, así que no puede tener preferencia ni veredicto.
 *
 * Nada de este módulo escribe: la lectura de una comparación histórica no toca el disco.
 */

import type {
  ApoyoDeMetrica,
  CodigoDeEvaluacion,
  EvaluacionDeMetrica,
  EvaluacionPersistida,
  EstadoDeLaEvaluacionLeida,
  SavedComparison,
} from './api';
import type { BenchmarkComparison } from './types';
import { estabilidad, estabilidadLabel } from './estabilidad';

/**
 * Los cuatro estados de una evaluación registrada, tal como los clasifica Rust.
 *
 * Se nombran explícitamente porque son cuatro situaciones distintas y confundirlas tiene
 * consecuencias:
 *
 *  - `no_registrada`: la comparación es **histórica** (anterior al motor). Sus cifras se
 *    enseñan, pero no hay veredicto y no se genera uno.
 *  - `soportada`: hay evaluación y esta versión la entiende.
 *  - `no_soportada`: hay evaluación, pero de una metodología o con códigos que esta
 *    versión no sabe leer. Los datos se conservan y se enseña **el código crudo**.
 *  - `malformada`: hay un registro que no se puede interpretar. No es «sin evaluación».
 */
export type EstadoDeEvaluacionGuardada = EstadoDeLaEvaluacionLeida;

/** De dónde sale lo que se presenta. */
export type OrigenDePresentacion =
  /** Evaluación pedida al motor ahora mismo (vista previa o recién guardada). */
  | 'evaluada'
  /** Registro persistido, leído tal cual. */
  | 'persistida';

/** Código que se enseña tal cual si esta versión no lo conoce. */
export interface EtiquetaDeCodigo {
  codigo: string;
  /** Texto cuando el código es conocido; si no, `null`. */
  texto: string | null;
  /** `false` ⇒ no se reconoce y debe pintarse el código crudo. */
  conocido: boolean;
}

function etiquetaDeCodigo(
  codigo: CodigoDeEvaluacion | null | undefined,
  vocabulario: Readonly<Record<string, string>>
): EtiquetaDeCodigo | null {
  if (codigo == null || codigo === '') return null;
  const texto = vocabulario[codigo];
  return texto == null
    ? { codigo, texto: null, conocido: false }
    : { codigo, texto, conocido: true };
}

/**
 * Tono de una etiqueta. Solo expresa **si la interfaz puede afirmar algo**, no si el
 * resultado es bueno: `neutro` es el tono por defecto y el único que admite una métrica.
 */
export type Tono = 'neutro' | 'positivo' | 'negativo' | 'aviso';

/**
 * En qué situación está una métrica, decidido **una vez** aquí.
 *
 * Las cuatro son excluyentes y no se deducen del color ni del signo:
 *
 *  - `con_preferencia`: el motor la juzgó y favorece a alguien.
 *  - `indistinta`: el motor la juzgó y la diferencia quedó dentro del margen. **Participó**,
 *    pero no favorece a nadie. No es lo mismo que no haberla juzgado.
 *  - `no_evaluada`: el motor no emitió preferencia (apoyo insuficiente, métrica excluida).
 *  - `descriptiva`: la métrica no forma parte del contrato de evaluación (estabilidad).
 *    Es un dato medido, no un juicio.
 */
export type EstadoDeLaMetrica =
  | 'con_preferencia'
  | 'indistinta'
  | 'no_evaluada'
  | 'descriptiva';

/**
 * Apoyo declarado por el motor para cada lado, tal cual, sin interpretar.
 *
 * Solo se transportan las cifras que el motor escribió. El juicio sobre si sostienen la
 * preferencia ya está hecho: viene en `preferencia`. Aquí solo se dice **qué se midió**,
 * para poder explicar una exclusión sin inventar la causa.
 */
export interface ApoyoPresentado {
  referencia: { muestras: number } | null;
  candidata: { muestras: number } | null;
  /**
   * Qué lados **no** traen dato. La ausencia es ausencia: si el motor no lo dijo, no se
   * reparte la culpa entre referencia y candidata.
   */
  ausente: { referencia: boolean; candidata: boolean };
}

/**
 * Una métrica, ya presentada.
 *
 * Ojo: **no hay campo `mejor` ni `peor`**. El color no puede venir de aquí; viene de
 * `preferencia`, que solo existe cuando el motor la estableció, y aun así es la preferencia
 * *de esta métrica*, no el veredicto de la comparación.
 */
export interface MetricaPresentada {
  clave: string;
  nombre: EtiquetaDeCodigo;
  antes: number | null;
  despues: number | null;
  diferencia: number | null;
  diferenciaPct: number | null;
  /** `sube` | `baja` | `igual` | `no_consta` — qué hizo el número, no si es bueno. */
  direccion: EtiquetaDeCodigo | null;
  /** Preferencia **establecida por el motor** para esta métrica. */
  preferencia: EtiquetaDeCodigo | null;
  /** Situación de la métrica, decidida una vez a partir de los datos de Rust. */
  estado: EstadoDeLaMetrica;
  /** Apoyo por lado que declaró el motor, o `null` si no declaró ninguno. */
  apoyo: ApoyoPresentado | null;
  /** Confianza de esta métrica, si el motor la registró. */
  confianza: EtiquetaDeCodigo | null;
  motivos: EtiquetaDeCodigo[];
  limitaciones: EtiquetaDeCodigo[];
}

/**
 * A: por qué no hay una evaluación aprovechable, en cuatro casos que **no** son el mismo.
 *
 * Un solo booleano («interpretable / no interpretable») colapsaba la evaluación ausente con
 * la incompatible, y acababa afirmando que «no se puede interpretar» algo que nunca se
 * registró. Distinguirlos es una cuestión de procedencia, no de redacción:
 *
 * - `soportada`        — hay evaluación y esta versión la entiende.
 * - `no_soportada`     — hay evaluación, pero de una metodología que esta versión no sabe leer.
 * - `malformada`       — hay un registro de evaluación, pero no se puede leer.
 * - `ausente`          — **no hay** evaluación registrada. No es que no se lea: no existe.
 */
export type EstadoDeLectura = 'soportada' | 'no_soportada' | 'malformada' | 'ausente';

/** Sólo `soportada` permite afirmar nada sobre el contenido de la evaluación. */
export function esLegible(e: EstadoDeLectura): boolean {
  return e === 'soportada';
}

/**
 * C3-25: la métrica concreta a la que afecta una limitación global, con el apoyo que el
 * motor declaró para cada lado.
 *
 * La limitación global dice *qué pasó* («no pudo juzgar esta métrica»), pero no *a cuál*.
 * El contrato sí lo trae: cada métrica lleva sus propias `limitaciones`, y el apoyo por
 * lado viene en `apoyo_referencia` / `apoyo_candidata`. Nombrar la métrica convierte una
 * frase genérica en una explicación que se puede comprobar en la tabla.
 */
export interface MetricaAfectada {
  clave: string;
  nombre: string;
  motivos: EtiquetaDeCodigo[];
  /** Apoyo por lado **tal como lo registró el motor**; `null` cuando no consta. */
  apoyoReferencia: { muestras: number } | null;
  apoyoCandidata: { muestras: number } | null;
}

/** Cobertura de la evaluación: cuántas métricas se pudieron juzgar de cuántas. */
export interface CoberturaDeLaEvaluacion {
  /**
   * Métricas que **entraron en la decisión** del estado.
   *
   * Incluye `indistinto` (dentro del margen): una métrica elegible que se juzgó como
   * indistinta participó, aunque no favorezca a ningún lado. Contar solo las que
   * favorecen a alguien haría parecer que decidieron menos métricas de las que decidieron.
   */
  evaluadas: number;
  /** Métricas presentes en la evaluación (incluidas las que no se pudieron juzgar). */
  totales: number;
  /** `true` si el motor declara `evaluacion_parcial`: decidió solo una parte. */
  parcial: boolean;
  /**
   * Métricas que el motor **favorecieron a alguien**. Se cuenta aparte de `evaluadas`
   * porque una métrica indistinta participó sin favorecer a nadie: decir «3 de 3 con
   * preferencia establecida» cuando las tres son indistintas es falso.
   */
  conPreferencia: number;
  /** Métricas que el motor juzgó indistintas (dentro del margen). */
  indistintas: number;
  /** `true` si el motor declara `perdida_observada_sin_evaluar`. */
  perdidaEnMetricaExcluida: boolean;
  /** `true` si el motor declara `cobertura_insuficiente_para_conclusion_global`. */
  coberturaInsuficienteParaConclusion: boolean;
}

/** Datos medidos que se enseñan siempre, sin veredicto. */
export interface DatosDescriptivos {
  fpsAntes: number | null;
  fpsDespues: number | null;
  diferenciaFpsPct: number | null;
  deltaLow1Pct: number | null;
  deltaLow01Pct: number | null;
  deltaFrameTimePct: number | null;
  /**
   * Índices de estabilidad de cada grabación, calculados **fuera del motor** a partir de
   * lo medido (FPS medios y percentiles). Son descriptivos: no se evalúan, no se
   * persisten y no dan preferencia. Rust no tiene clave de estabilidad.
   */
  estabilidad: {
    referencia: { indice: number; etiqueta: EtiquetaDeCodigo } | null;
    candidata: { indice: number; etiqueta: EtiquetaDeCodigo } | null;
  } | null;
}

/**
 * Qué se puede afirmar de la comparación en conjunto.
 *
 * Es el campo que sustituye al veredicto de `interpretar()`: una sola fuente, la de Rust.
 */
export type VeredictoPresentado =
  | { tipo: 'sin_evaluacion'; motivo: 'historica' }
  | { tipo: 'no_legible'; estado: 'no_soportada' | 'malformada'; detalle: EtiquetaDeCodigo | null }
  | { tipo: 'sin_veredicto'; motivo: 'datos_insuficientes' | 'solo_descriptiva' }
  | { tipo: 'sin_ganadora'; motivo: 'mixto' | 'sin_cambio' }
  /**
   * El motor vio una mejora/empeora, pero **decidió con parte** de las métricas: se afirma
   * el cambio en lo evaluado y se niega la victoria sobre el conjunto.
   */
  | {
      tipo: 'cambio_parcial';
      favor: 'candidata' | 'referencia';
      estado: EtiquetaDeCodigo;
    }
  | { tipo: 'ganadora'; favor: 'candidata' | 'referencia'; estado: EtiquetaDeCodigo };

/** El modelo que consumen las tres vistas. */
export interface ModeloDePresentacion {
  origen: OrigenDePresentacion;
  estado: EstadoDeEvaluacionGuardada;
  tono: Tono;
  /** Código de cabecera; la redacción vive en la interfaz. */
  titulo:
    | 'historica'
    | 'no_soportada'
    | 'malformada'
    | 'datos_insuficientes'
    | 'solo_descriptiva'
    | 'mixto'
    | 'sin_cambio'
    | 'mejora'
    | 'empeora';
  veredicto: VeredictoPresentado;
  /**
   * A quién favorece **la comparación**, no una métrica.
   *
   * `null` siempre que no haya ganadora. Que una métrica suba no lo rellena nunca.
   */
  quienFavorece: 'candidata' | 'referencia' | null;
  confianza: EtiquetaDeCodigo | null;
  estadoEvaluado: EtiquetaDeCodigo | null;
  cobertura: CoberturaDeLaEvaluacion;
  motivos: EtiquetaDeCodigo[];
  limitaciones: EtiquetaDeCodigo[];
  /** A: por qué la evaluación no es aprovechable, distinguiendo ausencia de ilegibilidad. */
  estadoDeLectura: EstadoDeLectura;
  /** C3-25: qué métricas concretas motivaron esas limitaciones globales. */
  metricasAfectadas: MetricaAfectada[];
  /**
   * El margen que el motor **aplicó** para afirmar (%), y el ruido combinado del que se
   * partió. Son dos datos distintos a propósito.
   */
  margenAplicado: number | null;
  ruidoCombinado: number | null;
  compatibilidad: EtiquetaDeCodigo | null;
  variableEvaluada: EtiquetaDeCodigo | null;
  metricas: MetricaPresentada[];
  descriptivas: DatosDescriptivos;
  /**
   * La narrativa que el motor escribió **al medir**, guardada con la comparación.
   *
   * No es un veredicto reconstruido: es el texto original de entonces, y se enseña tal
   * cual en las comparaciones históricas, que no tienen evaluación registrada. `null`
   * cuando el registro no la trae (nunca se rellena con una inventada).
   */
  narrativaOriginal: string | null;
}

// ─────────────────────────── vocabularios ───────────────────────────

const ESTADOS_CONOCIDOS = new Set([
  'mejora',
  'empeora',
  'mixto',
  'sin_cambio',
  'datos_insuficientes',
  'solo_descriptiva',
]);

const NOMBRE_METRICA: Record<string, string> = {
  avg_fps: 'FPS medios',
  fps_1_percent_low: '1 % low',
  fps_0_1_percent_low: '0,1 % low',
};

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

/**
 * Los motivos y limitaciones que el motor puede emitir hoy. Uno desconocido **no** se
 * traduce: se enseña su código, porque traducirlo a uno conocido sería inventar el motivo.
 */
const TEXTO_DETALLE: Record<string, string> = {
  datos_insuficientes: 'no hay datos suficientes',
  sin_metricas: 'no hay métricas evaluables',
  metrica_no_evaluada: 'métrica sin preferencia establecida',
  sin_apoyo_suficiente: 'las muestras no sostienen una preferencia',
  comandos_distintos: 'los comandos de las dos pruebas no coinciden',
  metodologias_no_comparables: 'las metodologías no son comparables',
  cambio_dentro_del_margen: 'el cambio está dentro del margen',
  grabacion_corta: 'alguna grabación es corta',
  sin_variable_declarada: 'no se declaró la variable',
  valor_no_consta: 'hay un valor que no consta',
  // Elegibilidad: por qué una métrica con magnitud no decide.
  no_elegible: 'no pudo juzgar esta métrica: no reúne las condiciones para decidir',
  apoyo_insuficiente: 'las muestras no bastan para sostener una preferencia',
  apoyo_no_consta: 'no consta cuántas muestras hay, así que no se puede decidir',
  // Cobertura de la decisión.
  evaluacion_parcial: 'la evaluación solo juzgó parte de las métricas',
  perdida_observada_sin_evaluar:
    'hay una pérdida observada en una métrica que no se pudo juzgar',
  cobertura_insuficiente_para_conclusion_global:
    'una sola métrica no sostiene una conclusión sobre todo el rendimiento',
  // Valores.
  sin_datos: 'falta el valor en alguna de las dos grabaciones',
  valor_no_valido: 'hay un valor que no es un número utilizable',
  sin_metricas_medibles: 'ninguna métrica tiene un valor utilizable',
  sin_margen: 'no se aplicó margen: la diferencia no se juzgó',
  compatibilidad_no_establecida: 'las metodologías no están establecidas como comparables',
  dentro_del_margen: 'la diferencia entra dentro del margen',
  favorece_candidata: 'favorece a la grabación nueva',
  favorece_referencia: 'favorece a la grabación anterior',
  participo: 'cuenta en la decisión',
};

/**
 * Códigos de confianza **de Rust** (`evaluacion.rs`).
 *
 * No son `alta/media/baja`: la confianza describe *qué sostiene* el estado, no cuánta
 * certeza da. Un código que no esté aquí se enseña crudo, nunca se traduce a algo
 * parecido, porque «provisional» y «baja» no significan lo mismo.
 */
const TEXTO_CONFIANZA: Record<string, string> = {
  provisional: 'Provisional',
  observada: 'Observado',
  no_concluyente: 'No concluyente',
  insuficiente: 'Insuficiente',
};

const TEXTO_ESTADO: Record<string, string> = {
  mejora: 'mejora',
  empeora: 'empeora',
  mixto: 'mixto',
  sin_cambio: 'sin cambio',
  datos_insuficientes: 'datos insuficientes',
  solo_descriptiva: 'solo descriptiva',
};

const etiquetaEstado = (c: string) => etiquetaDeCodigo(c, TEXTO_ESTADO);
const etiquetaMetrica = (c: string) => etiquetaDeCodigo(c, NOMBRE_METRICA) ?? {
  codigo: c,
  texto: null,
  conocido: false,
};
const etiquetaDetalle = (c: string) => etiquetaDeCodigo(c, TEXTO_DETALLE) ?? {
  codigo: c,
  texto: null,
  conocido: false,
};

// ─────────────────────────── estados ───────────────────────────

/**
 * Clasifica una comparación guardada.
 *
 * Se usa la clasificación **de Rust** cuando viene (`estado_de_la_evaluacion`); el
 * respaldo es solo para listas antiguas sin ese campo. Rust es la fuente de verdad y la
 * regla no se reimplementa aquí.
 *
 * El respaldo distingue **no soportada** de **malformada** a propósito: son cosas distintas
 * y confundirlas oculta un archivo corrupto o miente sobre su procedencia. Un registro al
 * que le faltan los campos que toda evaluación tiene no es de otra versión: está roto.
 */
export function estadoDeEvaluacionGuardada(
  c: Pick<SavedComparison, 'evaluacion' | 'estado_de_la_evaluacion'>
): EstadoDeEvaluacionGuardada {
  if (c.estado_de_la_evaluacion) return c.estado_de_la_evaluacion;

  const ev = c.evaluacion;
  if (ev == null) return 'no_registrada';
  // Metodología de otra versión: se conserva y se dice, sin inventar un veredicto.
  if (ev.metodologia_interpretacion !== 'v1') return 'no_soportada';
  // La evaluación se escribió con esta versión, así que le faltan campos que debería
  // tener: no es «de otra versión», es ilegible.
  if (!esEvaluacionCompleta(ev)) return 'malformada';
  return ESTADOS_CONOCIDOS.has(ev.estado) ? 'soportada' : 'no_soportada';
}

/**
 * ¿Tiene un registro los campos que toda evaluación de `v1` escribe?
 *
 * No valida el contenido (eso es del motor): solo comprueba que la forma está completa.
 * Se usa para no presentar como «soportada» un registro truncado.
 */
function esEvaluacionCompleta(ev: EvaluacionPersistida): boolean {
  return (
    typeof ev.estado === 'string' &&
    ev.estado !== '' &&
    typeof ev.confianza === 'string' &&
    ev.confianza !== '' &&
    ev.referencia != null &&
    ev.candidata != null &&
    ev.condiciones != null
  );
}

const num = (v: number | null | undefined): number | null =>
  v != null && Number.isFinite(v) ? v : null;

/**
 * Cifra formateada para leer, sin artefactos de redondeo.
 *
 * El defecto que esto corrige (C3-22) no es el cero negativo original: `(-0).toFixed(1)`
 * ya da `"0.0"`. Sale con negativos pequeños que **redondean** a cero —`(-0.04).toFixed(1)`
 * es `"-0.0"`— y con positivos pequeños, que tras un prefijo `+` quedarían como `"+0.0"`.
 *
 * Se normaliza la cadena que produce `toFixed`, no el número de entrada: doble redondeo
 * sobre el valor (redondear y luego volver a formatear) desviaría cifras reales. Aquí el
 * único redondeo es el de `toFixed`; después solo se reescribe un cero con signo.
 *
 * Un valor ausente o no finito no se convierte en cero: devuelve `null` y la vista pinta
 * una raya, porque un cero es un dato y la falta de dato no lo es.
 */
export function formatearCifra(
  v: number | null | undefined,
  decimales = 1
): string | null {
  const n = num(v);
  if (n == null) return null;
  const texto = n.toFixed(decimales);
  // `-0.0`, `-0.00`, `+0.0`… son cero con signo: el signo no significa nada ahí.
  return /^[-+]?0(\.0+)?$/.test(texto) ? (0).toFixed(decimales) : texto;
}

/**
 * Cifra con signo explícito, para una diferencia.
 *
 * Un cero —también el que resulta de redondear— no lleva signo: `-0 pts` y `+0.0 %`
 * afirman una dirección que el número no tiene.
 */
export function formatearDiferencia(
  v: number | null | undefined,
  decimales = 1,
  sufijo = ''
): string | null {
  const texto = formatearCifra(v, decimales);
  if (texto == null) return null;
  const esCero = Number(texto) === 0;
  return `${esCero ? '' : Number(v) > 0 ? '+' : ''}${texto}${sufijo}`;
}

/**
 * Los datos medidos, **sin** veredicto.
 *
 * `estabilidad` es `null` si no se le pasan las dos grabaciones (por ejemplo, cuando solo
 * se tiene la comparación en el borrador de la vista). Se calcula sobre lo medido, no
 * sobre la evaluación.
 */
export function datosDescriptivos(c: BenchmarkComparison): DatosDescriptivos {
  return {
    fpsAntes: num(c.baseline?.avg_fps),
    fpsDespues: num(c.test?.avg_fps),
    diferenciaFpsPct: num(c.fps_change_percent),
    deltaLow1Pct: num(c.fps_1_percent_change_percent),
    deltaLow01Pct: num(c.fps_0_1_percent_change_percent),
    deltaFrameTimePct: num(c.frame_time_change_percent),
    estabilidad: estabilidadDeCadaGrabacion(c),
  };
}

/**
 * Índice de estabilidad de cada grabación, si sus tres cifras constan.
 *
 * Se calcula sobre **lo medido** (`estabilidad.ts`), no sobre la evaluación: el motor no
 * lo evalúa y no existe clave de estabilidad en el registro. Por eso no puede haber aquí
 * preferencia ni veredicto, solo una descripción de la forma de la distribución.
 */
function estabilidadDeCadaGrabacion(c: BenchmarkComparison) {
  const de = (b: BenchmarkComparison['baseline']) => {
    const indice = estabilidad(b?.avg_fps, b?.fps_1_percent_low, b?.fps_0_1_percent_low);
    if (indice == null) return null;
    return {
      indice,
      etiqueta: {
        codigo: String(indice),
        texto: estabilidadLabel(indice),
        conocido: true,
      } as EtiquetaDeCodigo,
    };
  };
  const referencia = de(c.baseline);
  const candidata = de(c.test);
  if (referencia == null && candidata == null) return null;
  return { referencia, candidata };
}

function cobertura(ev: EvaluacionPersistida | null): CoberturaDeLaEvaluacion {
  const metricas = ev?.metricas ?? [];
  // Una métrica participó si el motor se lo anotó (`participo`) o si su preferencia existe.
  // `indistinto` cuenta: dentro del margen **sí** hay juicio. Lo que no cuenta es
  // `no_establecida`, que es exactamente «no decidió».
  const evaluadas = metricas.filter(
    (m) =>
      m.preferencia === 'favor_de_candidata' ||
      m.preferencia === 'favor_de_referencia' ||
      m.preferencia === 'indistinto'
  ).length;

  // Estas dos se cuentan aparte porque `evaluadas` las mezcla a propósito. Decir «N de M
  // con preferencia establecida» usando `evaluadas` afirmaría una preferencia que tres
  // métricas indistintas no tienen.
  const conPreferencia = metricas.filter(
    (m) => m.preferencia === 'favor_de_candidata' || m.preferencia === 'favor_de_referencia'
  ).length;
  const indistintas = metricas.filter((m) => m.preferencia === 'indistinto').length;

  // Los códigos los pone el motor. No se recalculan umbrales en TypeScript.
  const codigos = new Set<string>(ev?.limitaciones ?? []);
  return {
    evaluadas,
    conPreferencia,
    indistintas,
    totales: metricas.length,
    parcial: codigos.has('evaluacion_parcial'),
    perdidaEnMetricaExcluida: codigos.has('perdida_observada_sin_evaluar'),
    coberturaInsuficienteParaConclusion: codigos.has(
      'cobertura_insuficiente_para_conclusion_global'
    ),
  };
}

/**
 * Situación de una métrica, a partir de la preferencia **que declaró Rust**.
 *
 * No interviene ningún umbral: si el motor no escribió preferencia, la métrica no está
 * evaluada, y punto. La única distinción que se hace es entre las tres formas de
 * `PreferenciaDeMetrica`, que son datos, no cálculos.
 */
function estadoDeLaMetrica(m: EvaluacionDeMetrica): EstadoDeLaMetrica {
  if (m.preferencia === 'favor_de_candidata' || m.preferencia === 'favor_de_referencia') {
    return 'con_preferencia';
  }
  if (m.preferencia === 'indistinto') return 'indistinta';
  return 'no_evaluada';
}

/**
 * Apoyo por lado, tal como lo declaró el motor.
 *
 * `null` cuando no declaró ninguno de los dos: sin datos no se dice quién se quedó corto,
 * porque atribuirlo sería inventar el motivo.
 */
function apoyoPresentado(m: EvaluacionDeMetrica): ApoyoPresentado | null {
  const de = (a: ApoyoDeMetrica | null | undefined) => {
    const n = a?.percentil?.muestras;
    return typeof n === 'number' && Number.isFinite(n) ? { muestras: n } : null;
  };
  const referencia = de(m.apoyo_referencia);
  const candidata = de(m.apoyo_candidata);
  if (referencia == null && candidata == null) return null;
  return {
    referencia,
    candidata,
    ausente: { referencia: referencia == null, candidata: candidata == null },
  };
}

function metricasPresentadas(ev: EvaluacionPersistida): MetricaPresentada[] {
  return (ev.metricas ?? []).map((m: EvaluacionDeMetrica) => ({
    clave: m.clave,
    nombre: etiquetaMetrica(m.clave),
    antes: num(m.antes),
    despues: num(m.despues),
    diferencia: num(m.diferencia),
    diferenciaPct: num(m.diferencia_pct),
    direccion: etiquetaDeCodigo(m.direccion, TEXTO_DIRECCION),
    preferencia: etiquetaDeCodigo(m.preferencia, TEXTO_PREFERENCIA),
    estado: estadoDeLaMetrica(m),
    apoyo: apoyoPresentado(m),
    confianza: etiquetaDeCodigo(m.confianza ?? null, TEXTO_CONFIANZA),
    motivos: (m.motivos ?? []).map(etiquetaDetalle),
    limitaciones: (m.limitaciones ?? []).map(etiquetaDetalle),
  }));
}

/**
 * Narrativa original de la comparación, la que el motor escribió al medir.
 *
 * Se lee del registro **tal cual**. Nunca se sintetiza: una comparación sin `conclusion`
 * (campo vacío o ausente en registros muy antiguos) no recibe una inventada, porque
 * atribuirle a una prueba un texto que no tuvo es reconstruir su historia.
 */
function narrativaDe(c: Pick<BenchmarkComparison, 'conclusion'> | null | undefined): string | null {
  const t = c?.conclusion;
  return typeof t === 'string' && t.trim() !== '' ? t : null;
}

/**
 * Retira la **afirmación** de preferencia de cada métrica, conservando sus datos medidos.
 *
 * Se usa cuando la evaluación no es legible para esta versión. Los valores observados
 * (`antes`, `despues`, `diferencia`) siguen ahí y se pueden enseñar: lo que no se puede
 * enseñar es que el motor prefiera una cosa ni que la otra, porque esas reglas son las
 * que esta versión desconoce.
 */
function neutralizarMetricas(metricas: MetricaPresentada[]): MetricaPresentada[] {
  return metricas.map((m) => ({
    ...m,
    preferencia: null,
    estado: 'no_evaluada',
    apoyo: null,
    confianza: null,
  }));
}

export function presentarEvaluacion(
  ev: EvaluacionPersistida,
  comparacion: BenchmarkComparison,
  opciones: {
    origen?: OrigenDePresentacion;
    estado?: EstadoDeEvaluacionGuardada;
  } = {}
): ModeloDePresentacion {
  const estado = opciones.estado ?? 'soportada';
  const descriptivas = datosDescriptivos(comparacion);
  const base = {
    origen: opciones.origen ?? 'persistida',
    estado,
    cobertura: cobertura(ev),
    // Texto que el motor escribió al medir, guardado con la comparación. No se fabrica:
    // si el registro no lo trae, es `null` y la vista no enseña narrativa.
    narrativaOriginal: narrativaDe(comparacion),
    metricas: metricasPresentadas(ev),
    descriptivas,
    margenAplicado: num(ev.condiciones?.margen_porcentaje),
    ruidoCombinado: num(ev.condiciones?.ruido_combinado_porcentaje),
    compatibilidad: etiquetaDeCodigo(
      ev.condiciones?.compatibilidad_metodologica,
      TEXTO_COMPATIBILIDAD
    ),
    variableEvaluada: ev.condiciones?.variable_evaluada
      ? etiquetaDetalle(ev.condiciones.variable_evaluada.codigo)
      : null,
    confianza: etiquetaDeCodigo(ev.confianza, TEXTO_CONFIANZA),
    estadoEvaluado: etiquetaEstado(ev.estado),
    motivos: (ev.motivos ?? []).map(etiquetaDetalle),
    limitaciones: (ev.limitaciones ?? []).map(etiquetaDetalle),
    estadoDeLectura: 'soportada' as EstadoDeLectura,
    // C3-25: solo las métricas que el motor marcó con alguna limitación. Si no marcó
    // ninguna, la lista va vacía: no se deduce a quién afecta una limitación global.
    metricasAfectadas: (ev.metricas ?? [])
      .filter((m) => (m.limitaciones ?? []).length > 0)
      .map((m) => {
        const apoyo = apoyoPresentado(m);
        return {
          clave: m.clave,
          nombre: textoDeEtiqueta(etiquetaMetrica(m.clave)),
          motivos: (m.limitaciones ?? []).map(etiquetaDetalle),
          apoyoReferencia: apoyo?.referencia ?? null,
          apoyoCandidata: apoyo?.candidata ?? null,
        };
      }),
  } as const;

  // No legible: no se afirma nada, y el código crudo viaja para que se pueda enseñar.
  //
  // C3-14 / C3-07: una evaluación que esta versión no entiende **no puede seguir
  // mostrando preferencias ni afirmar comparabilidad**, aunque esos campos vengan
  // escritos en el registro. Enseñar `favor_de_candidata` de una metodología desconocida
  // sería presentar como vigente un juicio cuyas reglas no conocemos, y `compatible`
  // afirmaría una comparabilidad que no podemos revalidar. Los datos se conservan en
  // disco: lo que se retira es la **afirmación**, no el dato.
  if (estado === 'no_soportada' || estado === 'malformada') {
    return {
      ...base,
      tono: 'aviso',
      titulo: estado === 'malformada' ? 'malformada' : 'no_soportada',
      veredicto: { tipo: 'no_legible', estado, detalle: etiquetaEstado(ev.estado) },
      quienFavorece: null,
      // Sin preferencia, sin apoyo y sin comparabilidad: no hay base para afirmarlas.
      metricas: neutralizarMetricas(base.metricas),
      compatibilidad: null,
      confianza: null,
      estadoEvaluado: null,
      // C3-07/C3-14/C3-17: el margen y el ruido son **criterios de la metodología**. Los
      // de una v9 no los sabe aplicar esta versión, así que no se presentan como juicio
      // vigente. El valor sigue en disco; lo que se retira es su condición de criterio
      // interpretado. Un «Margen aplicado: ±5 %» bajo un veredicto de «no soportada»
      // afirmaba que esta versión lo había aplicado.
      margenAplicado: null,
      ruidoCombinado: null,
      estadoDeLectura: estado as EstadoDeLectura,
    };
  }

  // Un estado que esta versión no sabe leer tampoco autoriza un veredicto: se presenta
  // como no soportado, con su código, en vez de caer en un default.
  if (!ESTADOS_CONOCIDOS.has(ev.estado)) {
    return {
      ...base,
      tono: 'aviso',
      titulo: 'no_soportada',
      veredicto: { tipo: 'no_legible', estado: 'no_soportada', detalle: etiquetaEstado(ev.estado) },
      quienFavorece: null,
      metricas: neutralizarMetricas(base.metricas),
      compatibilidad: null,
      confianza: null,
      estadoEvaluado: null,
      margenAplicado: null,
      ruidoCombinado: null,
      estadoDeLectura: 'no_soportada' as EstadoDeLectura,
    };
  }

  if (ev.estado === 'datos_insuficientes') {
    return {
      ...base,
      tono: 'aviso',
      titulo: 'datos_insuficientes',
      veredicto: { tipo: 'sin_veredicto', motivo: 'datos_insuficientes' },
      quienFavorece: null,
    };
  }

  if (ev.estado === 'solo_descriptiva') {
    return {
      ...base,
      tono: 'neutro',
      titulo: 'solo_descriptiva',
      veredicto: { tipo: 'sin_veredicto', motivo: 'solo_descriptiva' },
      quienFavorece: null,
    };
  }

  // `mixto` y `sin_cambio` son resultados reales y **no** son una ganadora: uno dice que
  // las métricas van en sentidos distintos, el otro que no hay cambio que sostener. Ninguno
  // se convierte en equivalencia.
  if (ev.estado === 'mixto' || ev.estado === 'sin_cambio') {
    return {
      ...base,
      tono: 'neutro',
      titulo: ev.estado,
      veredicto: { tipo: 'sin_ganadora', motivo: ev.estado },
      quienFavorece: null,
    };
  }

  // Ganadora: el motor vio un cambio sostenido. Pero solo se corona si la decisión cubrió
  // **todas** las métricas decisorias. Con `evaluacion_parcial` o cobertura insuficiente el
  // motor sí afirma el cambio —en lo que miró—, y eso se presenta; lo que no se puede
  // presentar es una victoria global sobre una experiencia que se midió a medias.
  const favor = ev.estado === 'mejora' ? 'candidata' : 'referencia';
  const coberturaDeLaDecision = base.cobertura;
  const globalAlcanzada =
    !coberturaDeLaDecision.parcial &&
    !coberturaDeLaDecision.coberturaInsuficienteParaConclusion;

  // El color sigue al cambio observado en lo evaluado: es un hecho del motor. Lo que cambia
  // es que sin cobertura completa no hay corona ni titular de ganadora.
  const tono = favor === 'candidata' ? 'positivo' : 'negativo';

  if (!globalAlcanzada) {
    return {
      ...base,
      tono: 'neutro',
      // El titular nombra el cambio observado **en lo evaluado**, nunca una victoria.
      titulo: favor === 'candidata' ? 'mejora' : 'empeora',
      veredicto: { tipo: 'cambio_parcial', favor, estado: etiquetaEstado(ev.estado)! },
      quienFavorece: null,
    };
  }

  return {
    ...base,
    tono,
    titulo: favor === 'candidata' ? 'mejora' : 'empeora',
    veredicto: { tipo: 'ganadora', favor, estado: etiquetaEstado(ev.estado)! },
    quienFavorece: favor,
  };
}

/**
 * El modelo de una comparación **histórica**: sin evaluación y sin veredicto.
 *
 * Las cifras se enseñan porque se midieron; lo que no se hace es construir una narrativa
 * nueva. Por eso no hay `titulo` de veredicto ni `quienFavorece`: lo único que se puede
 * decir es que se midió esto y entonces no se registró una evaluación.
 */
export function presentarSinEvaluacion(
  comparacion: BenchmarkComparison,
  opciones: {
    origen?: OrigenDePresentacion;
  } = {}
): ModeloDePresentacion {
  return {
    origen: opciones.origen ?? 'persistida',
    estado: 'no_registrada',
    tono: 'neutro',
    titulo: 'historica',
    veredicto: { tipo: 'sin_evaluacion', motivo: 'historica' },
    quienFavorece: null,
    confianza: null,
    estadoEvaluado: null,
    cobertura: {
      evaluadas: 0,
      conPreferencia: 0,
      indistintas: 0,
      totales: 0,
      parcial: false,
      perdidaEnMetricaExcluida: false,
      coberturaInsuficienteParaConclusion: false,
    },
    motivos: [],
    limitaciones: [],
    margenAplicado: null,
    ruidoCombinado: null,
    compatibilidad: null,
    variableEvaluada: null,
    metricas: [],
    descriptivas: datosDescriptivos(comparacion),
    // Lo único que una comparación histórica **sí** tiene de su evaluación: la narrativa
    // que Rust escribió al medirla. Se conserva y se enseña; no se sustituye por una nueva.
    narrativaOriginal: narrativaDe(comparacion),
    // A: no hay evaluación registrada. Ausente ≠ ilegible.
    estadoDeLectura: 'ausente' as EstadoDeLectura,
    // Sin evaluación no hay limitaciones por métrica que nombrar.
    metricasAfectadas: [],
  };
}

/**
 * Presenta una comparación **guardada**, sea cual sea su estado.
 *
 * Es la única entrada que usa el listado y el detalle. Un registro `malformada` no lanza
 * excepción: devuelve un modelo `malformada` que la interfaz pinta como aviso, para que un
 * archivo estropeado no tumbe la lista entera.
 */
export function presentarGuardada(
  c: Pick<SavedComparison, 'evaluacion' | 'estado_de_la_evaluacion' | 'comparison'>
): ModeloDePresentacion {
  const estado = estadoDeEvaluacionGuardada(c);
  const comparacion = c.comparison;

  // C3-15 / C3-01: nunca se llama al motor desde aquí, pero **tampoco se pierde la
  // clasificación de Rust**. Rust distingue «no hay evaluación» de «hay un registro que no
  // se puede leer» (`evaluacion_ilegible`): en el segundo caso `evaluacion` es `null` y aun
  // así la comparación **no es histórica**. Presentarla como tal borraría que existió una
  // evaluación, que es justo lo que el registro dice.
  if (c.evaluacion == null) {
    const sin = presentarSinEvaluacion(comparacion);
    if (estado === 'malformada') {
      return {
        ...sin,
        estado: 'malformada',
        tono: 'aviso',
        titulo: 'malformada',
        veredicto: { tipo: 'no_legible', estado: 'malformada', detalle: null },
        estadoDeLectura: 'malformada' as EstadoDeLectura,
      };
    }
    if (estado === 'no_soportada') {
      return {
        ...sin,
        estado: 'no_soportada',
        tono: 'aviso',
        titulo: 'no_soportada',
        veredicto: { tipo: 'no_legible', estado: 'no_soportada', detalle: null },
        estadoDeLectura: 'no_soportada' as EstadoDeLectura,
      };
    }
    return sin;
  }

  return presentarEvaluacion(c.evaluacion, comparacion, { estado });
}

// ─────────────────────────── compatibilidad ───────────────────────────

/**
 * Vocabulario de direcciones y preferencias, para las vistas que ya lo usaban.
 *
 * Se conserva porque `SavedComparisonDetail` pinta código a código; el respaldo es el
 * propio código, nunca una traducción inventada.
 */
export const textoDeDireccion = (codigo: string): string => TEXTO_DIRECCION[codigo] ?? codigo;
export const textoDePreferencia = (codigo: string): string =>
  TEXTO_PREFERENCIA[codigo] ?? codigo;
export const textoDeCompatibilidad = (codigo: string): string =>
  TEXTO_COMPATIBILIDAD[codigo] ?? codigo;

/**
 * C: ¿la evaluación de este registro se puede interpretar?
 *
 * `false` cuando el veredicto es `no_legible` — no soportada o malformada. Entonces las
 * métricas se pueden **describir** (valores medidos), pero no se puede afirmar que una
 * careciera de apoyo, que se evaluara ni qué prefirió el motor: el registro no lo dice de
 * forma legible. Es una sola función para que ningún componente lo decida por su cuenta.
 */
export function evaluacionInterpretable(m: ModeloDePresentacion): boolean {
  // J: «sin evaluación registrada» tampoco es interpretable. No hay registro que leer, así
  // que la narrativa persistida es lo **único** que consta y no puede rotularse como relato
  // de una evaluación que no existe. Antes esto devolvía `true` para la histórica.
  if (m.veredicto.tipo === 'sin_evaluacion') return false;
  return m.veredicto.tipo !== 'no_legible';
}

/** Texto de una etiqueta: el traducido si es conocida, y si no, el código crudo. */
export function textoDeEtiqueta(e: EtiquetaDeCodigo | null | undefined): string {
  if (e == null) return '';
  return e.texto ?? e.codigo;
}
