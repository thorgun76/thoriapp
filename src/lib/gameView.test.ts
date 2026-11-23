import { describe, expect, it, vi } from 'vitest';
import {
  filterParams,
  quickCommandFor,
  impactsPresent,
  isDirty,
  recallTab,
  rememberTab,
  snapshotOf,
  summarizeImpacts,
} from './gameView';
import type { Param } from './types';

const param = (overrides: Partial<Param> = {}): Param => ({
  kind: 'env',
  value: 'DXVK_HUD=1',
  title: 'HUD',
  why: 'Muestra los FPS.',
  impact: 'bajo',
  requires: [],
  default_on: false,
  evidence: 'community',
  proton_only: false,
  ...overrides,
});

const normalize = (t: string) => t.toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '');

describe('summarizeImpacts', () => {
  it('sin ajustes activos lo dice y no inventa riesgo', () => {
    expect(summarizeImpacts([param()], [false])).toEqual({
      count: 0,
      worst: null,
      label: 'Sin ajustes activos',
      tone: 'none',
    });
  });

  it('cuenta los activos y se queda con el peor impacto', () => {
    // Un ajuste inocuo y uno peligroso: el resumen debe avisar del peligroso.
    const params = [
      param({ impact: 'bajo', title: 'A' }),
      param({ impact: 'evitar', title: 'B' }),
      param({ impact: 'medio', title: 'C' }),
    ];
    const s = summarizeImpacts(params, [true, true, false]);

    expect(s.count).toBe(2);
    expect(s.worst).toBe('evitar');
    expect(s.label).toBe('2 ajustes activos · riesgo evitar');
    expect(s.tone).toBe('danger');
  });

  it('singular cuando solo hay uno', () => {
    expect(summarizeImpacts([param({ impact: 'bajo' })], [true]).label).toBe(
      '1 ajuste activo · riesgo bajo'
    );
  });

  it('los impactos inocuos van en verde y los medios en ámbar', () => {
    expect(summarizeImpacts([param({ impact: 'bajo' })], [true]).tone).toBe('ok');
    expect(summarizeImpacts([param({ impact: 'medio' })], [true]).tone).toBe('caution');
    expect(summarizeImpacts([param({ impact: 'medicion' })], [true]).tone).toBe('caution');
    expect(summarizeImpacts([param({ impact: 'dudoso' })], [true]).tone).toBe('danger');
  });

  it('tolera huecos en `enabled` (arrays más cortos que params)', () => {
    // Regresión: `enabled` se dimensiona en un efecto; un render intermedio puede
    // llegar con el array vacío y no debe romper el encabezado.
    expect(summarizeImpacts([param(), param()], []).count).toBe(0);
    expect(summarizeImpacts([param(), param()], [true]).count).toBe(1);
  });
});

describe('filterParams', () => {
  const params = [
    param({ title: 'FSR en Calidad', value: 'WINE_FSR=1', impact: 'medio' }),
    param({ title: 'HUD', value: 'DXVK_HUD=1', impact: 'bajo' }),
    param({ title: 'Ray tracing', value: 'DXR=1', impact: 'evitar', why: 'Añade mucha carga.' }),
  ];

  it('sin consulta ni impacto devuelve todo', () => {
    expect(filterParams(params, '', null, normalize)).toHaveLength(3);
  });

  it('busca por título, valor y explicación', () => {
    expect(filterParams(params, 'fsr', null, normalize).map((p) => p.title)).toEqual([
      'FSR en Calidad',
    ]);
    expect(filterParams(params, 'dxvk', null, normalize).map((p) => p.title)).toEqual(['HUD']);
    expect(filterParams(params, 'carga', null, normalize).map((p) => p.title)).toEqual([
      'Ray tracing',
    ]);
  });

  it('ignora acentos y mayúsculas', () => {
    expect(filterParams(params, 'EXPLICACIÓN', null, normalize)).toHaveLength(0);
    expect(filterParams(params, 'ra', null, normalize).map((p) => p.title)).toContain(
      'Ray tracing'
    );
  });

  it('combina con el filtro de impacto', () => {
    expect(filterParams(params, '', 'evitar', normalize).map((p) => p.title)).toEqual([
      'Ray tracing',
    ]);
    expect(filterParams(params, 'hud', 'evitar', normalize)).toHaveLength(0);
  });
});

describe('impactsPresent', () => {
  it('lista los impactos de peor a mejor, sin repetir', () => {
    const params = [
      param({ impact: 'bajo' }),
      param({ impact: 'evitar' }),
      param({ impact: 'bajo' }),
      param({ impact: 'medio' }),
    ];

    expect(impactsPresent(params)).toEqual(['evitar', 'medio', 'bajo']);
  });
});

describe('pestaña recordada por juego', () => {
  const fakeStore = () => {
    const data = new Map<string, string>();
    return {
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
    };
  };

  it('sin nada guardado abre Optimización', () => {
    expect(recallTab(440, fakeStore())).toBe('recommendations');
  });

  it('recuerda la pestaña de cada juego por separado', () => {
    const store = fakeStore();
    rememberTab(440, 'compatibility', store);

    expect(recallTab(440, store)).toBe('compatibility');
    // Otro juego no hereda la elección del anterior.
    expect(recallTab(730, store)).toBe('recommendations');
  });

  it('un valor corrupto no rompe la vista', () => {
    const store = { getItem: () => 'loquesea', setItem: vi.fn() };

    expect(recallTab(440, store)).toBe('recommendations');
  });

  it('un localStorage bloqueado no lanza', () => {
    const broken = {
      getItem: () => {
        throw new Error('bloqueado');
      },
      setItem: () => {
        throw new Error('bloqueado');
      },
    };

    expect(recallTab(440, broken)).toBe('recommendations');
    expect(() => rememberTab(440, 'compatibility', broken)).not.toThrow();
  });
});

describe('isDirty', () => {
  const base = {
    enabled: [false, true],
    selectedCommunityRecs: new Set<number>(),
    selectedEngineRecs: new Set<number>(),
    forceProton: false,
    manualCommand: '',
    useManualCommand: false,
  };

  it('sin perfil guardado no hay nada que perder', () => {
    expect(isDirty(snapshotOf(base), null)).toBe(false);
  });

  it('estado idéntico no está sucio', () => {
    expect(isDirty(snapshotOf(base), snapshotOf(base))).toBe(false);
  });

  it('activar un parámetro marca cambios sin guardar', () => {
    const saved = snapshotOf(base);
    const changed = snapshotOf({ ...base, enabled: [true, true] });

    expect(isDirty(changed, saved)).toBe(true);
  });

  it('el orden en que se activaron las recomendaciones no cuenta', () => {
    const saved = snapshotOf({ ...base, selectedCommunityRecs: new Set([2, 1]) });
    const current = snapshotOf({ ...base, selectedCommunityRecs: new Set([1, 2]) });

    expect(isDirty(current, saved)).toBe(false);
  });

  it('los espacios sobrantes del comando manual no cuentan', () => {
    const saved = snapshotOf({ ...base, useManualCommand: true, manualCommand: '%command%' });
    const current = snapshotOf({ ...base, useManualCommand: true, manualCommand: '  %command%  ' });

    expect(isDirty(current, saved)).toBe(false);
  });

  it('cambiar el modo experto no cuenta como cambio del perfil', () => {
    // `expertMode` solo afecta a lo que se ve, no al comando generado.
    const saved = snapshotOf(base);
    expect(isDirty(snapshotOf({ ...base }), saved)).toBe(false);
  });
});

describe('quickCommandFor', () => {
  const profiles = [
    { id: 'a', appid: 440 },
    { id: 'b', appid: 730 },
  ];

  it('sin perfiles guardados no ofrece copiar', () => {
    expect(quickCommandFor(1623730, profiles, {})).toBeNull();
  });

  it('devuelve el comando del perfil guardado de ese juego', () => {
    const commands = { a: 'DXVK_HUD=1 %command%', b: 'MANGOHUD=1 %command%' };

    expect(quickCommandFor(440, profiles, commands)).toBe('DXVK_HUD=1 %command%');
  });

  it('no copia un %command% pelado: no aporta nada', () => {
    expect(quickCommandFor(440, profiles, { a: '%command%' })).toBeNull();
  });

  it('salta los perfiles sin comando y usa el siguiente', () => {
    const two = [
      { id: 'x', appid: 440 },
      { id: 'y', appid: 440 },
    ];

    expect(quickCommandFor(440, two, { y: 'gamemoderun %command%' })).toBe('gamemoderun %command%');
  });
});
