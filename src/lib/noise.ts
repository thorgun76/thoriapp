/**
 * Ruido entre dos grabaciones del mismo juego.
 *
 * El backend estima, para cada grabación, cuánto puede moverse su FPS promedio solo por
 * variación natural (±x %, 95 % de confianza). Aquí se decide, con ese dato, a partir de
 * qué diferencia se puede hablar de mejora o empeoramiento.
 */

/**
 * Diferencia mínima (en %) que siempre se considera "igual", aunque las grabaciones
 * digan que el ruido es menor. MangoHud y el propio juego varían más que eso entre
 * partidas en las mismas condiciones.
 */
export const MIN_TIE = 5;

/** Margen de ruido combinado, nunca por debajo de {@link MIN_TIE}. */
export function tieThreshold(noisePercent?: number | null): number {
  if (noisePercent == null || !Number.isFinite(noisePercent) || noisePercent <= 0) {
    return MIN_TIE;
  }
  return Math.max(MIN_TIE, noisePercent);
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
