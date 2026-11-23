/**
 * Navegación entre la barra lateral y la vista de juego.
 *
 * La vista de juego (reglas, ProtonDB, motor, recomendaciones) solo se pinta
 * dentro de la sección "Mi biblioteca" de App.tsx. Abrir un juego desde el
 * buscador estando en pruebas, perfiles o benchmarks dejaba el estado `current`
 * cargado pero sin vista que lo mostrara: el clic en la sugerencia no hacía nada.
 */
import type { SidebarView } from './types';

/** Sección a la que hay que saltar para que se vea la vista de un juego abierto. */
export const GAME_VIEW_SECTION: SidebarView = 'library';

/**
 * Sección activa tras abrir un juego desde el buscador.
 *
 * Siempre la biblioteca: es la única sección que renderiza la vista de juego.
 */
export function viewForOpenedGame(_from: SidebarView): SidebarView {
  return GAME_VIEW_SECTION;
}
