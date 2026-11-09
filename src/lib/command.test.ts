import { describe, expect, it } from 'vitest';
import {
  buildProfileCommand,
  classifyParameter,
  isEnvParameter,
  joinCommand,
} from './command';
import type { CommunityRecommendation, GameEngine, GameRules, Param, Profile } from './types';

/** Parámetro mínimo válido; cada test sobrescribe solo lo que le importa. */
const param = (over: Partial<Param> = {}): Param => ({
  kind: 'env',
  value: 'DXVK_HUD=0',
  title: '',
  why: '',
  impact: 'low',
  requires: [],
  default_on: false,
  evidence: 'docs',
  proton_only: false,
  ...over,
});

const rules = (params: Param[]): GameRules => ({
  name: 'Test',
  appid: 1,
  bottleneck: '',
  summary: '',
  params,
  tips: [],
  has_rules: true,
});

const profile = (over: Partial<Profile> = {}): Profile => ({
  id: 'p1',
  name: 'Perfil',
  appid: 1,
  game_name: 'Juego',
  enabled_params: [],
  selected_community_recs: [],
  selected_engine_recs: [],
  force_proton: false,
  expert_mode: false,
  use_manual_command: false,
  created_at: '',
  updated_at: '',
  ...over,
});

const engine: GameEngine = { name: 'Unity', version: '2022', api: 'vulkan' };

describe('isEnvParameter', () => {
  it('detecta variables de entorno por el signo igual', () => {
    expect(isEnvParameter('DXVK_HUD=0')).toBe(true);
    expect(isEnvParameter('WINE_FULLSCREEN_FSR=1')).toBe(true);
  });

  it('detecta variables de entorno por prefijo, aunque no lleven "="', () => {
    for (const p of ['PROTON_NO_ESYNC', 'DXVK_ASYNC', 'VKD3D_X', 'WINE_Y', 'AMD_Z', 'RADV_W', '__GL_SHADER_DISK_CACHE']) {
      expect(isEnvParameter(p), p).toBe(true);
    }
  });

  it('no confunde argumentos con variables de entorno', () => {
    expect(isEnvParameter('-novid')).toBe(false);
    expect(isEnvParameter('+fps_max 120')).toBe(false);
    expect(isEnvParameter('gamemoderun')).toBe(false);
  });
});

describe('classifyParameter', () => {
  it('clasifica variables de entorno', () => {
    expect(classifyParameter('DXVK_HUD=fps')).toBe('env');
  });

  it('reconoce los envoltorios conocidos', () => {
    for (const w of ['gamemoderun', 'mangohud', 'gamescope']) {
      expect(classifyParameter(w), w).toBe('wrapper');
    }
  });

  it('todo lo demás es argumento', () => {
    expect(classifyParameter('-novid')).toBe('arg');
    expect(classifyParameter('+fps_max')).toBe('arg');
    // Un envoltorio desconocido se trata como argumento, no como wrapper
    expect(classifyParameter('prime-run')).toBe('arg');
  });
});

describe('joinCommand', () => {
  it('respeta el orden canónico: env, wrappers, %command%, args', () => {
    const cmd = joinCommand({
      envVars: ['DXVK_HUD=fps'],
      wrappers: ['gamemoderun', 'mangohud'],
      args: ['-novid', '+fps_max 120'],
    });
    expect(cmd).toBe('DXVK_HUD=fps gamemoderun mangohud %command% -novid +fps_max 120');
  });

  it('sin ajustes devuelve solo %command%', () => {
    expect(joinCommand({ envVars: [], wrappers: [], args: [] })).toBe('%command%');
  });
});

describe('buildProfileCommand', () => {
  const baseRules = rules([
    param({ kind: 'env', value: 'PROTON_NO_ESYNC=1' }),
    param({ kind: 'wrapper', value: 'gamemoderun' }),
    param({ kind: 'arg', value: '-novid' }),
  ]);

  it('incluye solo los parámetros activados', () => {
    const cmd = buildProfileCommand(profile({ enabled_params: [false, true, false] }), baseRules, [], null, []);
    expect(cmd).toBe('gamemoderun %command%');
  });

  it('respeta el modo manual por encima de todo lo demás', () => {
    const cmd = buildProfileCommand(
      profile({
        use_manual_command: true,
        manual_command: 'MI_COMANDO=1 %command% --custom',
        enabled_params: [true, true, true],
      }),
      baseRules,
      [],
      null,
      []
    );
    expect(cmd).toBe('MI_COMANDO=1 %command% --custom');
  });

  it('si use_manual_command está activo pero no hay texto, usa la selección', () => {
    const cmd = buildProfileCommand(
      profile({ use_manual_command: true, manual_command: undefined, enabled_params: [true, false, false] }),
      baseRules,
      [],
      null,
      []
    );
    expect(cmd).toBe('PROTON_NO_ESYNC=1 %command%');
  });

  it('clasifica las recomendaciones de la comunidad en el grupo correcto', () => {
    const recs: CommunityRecommendation[] = [
      { parameter: 'DXVK_ASYNC=1', description: '', optimized_for: [] },
      { parameter: 'mangohud', description: '', optimized_for: [] },
      { parameter: '-dx11', description: '', optimized_for: [] },
    ];
    const cmd = buildProfileCommand(
      profile({ selected_community_recs: [0, 1, 2] }),
      rules([]),
      recs,
      null,
      []
    );
    expect(cmd).toBe('DXVK_ASYNC=1 mangohud %command% -dx11');
  });

  it('ignora las recomendaciones de la comunidad no seleccionadas', () => {
    const recs: CommunityRecommendation[] = [
      { parameter: 'DXVK_ASYNC=1', description: '', optimized_for: [] },
      { parameter: 'mangohud', description: '', optimized_for: [] },
    ];
    const cmd = buildProfileCommand(profile({ selected_community_recs: [1] }), rules([]), recs, null, []);
    expect(cmd).toBe('mangohud %command%');
  });

  it('las recomendaciones del motor son variables de entorno', () => {
    const cmd = buildProfileCommand(
      profile({ selected_engine_recs: [0, 2] }),
      rules([]),
      [],
      engine,
      ['PROTON_FORCE_LARGE_ADDRESS_AWARE=1', 'IGNORADA=1', 'WINE_HIDE_NVIDIA_GPU=1']
    );
    expect(cmd).toBe('PROTON_FORCE_LARGE_ADDRESS_AWARE=1 WINE_HIDE_NVIDIA_GPU=1 %command%');
  });

  it('sin motor, las recomendaciones de motor se ignoran aunque estén marcadas', () => {
    const cmd = buildProfileCommand(profile({ selected_engine_recs: [0] }), rules([]), [], null, ['X=1']);
    expect(cmd).toBe('%command%');
  });

  it('ignora índices de enabled_params fuera de rango sin romperse', () => {
    // enabled_params más corto que params: los que faltan cuentan como false.
    // Es el caso que provocaba nulls al serializar en el frontend.
    const cmd = buildProfileCommand(profile({ enabled_params: [true] }), baseRules, [], null, []);
    expect(cmd).toBe('PROTON_NO_ESYNC=1 %command%');
  });

  it('combina las tres fuentes en una sola línea', () => {
    const recs: CommunityRecommendation[] = [
      { parameter: 'DXVK_HUD=fps', description: '', optimized_for: [] },
    ];
    const cmd = buildProfileCommand(
      profile({
        enabled_params: [false, true, true],
        selected_community_recs: [0],
        selected_engine_recs: [0],
      }),
      baseRules,
      recs,
      engine,
      ['PROTON_FORCE_LARGE_ADDRESS_AWARE=1']
    );
    expect(cmd).toBe(
      'DXVK_HUD=fps PROTON_FORCE_LARGE_ADDRESS_AWARE=1 gamemoderun %command% -novid'
    );
  });
});

describe('buildProfileCommand: parámetros que no son del lanzador', () => {
  it('un ajuste del juego (`config`) nunca acaba dentro del comando', () => {
    // Los perfiles por juego traen consejos tipo «usa FSR en Calidad»: llevan
    // espacios y, si se pegaran al comando, el lanzador de Steam lo rechazaría.
    const cmd = buildProfileCommand(
      profile({ enabled_params: [true, true] }),
      rules([
        param({ kind: 'env', value: 'DXVK_HUD=0' }),
        param({ kind: 'config', value: 'Activa FSR dentro del juego' }),
      ]),
      [],
      null,
      []
    );

    expect(cmd).toBe('DXVK_HUD=0 %command%');
    expect(cmd).not.toContain('FSR');
  });

  it('un argumento de verdad (`arg`) sí va detrás de %command%', () => {
    const cmd = buildProfileCommand(
      profile({ enabled_params: [true] }),
      rules([param({ kind: 'arg', value: '-novid' })]),
      [],
      null,
      []
    );
    expect(cmd).toBe('%command% -novid');
  });
});
