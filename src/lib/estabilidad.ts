/**
 * Índice de estabilidad de FPS de Thoriapp.
 *
 * **Es un índice propio, no una métrica estándar.** MangoHud, CapFrameX y MSI
 * Afterburner no publican nada parecido: aquí se define como *qué fracción de los FPS
 * medios se conserva en los peores momentos*.
 *
 *   c1  = fps_1_percent_low   / avg_fps
 *   c01 = fps_0_1_percent_low / avg_fps
 *   estabilidad = ((2·c1 + c01) / 3) · 100
 *
 * El 1 % low pesa el doble que el 0,1 % low porque el 0,1 % low depende de muy pocas
 * muestras (ver {@link low01Reliability}) y no es un número estable.
 *
 * ## Lo que este índice NO dice
 *
 * - **No dice si hay FPS suficientes.** Un juego clavado a 30 FPS da 100/100. Se lee
 *   como «va parejo», nunca como «va fluido». Por eso en la interfaz siempre va
 *   acompañado de los FPS bajos absolutos.
 * - **No es comparable entre juegos.** Es un cociente sobre el propio promedio: 60/100
 *   con 120 FPS y 60/100 con 30 FPS son el mismo número y experiencias distintas.
 * - **No es un porcentaje de nada.** Se muestra sobre 100, no con `%`.
 *
 * Se conserva la fórmula original para que las grabaciones guardadas sigan dando el
 * mismo número: cambiar los pesos invalidaría el historial sin ganar precisión.
 */

/** Estabilidad por debajo de esto se considera "poco parejo". */
export const ESTABILIDAD_BAJA = 35;
/** Estabilidad por encima de esto se considera "bastante parejo". */
export const ESTABILIDAD_ALTA = 65;

/**
 * Estabilidad 0-100 de una grabación, o `null` si no se puede calcular.
 *
 * Devuelve `null` en vez de 0 cuando faltan datos o son inválidos: un 0 se leería como
 * «pesimísimo» cuando en realidad es «no medido», que es una diferencia importante.
 */
export function estabilidad(
  avgFps: number | null | undefined,
  low1: number | null | undefined,
  low01: number | null | undefined
): number | null {
  // Sin FPS medios no hay ratio posible. Cero o negativo es un dato corrupto.
  if (avgFps == null || !Number.isFinite(avgFps) || avgFps <= 0) return null;
  if (low1 == null || !Number.isFinite(low1) || low1 < 0) return null;
  if (low01 == null || !Number.isFinite(low01) || low01 < 0) return null;

  const c1 = low1 / avgFps;
  const c01 = low01 / avgFps;
  const score = ((2 * c1 + c01) / 3) * 100;

  // Los percentiles bajos calculados desde frametime pueden superar al promedio (si el
  // log trae filas raras); se satura a 100 para no presentar 143/100.
  return Math.max(0, Math.min(100, score));
}

/** Igual que {@link estabilidad}, pero a partir de un benchmark. */
export function estabilidadDe(b: {
  avg_fps: number;
  fps_1_percent_low: number;
  fps_0_1_percent_low: number;
}): number | null {
  return estabilidad(b.avg_fps, b.fps_1_percent_low, b.fps_0_1_percent_low);
}

/**
 * Etiqueta cualitativa. Deliberadamente **no** dice «fluido»: el índice mide
 * regularidad relativa, y decir «muy fluido» a un juego a 30 FPS estables sería falso.
 */
export function estabilidadLabel(score: number | null): string {
  if (score == null) return 'Sin datos';
  if (score >= ESTABILIDAD_ALTA) return 'Muy estable';
  if (score >= 50) return 'Estable';
  if (score >= ESTABILIDAD_BAJA) return 'Algo irregular';
  return 'Muy irregular';
}

/** Diferencia en **puntos** (no en %): de 40 a 55 son 15 puntos. */
export function puntosDeDiferencia(
  antes: number | null,
  despues: number | null
): number | null {
  if (antes == null || despues == null) return null;
  return despues - antes;
}

/** Sobre qué se apoya el 0,1 % low de una grabación. */
export interface Low01Reliability {
  /** Muestras del log tras el calentamiento (≈ 10 por segundo con MangoHud). */
  muestras: number;
  /** Cuántas muestras caen dentro del 0,1 % más lento. */
  muestrasExtremas: number;
  /** `false` cuando el valor depende de una o dos muestras. */
  fiable: boolean;
  /** Aviso para la interfaz, o `null` si no hace falta. */
  aviso: string | null;
}

/**
 * ¿Cuántos datos sostienen el 0,1 % low?
 *
 * El 0,1 % low es el percentil 99,9 del frametime, interpolado. Con `log_interval=100`
 * (10 muestras/s, lo normal en MangoHud) una grabación de 30 s da ~300 muestras y el
 * 0,1 % son **0,3 muestras**: el valor sale de interpolar entre las dos peores y lo
 * determina básicamente un único frame. Llamarlo «los peores momentos» invita a
 * decisiones que el dato no sostiene.
 *
 * No se aplica un umbral estadístico: se informa de cuántas muestras hay, que es un
 * hecho verificable, y se marca poco fiable por debajo de 10 muestras.
 */
export function low01Reliability(muestras: number | null | undefined): Low01Reliability {
  const n = muestras != null && Number.isFinite(muestras) && muestras > 0 ? muestras : 0;
  // Nunca 0: el 0,1 % low se interpola entre las dos peores muestras, así que como poco
  // se apoya en una. Redondear a la baja convertía 493 muestras en «~0 muestras», que
  // además se leía igual que «no hay datos» cuando el caso real es «depende de 1 frame».
  const extremas = Math.max(1, Math.round((n * 0.1) / 100));

  if (n === 0) {
    return {
      muestras: 0,
      muestrasExtremas: 0,
      fiable: false,
      aviso: 'No se registró cuántas muestras tiene esta grabación.',
    };
  }
  if (extremas < 10) {
    return {
      muestras: n,
      muestrasExtremas: extremas,
      fiable: false,
      aviso:
        extremas === 1
          ? `El 0,1 % low se calcula sobre 1 sola muestra de ${n}: depende de un único frame y puede moverse mucho entre partidas.`
          : `El 0,1 % low se calcula sobre ${extremas} de las ${n} muestras: son muy pocas y puede moverse mucho entre partidas.`,
    };
  }
  return { muestras: n, muestrasExtremas: extremas, fiable: true, aviso: null };
}

/** Muestras mínimas para que el 0,1 % low se apoye en 10 o más. */
export const MUESTRAS_PARA_LOW01_FIABLE = 10_000;
