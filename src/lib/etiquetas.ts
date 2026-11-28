/**
 * Etiquetas y nombres que **no** interpretan nada.
 *
 * Vive aparte de `interpretacion.ts` porque lo que queda aquí no decide resultados: son
 * nombres de las dos grabaciones, textos fijos y limpieza del nombre de un juego. Se
 * conservan después de retirar el motor de interpretación de TypeScript, que desde C.3 es
 * responsabilidad **solo** de Rust (`evaluacion.rs`).
 *
 * Nada de este módulo calcula preferencias, márgenes ni veredictos.
 */

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

/** Nombre de juego legible: `7DaysToDie` → `7 Days to Die` es imposible en general. */
export function nombreLegible(nombre: string | null | undefined): string {
  const n = (nombre ?? '').trim();
  if (!n) return 'Juego desconocido';
  // Solo se limpian los sufijos técnicos más comunes; no se intenta adivinar espacios.
  return (
    n
      .replace(/\.(x86_64|x86|exe|bin|sh)$/i, '')
      .replace(/-Win64-Shipping$/i, '')
      .trim() || n
  );
}
