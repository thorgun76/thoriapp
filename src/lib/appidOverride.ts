import { normalizeName } from './appid';

/**
 * AppID que el usuario ha vinculado a mano a un juego de otro launcher.
 *
 * La detección automática busca el juego por nombre en la tienda de Steam, y eso
 * funciona con la mayoría, pero no con todos: hay bibliotecas que guardan
 * "NoManSky_v5.0_fitgirl" o un nombre en otro idioma, y ninguna regla de parecido
 * puede adivinar eso sin riesgo de enseñar el nivel de ProtonDB de otro juego.
 *
 * Así que la app deja vincularlo a mano una vez y lo recuerda. Se guarda por
 * **launcher + nombre normalizado**, no por appid (cuando no hay appid, es justo
 * lo que falta), en `localStorage`: sobrevive a reiniciar la app y no ensucia
 * ningún fichero de la biblioteca.
 */

const STORAGE_KEY = 'thoriapp:appidOverrides';

/** Clave estable de un juego sin AppID. */
export function overrideKey(game: { name: string; launcher?: string }): string {
  return `${game.launcher ?? 'steam'}:${normalizeName(game.name)}`;
}

/** Todos los vínculos guardados (`{}` si no hay ninguno o el almacén falla). */
export function loadOverrides(): Record<string, number> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    // Se filtra al leer: si el almacén se corrompe no debe romper la app.
    const out: Record<string, number> = {};
    for (const [k, v] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof v === 'number' && Number.isInteger(v) && v > 0) out[k] = v;
    }
    return out;
  } catch {
    return {};
  }
}

/** AppID vinculado a mano para este juego, o `null` si no hay ninguno. */
export function getOverride(game: { name: string; launcher?: string }): number | null {
  return loadOverrides()[overrideKey(game)] ?? null;
}

/** Guarda el vínculo. Devuelve `false` si el almacén no está disponible (sin romper nada). */
export function setOverride(
  game: { name: string; launcher?: string },
  appid: number
): boolean {
  try {
    const all = loadOverrides();
    all[overrideKey(game)] = appid;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
    return true;
  } catch {
    return false;
  }
}

/** Borra el vínculo de este juego. */
export function clearOverride(game: { name: string; launcher?: string }): void {
  try {
    const all = loadOverrides();
    delete all[overrideKey(game)];
    localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
  } catch {
    /* sin almacén no hay nada que borrar */
  }
}
