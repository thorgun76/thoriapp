import type { CommunityRecommendation, GameEngine, GameRules, Profile } from './types';

/** Envoltorios que van ANTES del `%command%`. */
const KNOWN_WRAPPERS = new Set(['gamemoderun', 'mangohud', 'gamescope']);

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
  if (KNOWN_WRAPPERS.has(parameter)) return 'wrapper';
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

  if (profile.use_manual_command && profile.manual_command) {
    return profile.manual_command;
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
