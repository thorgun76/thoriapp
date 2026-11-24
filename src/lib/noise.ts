/**
 * Ruido entre dos grabaciones del mismo juego.
 *
 * El backend estima, para cada grabación, cuánto puede moverse su FPS promedio solo por
 * variación natural (±x %, 95 % de confianza). Aquí se decide, con ese dato, a partir
 * de qué diferencia se puede hablar de mejora o empeoramiento.
 *
 * Los dos umbrales viven en `lib/umbrales.ts`, con la explicación de por qué son dos
 * conceptos distintos y no uno.
 */

import { MIN_TIE_PERCENT } from './umbrales';

/**
 * Diferencia mínima (en %) que siempre se considera "igual", aunque las grabaciones
 * digan que el ruido es menor. MangoHud y el propio juego varían más que eso entre
 * partidas en las mismas condiciones.
 *
 * @deprecated Usa {@link MIN_TIE_PERCENT} de `lib/umbrales.ts`. Se mantiene este alias
 * porque hay código y tests que lo importan desde aquí.
 */
export const MIN_TIE = MIN_TIE_PERCENT;

/** Margen de ruido combinado, nunca por debajo de {@link MIN_TIE_PERCENT}. */
export function tieThreshold(noisePercent?: number | null): number {
  if (noisePercent == null || !Number.isFinite(noisePercent) || noisePercent <= 0) {
    return MIN_TIE_PERCENT;
  }
  return Math.max(MIN_TIE_PERCENT, noisePercent);
}

/** Texto corto del margen para la interfaz: `'±4.2%'`, o `null` si no hay dato. */
export function marginLabel(noisePercent?: number | null): string | null {
  if (noisePercent == null || !Number.isFinite(noisePercent) || noisePercent <= 0) {
    return null;
  }
  return `±${noisePercent.toFixed(1)}%`;
}

/** ¿La diferencia entra dentro de lo que puede explicar el ruido? */
export function withinNoise(changePercent: number, noisePercent?: number | null): boolean {
  return Math.abs(changePercent) <= tieThreshold(noisePercent);
}
