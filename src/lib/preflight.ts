import type { RuntimeEnv } from './types';

/**
 * Revisa el comando de lanzamiento ANTES de que el usuario lo pegue en el lanzador y
 * detecta los fallos que se pagan con un "el juego no arranca":
 *
 * - un wrapper que no está instalado (`gamemoderun` en un equipo sin GameMode),
 * - variables de Proton en un juego nativo (no hacen nada),
 * - `PROTON_USE_NTSYNC=1` sin `/dev/ntsync`,
 * - MangoHud en un Steam Flatpak, donde tiene que estar dentro del sandbox,
 * - un comando sin `%command%`, que Steam no sabe qué lanzar.
 *
 * Función PURA: el estado del sistema entra como parámetro (`RuntimeEnv`), así se puede
 * probar sin depender de lo que haya instalado en la máquina que ejecuta los tests.
 */

export type Severity = 'error' | 'warn' | 'info';

export interface PreflightCheck {
  /** Identificador estable (para tests y para no repetir avisos). */
  id: string;
  severity: Severity;
  title: string;
  detail: string;
  /** Qué hacer para arreglarlo. */
  hint?: string;
}

export interface PreflightContext {
  /** El juego tiene build nativo de Linux (las variables de Proton no aplican). */
  native?: boolean;
  /** Launcher de origen: `steam`, `lutris`, `heroic`, `heronite`… */
  launcher?: string;
  /** El comando lo escribió el usuario a mano (no exigimos `%command%`). */
  manual?: boolean;
}

/**
 * Wrappers que se buscan en el comando. Es más amplia que la de `command.ts` (que
 * clasifica parámetros al construir): aquí se revisan también comandos escritos a mano.
 */
export const CHECKED_WRAPPERS = [
  'gamemoderun',
  'mangohud',
  'gamescope',
  'prime-run',
  'vkbasalt',
  'obs-gamecapture',
  'game-performance',
  'scx_lavd',
] as const;

const WRAPPER_HINTS: Record<string, string> = {
  gamemoderun: 'Instala el paquete `gamemode` (Debian/Ubuntu: sudo apt install gamemode).',
  mangohud: 'Instala el paquete `mangohud`. Si la capa Vulkan sí está presente, puedes usar MANGOHUD=1 en lugar del wrapper.',
  gamescope:
    'Instala el paquete `gamescope`. En NVIDIA necesita además los drivers con soporte de Vulkan explícito.',
  'prime-run': 'Viene con `nvidia-prime` (Ubuntu/Pop!_OS). En Arch se usa `prime-run` de `nvidia-prime`.',
  vkbasalt: 'Instala el paquete `vkbasalt` (y su capa Vulkan).',
  'obs-gamecapture': 'Viene con `obs-studio`.',
  'game-performance':
    'Requiere el `game-performance` de Pop!_OS/CachyOS (system76-scheduler o el script equivalente).',
  scx_lavd: 'Viene con los schedulers `scx` (paquete scx-scheds).',
};

/** Variables que solo tienen efecto cuando el juego corre bajo Proton/Wine. */
const PROTON_ONLY_ENV = /^(PROTON|DXVK|VKD3D)_[A-Z0-9_]+$/;

const escapeRegExp = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** Nombres de los wrappers presentes en el comando (palabra completa, sin confundir `mangohud-config`). */
export function wrappersIn(command: string): string[] {
  return CHECKED_WRAPPERS.filter((w) =>
    new RegExp(`(^|[\\s;|&])${escapeRegExp(w)}(?=$|[\\s;|&])`).test(command)
  );
}

/** Nombres de las variables de entorno asignadas en el comando (`FOO=1 bar`). */
export function envVarsIn(command: string): string[] {
  const found = new Set<string>();
  const tokens = command.split(/\s+/);

  tokens.forEach((token, i) => {
    const match = /^([A-Za-z_][A-Za-z0-9_]*)=/.exec(token);
    if (!match) return;
    // `+set com_maxfps=250` no es una variable: es un argumento del juego tras un flag.
    const previous = tokens[i - 1];
    if (previous && /^[+-]/.test(previous)) return;
    found.add(match[1]);
  });

  return [...found];
}

const SEVERITY_ORDER: Record<Severity, number> = { error: 0, warn: 1, info: 2 };

/**
 * Comprobaciones sobre el comando final. Devuelve la lista ordenada (errores primero).
 * Si `env` es `null` (aún cargando o backend caído) solo se avisa de lo que se puede
 * saber mirando el texto del comando.
 */
export function preflight(
  command: string,
  env: RuntimeEnv | null,
  ctx: PreflightContext = {}
): PreflightCheck[] {
  const checks: PreflightCheck[] = [];
  const trimmed = command.trim();
  if (!trimmed) return checks;

  const wrappers = wrappersIn(trimmed);
  const vars = envVarsIn(trimmed);

  // 1) Sin %command% el lanzador no sabe qué ejecutar (Steam lo sustituye por el juego).
  if (!ctx.manual && !trimmed.includes('%command%')) {
    checks.push({
      id: 'sin-command',
      severity: 'error',
      title: 'Falta %command%',
      detail:
        'Es el marcador que el lanzador sustituye por el juego. Sin él, el comando no lanza nada.',
      hint: 'Un comando válido termina en `%command%`, por ejemplo: gamemoderun mangohud %command%',
    });
  }

  // 2) Wrappers que el sistema no tiene.
  if (env) {
    const installed = new Set(env.binaries.map((b) => b.name));
    for (const wrapper of wrappers) {
      if (installed.has(wrapper)) continue;
      checks.push({
        id: `falta-${wrapper}`,
        severity: 'error',
        title: `«${wrapper}» no está instalado`,
        detail: `El lanzador no encontrará ese comando y el juego no arrancará.`,
        hint: WRAPPER_HINTS[wrapper],
      });
    }
  }

  // 3) Variables de Proton en un juego que no usa Proton.
  if (ctx.native === true) {
    const protonVars = vars.filter((v) => PROTON_ONLY_ENV.test(v));
    if (protonVars.length > 0) {
      checks.push({
        id: 'proton-en-nativo',
        severity: 'warn',
        title: 'Variables de Proton en un juego nativo',
        detail: `${protonVars.join(', ')} solo afectan a juegos que corren bajo Proton/Wine. Aquí no hacen nada.`,
        hint: 'Quita esas variables: el juego ya corre en Linux de forma nativa.',
      });
    }
  }

  // 4) ntsync pedido en un sistema sin /dev/ntsync.
  const usesNtsync = vars.includes('PROTON_USE_NTSYNC') || vars.includes('PROTON_NTSYNC');
  if (env && usesNtsync && !env.ntsync) {
    checks.push({
      id: 'sin-ntsync',
      severity: 'error',
      title: 'ntsync no está disponible',
      detail: `Activaste ntsync pero no existe /dev/ntsync en este sistema${
        env.kernel ? ` (kernel ${env.kernel})` : ''
      }.`,
      hint: 'Necesita kernel 6.14 o superior con el módulo ntsync. Quita la variable o actualiza el kernel.',
    });
  }

  // 5) Steam Flatpak: MangoHud tiene que existir DENTRO del sandbox.
  const wantsMangohud = wrappers.includes('mangohud') || vars.includes('MANGOHUD');
  if (env?.steam_flatpak && ctx.launcher === 'steam' && wantsMangohud) {
    checks.push({
      id: 'steam-flatpak',
      severity: 'warn',
      title: 'Steam es Flatpak',
      detail:
        'El comando se ejecuta dentro del sandbox de Steam, así que no ve el MangoHud del sistema.',
      hint: 'Instala `org.freedesktop.Platform.VulkanLayer.MangoHud` con flatpak o usa Steam nativo.',
    });
  }

  // 6) `MANGOHUD=1` sin nada instalado: el HUD simplemente no aparece.
  if (
    env &&
    vars.includes('MANGOHUD') &&
    !wrappers.includes('mangohud') &&
    !env.mangohud_binary &&
    !env.mangohud_vulkan_layer
  ) {
    checks.push({
      id: 'sin-mangohud',
      severity: 'warn',
      title: 'MangoHud no está instalado',
      detail: 'Se pide el HUD con MANGOHUD=1, pero no hay ni binario ni capa Vulkan de MangoHud.',
      hint: 'Instala el paquete `mangohud` (de ahí sale la capa Vulkan que hace que MANGOHUD=1 funcione).',
    });
  }

  return checks.sort((a, b) => SEVERITY_ORDER[a.severity] - SEVERITY_ORDER[b.severity]);
}

/** ¿Hay algo que impida lanzar el juego? */
export const hasBlockingIssues = (checks: PreflightCheck[]): boolean =>
  checks.some((c) => c.severity === 'error');
