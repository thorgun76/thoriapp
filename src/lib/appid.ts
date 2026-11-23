/**
 * Emparejar un juego de un launcher externo (Lutris, Heroic, HeroNite) con su
 * AppID de Steam.
 *
 * Esas entradas no traen AppID, pero sí un nombre. Steam y ProtonDB se indexan
 * por appid, así que sin resolverlo no hay ni nivel de ProtonDB ni ficha de
 * requisitos: solo se puede buscar en la tienda por nombre.
 *
 * La búsqueda de Steam es difusa (buscar "Stolen" devuelve "Probably Stolen",
 * "Stolen Soundtrack", "Stolen Realm"…), así que aquí **nunca se coge el primer
 * resultado**: solo vale una coincidencia única, y de tres maneras:
 *
 * 1. Exacta tras normalizar.
 * 2. Prefijo cortado en un separador ("Probably Stolen" → "Probably Stolen -
 *    Cyberpunk Shopkeeper Sim").
 * 3. Nombre **truncado**: casi todo el nombre coincide y solo falta el final
 *    ("Probably Stolen Cyberpunk Shopkeeper S" → "…Shopkeeper Sim"), que es como
 *    lo guardan algunos launchers.
 *
 * Si hay dudas se devuelve `null`, porque enseñar el nivel de ProtonDB de otro
 * juego es peor que no enseñar nada.
 */

/** Nombre comparable: minúsculas, sin acentos, sin signos y con un solo espacio. */
export function normalizeName(name: string): string {
  return name
    .normalize('NFD')
    // Marcas diacríticas sueltas tras NFD ("Pokémon" → "Pokemon").
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    // Apóstrofes se quitan, no se convierten en espacio: "No Man's Sky", "No Man’s
    // Sky" y "No Mans Sky" tienen que ser la misma cadena, y si el apóstrofe deja
    // un hueco la primera queda como "man s sky" y no empareja con la tercera.
    .replace(/['\u2018\u2019`]/g, '')
    // Todo lo que no sea letra/número (guiones, dos puntos, ®…) pasa a espacio:
    // "Cyberpunk 2077: Ultimate" y "Cyberpunk 2077 - Ultimate" son el mismo juego.
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

export interface StoreHit {
  appid: number;
  name: string;
}

/**
 * AppID de la coincidencia válida, o `null` si no hay una sola candidata.
 *
 * Dos niveles, ambos exigiendo unicidad:
 * 1. **Exacta** tras normalizar.
 * 2. **Prefijo cortado en separador** en cualquiera de las dos direcciones: la
 *    biblioteca puede guardar el nombre corto ("Probably Stolen") y la tienda el
 *    largo con subtítulo, o al revés. El corte tiene que caer en un separador
 *    (" - ", ": ", " ("), no en mitad del título: "Probably Stolen" es prefijo de
 *    "Probably Stolen Realm" letra a letra, pero no es el mismo juego.
 */
export function matchAppId(games: StoreHit[], name: string): number | null {
  const target = normalizeName(name);
  if (!target) return null;

  // La tienda repite el mismo juego en los resultados: se agrupa por AppID para
  // que un duplicado no parezca "dos candidatas distintas".
  const uniq = [...new Map(games.map((g) => [g.appid, g])).values()];
  const norm = uniq.map((g) => ({ appid: g.appid, n: normalizeName(g.name) }));

  const exact = norm.filter((g) => g.n === target);
  if (exact.length === 1) return exact[0].appid;

  const prefix = uniq.filter(
    (g) => isSeparatedPrefix(g.name, name) || isSeparatedPrefix(name, g.name)
  );
  if (prefix.length === 1) return prefix[0].appid;

  const truncated = norm.filter(
    (g) => isTruncation(g.n, target) || isTruncation(target, g.n)
  );
  if (truncated.length === 1) return truncated[0].appid;

  return null;
}

/**
 * ¿`short` es `long` a medias, cortado a mitad de palabra?
 *
 * Es el caso de un nombre truncado al guardarlo: "Probably Stolen Cyberpunk
 * Shopkeeper S" frente a "Probably Stolen - Cyberpunk Shopkeeper Sim". Se exige
 * que el trozo que falta sea pequeño (≥85 % del nombre ya coincide): así
 * "Probably Stolen" (15 de 21 letras, 71 %) **no** se empareja con "Probably
 * Stolen Realm", que es otro juego.
 */
function isTruncation(short: string, long: string): boolean {
  if (!short || short === long || short.length > long.length) return false;
  return long.startsWith(short) && short.length / long.length >= 0.85;
}

/** Minúsculas y sin acentos, **conservando la puntuación** (hace falta para el corte). */
function loose(name: string): string {
  return name
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * ¿`short` es el principio de `long` cortando justo en un separador?
 *
 * `"Probably Stolen"` → `"Probably Stolen - Cyberpunk Shopkeeper Sim"` sí (`-`).
 * `"Probably Stolen"` → `"Probably Stolen Realm"` no (sigue una palabra).
 */
function isSeparatedPrefix(short: string, long: string): boolean {
  const s = loose(short);
  const l = loose(long);
  if (!s || s === l || !l.startsWith(s)) return false;
  const rest = l.slice(s.length).trimStart();
  // El corte debe caer sobre puntuación, no sobre una letra o un dígito.
  return rest !== '' && /^[^\p{L}\p{N}]/u.test(rest);
}

/**
 * Términos de búsqueda a probar, del más fiable al más corto.
 *
 * Lutris guarda a veces el nombre con adornos ("(GOG)", "(v1.2)", "— GOTY"), y
 * la tienda devuelve cero resultados con ellos. Se reintenta con las primeras
 * palabras, que es lo que sí indexa Steam. El emparejamiento posterior sigue
 * exigiendo coincidencia única, así que acortar no relaja la seguridad.
 */
export function searchTerms(name: string): string[] {
  // Se sueltan los paréntesis/corchetes ("(GOG)", "[FitGirl]", "(v1.2)"): la tienda
  // no los conoce y devuelve cero resultados con ellos.
  const clean = name.replace(/[([][^)\]]*[)\]]/g, ' ');
  const words = clean.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w));
  const terms = [words.join(' '), words.slice(0, 4).join(' '), words.slice(0, 3).join(' ')];
  // Sin duplicados ni términos vacíos, manteniendo el orden.
  return terms.filter((t, i) => t !== '' && terms.indexOf(t) === i);
}

/**
 * Resuelve el AppID probando los términos de `searchTerms`.
 *
 * `search` es `api.searchStore` (inyectado para poder probarlo sin Tauri). Si la
 * red falla se corta: reintentar con otro término no arregla una caída.
 */
export async function resolveAppId(
  name: string,
  search: (term: string) => Promise<StoreHit[]>
): Promise<{ appid: number | null; tried: string[]; found: string[] }> {
  const tried: string[] = [];
  const found: string[] = [];

  for (const term of searchTerms(name)) {
    tried.push(term);
    let hits: StoreHit[];
    try {
      hits = await search(term);
    } catch (e) {
      console.warn('La búsqueda en Steam falló:', e);
      break;
    }
    found.push(...hits.map((h) => `${h.appid} ${h.name}`));
    const appid = matchAppId(hits, name);
    if (appid !== null) return { appid, tried, found };
  }

  return { appid: null, tried, found };
}
