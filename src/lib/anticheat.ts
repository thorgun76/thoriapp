import type { AntiCheatInfo } from './types';

/**
 * Traduce el estado de AreWeAntiCheatYet a algo que se pueda leer y accionar.
 *
 * Los cinco estados de la fuente no son obvios: `Running` (funciona, pero el anti-cheat
 * no está habilitado oficialmente) se confunde con `Supported` todo el tiempo, y la
 * diferencia importa mucho: en `Running` un parche del juego puede romperlo de un día
 * para otro sin que nadie lo haya prometido.
 */

export type AcSeverity = 'ok' | 'caution' | 'blocked' | 'unknown';

export interface AcVerdict {
  /** Etiqueta corta en español. */
  label: string;
  severity: AcSeverity;
  /** Qué significa para el jugador. */
  detail: string;
  /** Qué hacer al respecto, si hay algo que hacer. */
  advice: string | null;
}

const VERDICTS: Record<string, Omit<AcVerdict, 'label'>> = {
  supported: {
    severity: 'ok',
    detail: 'El anti-cheat funciona en Linux: el juego arranca en su modo normal.',
    advice: null,
  },
  running: {
    severity: 'caution',
    detail:
      'Funciona en Linux, pero el anti-cheat no está habilitado oficialmente. Un parche puede romperlo sin aviso.',
    advice: 'Si deja de arrancar tras una actualización, no es tu configuración: es el anti-cheat.',
  },
  planned: {
    severity: 'caution',
    detail: 'El soporte está anunciado, pero todavía no funciona.',
    advice: 'Juega en Windows mientras tanto, o comprueba el estado antes de comprar.',
  },
  broken: {
    severity: 'blocked',
    detail: 'El anti-cheat impide jugar en Linux con los datos actuales.',
    advice: 'Ningún ajuste de esta app lo arregla: el bloqueo está en el anti-cheat.',
  },
  denied: {
    severity: 'blocked',
    detail: 'El desarrollador ha descartado dar soporte a Linux o Steam Deck.',
    advice: 'No hay nada que configurar: el juego no va a funcionar bajo Proton.',
  },
};

const LABELS: Record<string, string> = {
  supported: 'Funciona',
  running: 'Funciona, sin soporte oficial',
  planned: 'Soporte anunciado',
  broken: 'No funciona',
  denied: 'Bloqueado por el anti-cheat',
};

/** Veredicto completo a partir del estado. Un estado desconocido no se da por bueno. */
export function antiCheatVerdict(info: AntiCheatInfo | null | undefined): AcVerdict {
  if (!info || !info.status) {
    return {
      label: 'Sin datos',
      severity: 'unknown',
      detail: 'No hay información de anti-cheat para este juego.',
      advice: null,
    };
  }

  const key = info.status.toLowerCase();
  const known = VERDICTS[key];
  if (!known) {
    // Estado nuevo en la fuente: se muestra tal cual y se deja en "cautela", nunca en "ok".
    return {
      label: info.status,
      severity: 'unknown',
      detail: 'Estado no reconocido en la lista de anti-cheat.',
      advice: null,
    };
  }

  return { ...known, label: LABELS[key] ?? info.status };
}

/** ¿Hay que avisar antes de que el usuario pierda el tiempo configurando? */
export const isBlocked = (info: AntiCheatInfo | null | undefined): boolean =>
  antiCheatVerdict(info).severity === 'blocked';

/**
 * ¿La app debe NEGARSE a generar un comando de lanzamiento para este juego?
 *
 * Es la puerta dura, y es más estrecha que `isBlocked` a propósito:
 *
 * - `Denied` aquí significa «el desarrollador ha descartado Linux/Steam Deck».
 *   No hay ajuste que arregle una decisión del desarrollador, y seguir un consejo
 *   de la app con un anti-cheat denegado es el error más caro posible (un baneo).
 * - `Broken` NO se bloquea: el juego no arranca, pero no hay riesgo de cuenta y el
 *   usuario puede tener una versión antigua. El aviso ya está en la tarjeta.
 * - Sin datos (`null`) NO se bloquea: sería negativo por defecto sobre juegos que
 *   funcionan perfectamente, y el catálogo de AreWeAntiCheatYet no cubre todo Steam.
 * - Un estado desconocido tampoco bloquea, por el mismo motivo.
 *
 * Función PURA: recibe el dato y decide. La UI y la reconstrucción de perfiles
 * guardados usan esta misma función, para que no puedan divergir.
 */
export const isDenied = (info: AntiCheatInfo | null | undefined): boolean =>
  (info?.status ?? '').trim().toLowerCase() === 'denied';

/**
 * Texto único del bloqueo, para que la tarjeta, la caja del comando y el
 * portapapeles digan exactamente lo mismo.
 */
export function deniedReason(info: AntiCheatInfo | null | undefined): string {
  const cheats = antiCheatsLabel(info);
  const usa = cheats ? ` (${cheats})` : '';
  return `Anti-cheat denegado${usa}: el juego no va a funcionar en Linux y la app no genera comandos para él.`;
}

/**
 * Marca de un perfil bloqueado. Vive aquí y no en `useProfiles` para que el texto
 * del bloqueo exista en un solo sitio y el marcador no pueda quedar desincronizado
 * con el mensaje.
 */
export function deniedCommand(appid: number): string {
  return `Bloqueado por anti-cheat (AppID ${appid}): no hay comando de lanzamiento.`;
}

/** ¿Este texto (que iba a ser un comando) es en realidad un bloqueo? */
export const isDeniedCommand = (text: string): boolean =>
  text.startsWith('Bloqueado por anti-cheat');

/** "Easy Anti-Cheat + BattlEye", o `null` si la fuente no los lista. */
export function antiCheatsLabel(info: AntiCheatInfo | null | undefined): string | null {
  const list = info?.anticheats ?? [];
  return list.length > 0 ? list.join(' + ') : null;
}
