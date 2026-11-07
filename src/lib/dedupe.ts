import type { Game } from './types';

/**
 * Normaliza un título para compararlo entre launchers.
 *
 * El mismo juego aparece escrito de mil formas según la tienda: «Soulmask»,
 * «SOULMASK», «Soulmask™», «Soulmask (Steam)». Se quitan acentos, marcas
 * comerciales, sufijos de tienda y todo lo que no sea letra o número para que
 * las tres variantes coincidan.
 */
export function normalizeTitle(name: string): string {
  return name
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '') // acentos (á -> a)
    .replace(/™|®|©/g, '')
    // Sufijos de tienda que algunos launchers añaden al título
    .replace(/\((steam|gog|epic|amazon|ea app|ubisoft|battle\.?net)\)/g, '')
    .replace(/\b(steam|gog|epic games?|amazon games?)\b/g, '')
    // "Edicion" y "Edition" son la misma edición
    .replace(/\bedition\b/g, 'edicion')
    .replace(/[^a-z0-9]/g, '');
}

/**
 * Quita las copias de juegos que ya están en Steam.
 *
 * Steam manda: si un juego está en tu biblioteca de Steam y además lo tienes
 * añadido en Lutris o Heroic (o en los dos), en la biblioteca debe aparecer una
 * sola vez, la de Steam. Así no se ven duplicados ni se elige sin querer la
 * copia "menos buena".
 *
 * Se compara por título normalizado y también por AppID, porque un juego de
 * Steam abierto desde Lutris conserva su appid y es la coincidencia más fiable.
 */
export function dedupeAgainstSteam(steamGames: Game[], externalGames: Game[]): Game[] {
  const steamTitles = new Set<string>();
  const steamAppIds = new Set<number>();

  for (const game of steamGames) {
    steamTitles.add(normalizeTitle(game.name));
    if (game.appid > 0) steamAppIds.add(game.appid);
  }

  return externalGames.filter((game) => {
    // Mismo appid de Steam: es el mismo juego, sin ambigüedad.
    if (game.appid > 0 && steamAppIds.has(game.appid)) return false;
    const title = normalizeTitle(game.name);
    // Título vacío (solo símbolos): no se puede comparar, se conserva.
    if (!title) return true;
    return !steamTitles.has(title);
  });
}
