import type { Benchmark } from './types';

/**
 * Texto del comando **con el que se probó**, para enseñarlo en el historial y en los
 * tooltips.
 *
 * Se prefiere lo que Steam tenía configurado (`steam_command`) porque es lo que de verdad
 * se lanzó al entrar al juego. El comando del editor (`launch_command`) es una propuesta
 * que el usuario copia a mano y puede no haber llegado nunca a Steam; además, cuando la
 * instantánea del editor sale vacía se pinta `%command%`, que no dice con qué argumentos se
 * probó. Antes se enseñaba solo ese, y una prueba real con
 * `mesa_glthread=true gamemoderun mangohud %command%` aparecía como `%command%`.
 *
 * Devuelve `null` cuando no hay dato de ninguna de las dos fuentes: la interfaz dice
 * «Comando no registrado» en vez de inventar `%command%`, que significaría «sin ajustes».
 *
 * Vive aquí y no en un componente para poder compartirse entre `BenchmarksView`,
 * `SavedComparisonDetail` y el tooltip de `PruebaN` sin crear dependencias circulares.
 */
export function textoComando(b: Benchmark | null | undefined): string | null {
  if (!b) return null;
  const steam = (b.steam_command ?? '').trim();
  if (steam) return `Steam: ${steam}`;
  // Steam leído y vacío es un dato: se lanzó sin argumentos.
  if (b.steam_command === '') return 'Steam: sin opciones de lanzamiento';
  const editor = (b.launch_command || '').trim();
  return editor ? `Comando: ${editor}${sufijoOrigen(b)}` : null;
}

/**
 * El comando **pegable**: sin `Steam:`/`Comando:` delante ni la marca de origen.
 *
 * Es lo que se lleva el botón de copiar. Se usa la misma prioridad que
 * {@link textoComando} (Steam primero, porque es lo que de verdad se ejecutó), pero
 * devolviendo solo el texto: pegar «Comando: mangohud %command% (origen desconocido)» en
 * Steam rompería el lanzamiento.
 *
 * Devuelve `null` si no hay comando registrado. Nunca se inventa `%command%`: copiar un
 * comando que el usuario no tuvo sería peor que no copiar nada.
 */
export function comandoPegable(b: Benchmark | null | undefined): string | null {
  if (!b) return null;
  const steam = (b.steam_command ?? '').trim();
  if (steam) return steam;
  // `''` = se leyó Steam y estaba vacío: no hay nada que pegar, y `%command%` sería una
  // invención. Se devuelve null y la interfaz no ofrece copiar.
  if (b.steam_command === '') return null;
  const editor = (b.launch_command || '').trim();
  return editor || null;
}

/**
 * Marca de dónde salió el comando, solo cuando **no** vino de la grabación.
 *
 * Sin esto, una prueba con `%command%` no se puede auditar: no se sabe si de verdad no
 * había ajustes o si el dato se leyó al guardar.
 */
export function sufijoOrigen(b: Benchmark): string {
  if (b.origen_comando === 'instantanea') return '';
  if (b.origen_comando === 'comando_actual') return ' (leído al guardar, no al grabar)';
  // `undefined` = prueba guardada antes de que existiera el campo. No se puede afirmar
  // que su comando saliera de la grabación, así que se marca igual que `desconocido`.
  return ' (origen desconocido)';
}

/**
 * Etiqueta de una grabación: «Prueba N», o «Prueba sin número» si es antigua.
 *
 * El número lo asigna el backend al guardar y no cambia nunca. Las grabaciones anteriores
 * a ese campo se quedan sin número a propósito: no se les inventa una cronología que no se
 * puede reconstruir.
 */
export function etiquetaPrueba(b: Benchmark | null | undefined): string {
  const n = b?.number;
  return n != null && Number.isFinite(n) && n > 0 ? `Prueba ${n}` : 'Prueba sin número';
}
