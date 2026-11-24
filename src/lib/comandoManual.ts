/**
 * Estado del editor manual del comando, **indexado por juego**.
 *
 * ## Por qué esto es un módulo y no un `useState` suelto
 *
 * Antes era un único `manualCommand`/`useManualCommand` global. Efecto real: al traer el
 * comando de Steam para Palworld (`mesa_glthread=true game-performance mangohud
 * %command%`), ese texto aparecía en **todos** los juegos, y `Restablecer` no lo quitaba
 * porque solo vaciaba los ajustes.
 *
 * Cada juego tiene su propia línea de lanzamiento en Steam, así que su comando manual
 * también tiene que ser suyo. Aquí vive el criterio de «qué clave identifica un juego»,
 * para que todas las rutas (mostrar, editar, restablecer) usen el mismo.
 */

/** Juego mínimo necesario para identificarlo. */
export interface JuegoIdentificable {
  appid: number;
  name: string;
  launcher?: string;
}

/**
 * Clave estable de un juego.
 *
 * - Con AppID de Steam: `steam:<appid>`. Es la identidad fuerte.
 * - Sin AppID (Lutris, Heroic, juego sin vincular): lanzador + nombre. Es lo único que hay;
 *   dos juegos homónimos del mismo lanzador se considerarían el mismo, que es el mismo
 *   criterio que ya usa el resto de la app para agrupar.
 */
export function claveJuego(juego: JuegoIdentificable | null | undefined): string {
  if (!juego) return '';
  if (juego.appid) return `steam:${juego.appid}`;
  return `${juego.launcher ?? 'x'}:${juego.name}`;
}

/** Comando manual guardado para un juego (`''` = no hay). */
export function comandoManualDe(
  comandos: Record<string, string>,
  clave: string
): string {
  return comandos[clave] ?? '';
}

/** ¿Está el editor manual activo para este juego? */
export function editandoAMano(
  activos: Record<string, boolean>,
  clave: string
): boolean {
  return activos[clave] ?? false;
}

/**
 * Estado tras pulsar `Restablecer` para un juego.
 *
 * Vuelve a `%command%`: desactiva el editor manual y borra su texto **solo de ese juego**.
 * Restablecer los demás no es lo que pide el botón y borraría trabajo que el usuario no
 * ha tocado.
 */
export function restablecerJuego(
  comandos: Record<string, string>,
  activos: Record<string, boolean>,
  clave: string
): { comandos: Record<string, string>; activos: Record<string, boolean> } {
  return {
    comandos: { ...comandos, [clave]: '' },
    activos: { ...activos, [clave]: false },
  };
}
