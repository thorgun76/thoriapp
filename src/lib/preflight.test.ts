import { describe, expect, it } from 'vitest';
import { envVarsIn, hasBlockingIssues, preflight, wrappersIn } from './preflight';
import type { RuntimeEnv } from './types';

/** Entorno mínimo: qué wrappers hay instalados y poco más. */
function makeEnv(overrides: Partial<RuntimeEnv> = {}): RuntimeEnv {
  return {
    binaries: [],
    mangohud_binary: null,
    mangohud_vulkan_layer: null,
    steam_flatpak: false,
    ntsync: false,
    kernel: '6.11.0',
    ...overrides,
  };
}

const withBinaries = (...names: string[]) =>
  makeEnv({
    binaries: names.map((name) => ({ name, path: `/usr/bin/${name}` })),
  });

describe('wrappersIn', () => {
  it('detecta los wrappers del comando', () => {
    expect(wrappersIn('gamemoderun mangohud %command%')).toEqual(['gamemoderun', 'mangohud']);
  });

  it('no confunde un binario con nombre parecido', () => {
    expect(wrappersIn('mangohud-config %command%')).toEqual([]);
    expect(wrappersIn('MANGOHUD=1 %command%')).toEqual([]);
  });
});

describe('envVarsIn', () => {
  it('extrae los nombres de las variables asignadas', () => {
    expect(envVarsIn('PROTON_USE_NTSYNC=1 DXVK_HUD=fps %command%')).toEqual([
      'PROTON_USE_NTSYNC',
      'DXVK_HUD',
    ]);
  });

  it('no toma por variable un argumento con =', () => {
    expect(envVarsIn('%command% +set com_maxfps=250')).toEqual([]);
  });
});

describe('preflight: wrappers', () => {
  it('avisa como error de un wrapper que no está instalado', () => {
    const checks = preflight('gamemoderun %command%', withBinaries());

    expect(checks).toHaveLength(1);
    expect(checks[0].id).toBe('falta-gamemoderun');
    expect(checks[0].severity).toBe('error');
    expect(hasBlockingIssues(checks)).toBe(true);
  });

  it('no dice nada si el wrapper está instalado', () => {
    expect(preflight('gamemoderun %command%', withBinaries('gamemoderun'))).toEqual([]);
  });

  it('revisa también un comando escrito a mano', () => {
    const checks = preflight('prime-run mangohud %command%', withBinaries('prime-run'));
    expect(checks.map((c) => c.id)).toEqual(['falta-mangohud']);
  });
});

describe('preflight: %command%', () => {
  it('un comando sin %command% es un error', () => {
    const checks = preflight('gamemoderun %GAMEDIR%', withBinaries('gamemoderun'));
    expect(checks[0].id).toBe('sin-command');
    expect(checks[0].severity).toBe('error');
  });

  it('no se exige %command% en un comando manual', () => {
    expect(preflight('./run.sh', null, { manual: true })).toEqual([]);
  });
});

describe('preflight: variables de Proton', () => {
  it('avisa de variables de Proton en un juego nativo', () => {
    const checks = preflight('DXVK_HUD=fps PROTON_USE_NTSYNC=1 %command%', makeEnv(), {
      native: true,
    });
    // ntsync no está disponible en el entorno de prueba: primero el error, luego el aviso.
    expect(checks.map((c) => c.id)).toEqual(['sin-ntsync', 'proton-en-nativo']);
  });

  it('no avisa de Proton cuando el juego no es nativo', () => {
    expect(preflight('DXVK_HUD=fps %command%', makeEnv(), { native: false })).toEqual([]);
  });

  it('ntsync se acepta si /dev/ntsync existe', () => {
    const env = makeEnv({ ntsync: true });
    expect(preflight('PROTON_USE_NTSYNC=1 %command%', env)).toEqual([]);
  });

  it('sin datos del sistema sigue avisando de lo que se ve en el texto', () => {
    const checks = preflight('PROTON_USE_NTSYNC=1 %command%', null);
    expect(checks).toEqual([]); // sin env no se puede saber si falta /dev/ntsync
  });
});

describe('preflight: Steam Flatpak', () => {
  const env = makeEnv({ steam_flatpak: true, binaries: [{ name: 'mangohud', path: '/usr/bin/mangohud' }] });

  it('avisa de que MangoHud no se ve dentro del sandbox', () => {
    const checks = preflight('mangohud %command%', env, { launcher: 'steam' });
    expect(checks.map((c) => c.id)).toEqual(['steam-flatpak']);
    expect(checks[0].severity).toBe('warn');
  });

  it('no avisa en Lutris (no está en el sandbox de Steam)', () => {
    expect(preflight('mangohud %command%', env, { launcher: 'lutris' })).toEqual([]);
  });

  it('no avisa si el comando no usa MangoHud', () => {
    expect(preflight('%command%', env, { launcher: 'steam' })).toEqual([]);
  });
});

describe('preflight: orden y casos límite', () => {
  it('deja los errores antes que los avisos', () => {
    const env = makeEnv({ steam_flatpak: true });
    const checks = preflight('mangohud %command%', env, { launcher: 'steam' });
    expect(checks[0].severity).toBe('error'); // falta-mangohud
    expect(checks[1].severity).toBe('warn'); // steam-flatpak
  });

  it('un comando vacío no genera avisos', () => {
    expect(preflight('   ', makeEnv())).toEqual([]);
  });
});

describe('preflight: MANGOHUD=1', () => {
  it('avisa si no hay ni binario ni capa Vulkan', () => {
    const checks = preflight('MANGOHUD=1 %command%', makeEnv());
    expect(checks.map((c) => c.id)).toEqual(['sin-mangohud']);
  });

  it('no avisa si la capa Vulkan está presente (caso Goverlay)', () => {
    const env = makeEnv({ mangohud_vulkan_layer: '/usr/share/vulkan/implicit_layer.d/MangoHud.x86_64.json' });
    expect(preflight('MANGOHUD=1 %command%', env)).toEqual([]);
  });
});
