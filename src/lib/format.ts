/** Utilidades de formato compartidas por las vistas de pruebas. */

/** Tiers de ProtonDB, de mejor a peor. */
export const PROTONDB_TIERS = ['platinum', 'gold', 'silver', 'bronze', 'borked'] as const;
export type ProtonTier = (typeof PROTONDB_TIERS)[number];

/**
 * `platinum` → "Platino", `borked` → "No funciona".
 *
 * Antes esta traducción estaba copiada dentro de CompatTab y la tarjeta del comando
 * se dejaba el valor crudo en inglés ("ProtonDB: borked").
 */
export function tierLabel(tier: string | null | undefined): string {
  switch ((tier ?? '').toLowerCase()) {
    case 'platinum':
      return 'Platino';
    case 'gold':
      return 'Oro';
    case 'silver':
      return 'Plata';
    case 'bronze':
      return 'Bronce';
    case 'borked':
      return 'No funciona';
    default:
      // Un tier desconocido (ProtonDB añade valores nuevos) se enseña tal cual.
      return tier ?? '';
  }
}

/**
 * Qué significa el tier para quien lo lee: un "Oro" no dice si hay que activar
 * ajustes ni si el multijugador va a funcionar.
 */
export function tierDescription(tier: string | null | undefined): string | null {
  switch ((tier ?? '').toLowerCase()) {
    case 'platinum':
      return 'Funciona sin tocar nada.';
    case 'gold':
      return 'Funciona tras ajustes sencillos.';
    case 'silver':
      return 'Funciona, pero con problemas notables.';
    case 'bronze':
      return 'Arranca, con fallos graves (cuelgues, gráficos rotos).';
    case 'borked':
      return 'No arranca o no se puede jugar con los componentes actuales de Proton.';
    default:
      return null;
  }
}

/** 90 → "1 min 30 s", 60 → "1 min", 45 → "45 s". */
export function fmtDuration(totalSeconds: number): string {
  const s = Math.round(totalSeconds);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  const rest = s % 60;
  return rest === 0 ? `${m} min` : `${m} min ${rest} s`;
}

/** Fecha ISO → "07/11/2026, 11:36:12" (o el texto original si no es una fecha). */
export function formatTimestamp(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    if (Number.isNaN(date.getTime())) return timestamp;
    return date.toLocaleString('es-ES', {
      day: '2-digit',
      month: '2-digit',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  } catch {
    return timestamp;
  }
}

/**
 * Texto normalizado para buscar por nombre de juego.
 *
 * Todo en minúsculas y sin acentos, para que «pokemon» encuentre «Pokémon» y
 * «lords of the fallen» encuentre «Lords of the Fallen». No se toca nada más:
 * quitar signos haría que «Fallout 4» y «Fallout: 4» se confundieran, y no
 * queremos inventar coincidencias que el usuario no espera.
 */
export function normalizeForSearch(text: string): string {
  return text
    .toLowerCase()
    // NFD separa la letra de su acento y el rango 0300–036F son las marcas
    // diacríticas sueltas: al quitarlas queda la letra base.
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');
}

/** ¿El nombre del juego coincide con lo que hay escrito en el buscador? */
export function matchesSearch(name: string, query: string): boolean {
  const q = normalizeForSearch(query.trim());
  if (!q) return true;
  return normalizeForSearch(name).includes(q);
}
