import type { CommunityRecommendation, GameEngine, GameRules, Profile } from './types';

/**
 * Envoltorios que van ANTES del `%command%`.
 *
 * Esta es la ÚNICA lista de envoltorios del proyecto. Antes había tres y se
 * desincronizaron: `preflight.rs` conocía 8, aquí había 3 (`KNOWN_WRAPPERS`) y 5
 * más en `CATALOG_WRAPPERS`. El resultado era que `prime-run`, `vkbasalt`,
 * `obs-gamecapture`, `game-performance` y `scx_lavd` se clasificaban como
 * argumento y acababan DETRÁS de `%command%`, o sea, como argumento del juego:
 * `%command% game-performance` no envuelve nada.
 *
 * Tiene que coincidir con `KNOWN_WRAPPERS` de `src-tauri/src/preflight.rs`. Un
 * test (`la lista de envoltorios coincide con la de preflight`) lo comprueba.
 */
export const KNOWN_WRAPPERS: readonly string[] = [
  'gamemoderun',
  'mangohud',
  'gamescope',
  'prime-run',
  'vkbasalt',
  'obs-gamecapture',
  'game-performance',
  'scx_lavd',
];

const KNOWN_WRAPPERS_SET = new Set(KNOWN_WRAPPERS);

/**
 * ¿Este parámetro de la comunidad es una variable de entorno o un argumento?
 * Réplica exacta de la lógica que tenía App.tsx, para no cambiar ningún comando.
 */
export function isEnvParameter(parameter: string): boolean {
  return (
    parameter.includes('=') ||
    parameter.startsWith('PROTON_') ||
    parameter.startsWith('DXVK_') ||
    parameter.startsWith('VKD3D_') ||
    parameter.startsWith('WINE_') ||
    parameter.startsWith('AMD_') ||
    parameter.startsWith('RADV_') ||
    parameter.startsWith('__')
  );
}

/** Clasifica un parámetro suelto en env / wrapper / argumento. */
export function classifyParameter(parameter: string): 'env' | 'wrapper' | 'arg' {
  if (isEnvParameter(parameter)) return 'env';
  if (KNOWN_WRAPPERS_SET.has(parameter)) return 'wrapper';
  return 'arg';
}

export interface CommandParts {
  envVars: string[];
  wrappers: string[];
  args: string[];
}

/** Une las piezas en el orden canónico: env + wrappers + %command% + args. */
export function joinCommand({ envVars, wrappers, args }: CommandParts): string {
  return [...envVars, ...wrappers, '%command%', ...args].join(' ');
}

/**
 * Ejecutables que Thoriapp acepta como PRIMERA palabra de un comando manual.
 *
 * `manual_command` no lo escribe la app: llega de un perfil importado, que es un
 * archivo cualquiera que el usuario abre. Tratarlo como una orden a ejecutar es
 * exactamente el puente "texto importado → instrucciones ejecutables" que hay que
 * cerrar. Se acepta un conjunto cerrado de lanzadores y envoltorios de juegos.
 *
 * Todo lo que pueda encabezar un comando generado por la app debe estar en esta
 * lista; si no, "Editar manualmente" nace mostrando un error. Ver
 * `command.test.ts`, que comprueba que cubre `KNOWN_WRAPPERS` y `games.json`.
 */
const ALLOWED_COMMANDS = new Set([
  // Envoltorios y capturadores (misma lista que arriba, sin duplicar el origen)
  ...KNOWN_WRAPPERS,
  // `env` es un lanzador (ejecuta el comando que le sigue) aunque no sea un envoltorio de
  // juegos que `preflight.rs` compruebe: no hay binario que revisar, es del sistema. Tiene
  // que estar aquí porque el validador de Rust (`launch_command::LANZADORES_PERMITIDOS`)
  // lo acepta, y si el frontend no, "Editar manualmente" rechazaría un comando válido.
  'env',
  'WINEDLLOVERRIDES',
  // Lanzadores de Steam
  'steam',
  'steam-native',
  'steam-runtime',
  // Gestores de prefijos / Proton
  'protontricks',
  'umu-run',
  'proton',
  'wine',
  'wine64',
  'winetricks',
]);

/**
 * Caracteres que permiten encadenar órdenes, redirigir o sustituir texto por la
 * salida de otro programa. Ninguno tiene uso legítimo en una línea de Steam.
 *
 * `%command%` se deja pasar: es el marcador de Steam, no del shell.
 */
const FORBIDDEN_SHELL_CHARS = /[;|&$`<>\n\r]/;

/** Máximo de caracteres que se aceptan en un comando manual. */
export const MAX_MANUAL_COMMAND_LENGTH = 2000;

export interface ManualCommandCheck {
  /** `true` si el comando se puede usar sin revisarlo a mano. */
  safe: boolean;
  /** Motivo legible del rechazo; vacío si `safe`. */
  reason: string;
  /** Primer ejecutable del comando, cuando se pudo identificar. */
  program: string | null;
}

/**
 * ¿Se puede enseñar y copiar este comando manual tal cual?
 *
 * Es una función PURA y deliberadamente conservadora: ante la duda, rechaza. No
 * intenta ejecutar nada ni sanear el texto (sanear un comando de shell es una
 * trampa: siempre queda un caso raro). Solo verifica que el texto tiene la forma
 * de una línea de lanzamiento de Steam y nada más.
 */
export function checkManualCommand(command: string): ManualCommandCheck {
  const text = command.trim();

  if (!text) {
    return { safe: false, reason: 'El comando está vacío.', program: null };
  }

  if (text.length > MAX_MANUAL_COMMAND_LENGTH) {
    return {
      safe: false,
      reason: `El comando es demasiado largo (máximo ${MAX_MANUAL_COMMAND_LENGTH} caracteres).`,
      program: null,
    };
  }

  // Un salto de línea permitiría colar una segunda orden sin que se vea.
  if (FORBIDDEN_SHELL_CHARS.test(text)) {
    const found = text.match(FORBIDDEN_SHELL_CHARS)?.[0] ?? '';
    const shown = found === '\n' || found === '\r' ? 'un salto de línea' : `«${found}»`;
    return {
      safe: false,
      reason:
        `El comando lleva ${shown}, que sirve para encadenar órdenes. ` +
        'Thoriapp solo ejecuta una orden de lanzamiento, así que no puede usarlo.',
      program: null,
    };
  }

  // El ejecutable, si lo hay, está ANTES de `%command%`: detrás solo van
  // argumentos del juego (`-novid`, `-dx11`…), que no son programas. Buscar en
  // toda la línea lo convertía en un falso positivo con cualquier bandera.
  const tokens: string[] = text.split(/\s+/);
  const tiene_marcador = tokens.includes('%command%');
  // Sin marcador no es una línea de Steam válida, pero se valida igual: en ese
  // caso el comando empieza directamente por el programa.
  const marcador = tokens.indexOf('%command%');
  const cabeza: string[] = marcador >= 0 ? tokens.slice(0, marcador) : tokens;

  const first = cabeza.find((t) => !t.includes('='));

  if (first === undefined && !tiene_marcador) {
    // Solo asignaciones de entorno y ningún `%command%`: no hay nada que Steam
    // pueda lanzar.
    return {
      safe: false,
      reason: 'El comando no lleva %command%, así que Steam no sabría qué lanzar.',
      program: null,
    };
  }

  if (first === undefined) {
    // Solo variables de entorno + `%command%`: no hay ejecutable propio.
    return { safe: true, reason: '', program: null };
  }

  const program = first.split('/').pop() || first;

  if (!ALLOWED_COMMANDS.has(program)) {
    return {
      safe: false,
      reason:
        `«${program}» no es un lanzador conocido de Thoriapp. Solo se aceptan ` +
        'envoltorios de juegos conocidos (gamemoderun, mangohud, gamescope…) y ' +
        'el marcador %command%.',
      program,
    };
  }

  // Rutas absolutas y `..`: un comando legítimo se resuelve por PATH o es
  // `%command%`. Una ruta concreta apunta a un binario arbitrario del disco.
  // Se mira la cabeza (programa y envoltorios) y no los argumentos del juego:
  // `-w 1920` es una bandera, no una ruta.
  const hasAbsolutePath = cabeza.some((t) => t.startsWith('/') || t.startsWith('~'));
  if (hasAbsolutePath) {
    return {
      safe: false,
      reason:
        'El comando usa una ruta del sistema. Thoriapp solo acepta órdenes que se ' +
        'resuelven por PATH para no lanzar un binario cualquiera.',
      program,
    };
  }

  return { safe: true, reason: '', program };
}

/**
 * Construye el comando de lanzamiento de un perfil a partir de sus datos y del
 * estado actual del juego (reglas, recomendaciones, motor).
 *
 * Función PURA: todo lo que necesita se lo pasan. Antes estas ~50 líneas
 * estaban duplicadas dentro de App.tsx (en loadAllProfiles y en loadProfile),
 * lo que hacía que cualquier cambio hubiera que hacerlo dos veces.
 */
export function buildProfileCommand(
  profile: Profile,
  rules: GameRules,
  communityRecs: CommunityRecommendation[],
  engine: GameEngine | null,
  engineRecs: string[]
): string {
  const parts: CommandParts = { envVars: [], wrappers: [], args: [] };

  // Un comando manual manda sobre lo demás, pero NO se devuelve a ciegas: si el
  // perfil llegó importado, este texto no lo ha escrito la app. Si no pasa la
  // comprobación se ignora y se sigue construyendo el comando normal, que es
  // seguro por construcción (sale de listas fijas). As\u00ed un perfil manipulado
  // no puede dejar un comando raro por el simple hecho de estar activo.
  if (profile.use_manual_command && profile.manual_command) {
    const manual = profile.manual_command.trim();
    if (checkManualCommand(manual).safe) return manual;
  }

  // 1) Parámetros del juego
  rules.params.forEach((p, i) => {
    if (!profile.enabled_params[i]) return;
    if (p.kind === 'env') parts.envVars.push(p.value);
    else if (p.kind === 'wrapper') parts.wrappers.push(p.value);
    // `config` es un ajuste dentro del juego (no un parámetro del lanzador) y sus
    // textos llevan espacios: dentro del comando lo dejarían inválido.
    else if (p.kind === 'arg') parts.args.push(p.value);
  });

  // 2) Recomendaciones de la comunidad (filtradas por hardware)
  communityRecs.forEach((rec, i) => {
    if (!profile.selected_community_recs.includes(i)) return;
    const kind = classifyParameter(rec.parameter);
    if (kind === 'env') parts.envVars.push(rec.parameter);
    else if (kind === 'wrapper') parts.wrappers.push(rec.parameter);
    else parts.args.push(rec.parameter);
  });

  // 3) Recomendaciones del motor del juego
  if (engine) {
    engineRecs.forEach((rec, i) => {
      if (profile.selected_engine_recs.includes(i)) {
        parts.envVars.push(rec);
      }
    });
  }

  return joinCommand(parts);
}
