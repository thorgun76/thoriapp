import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { OptimizeTab } from './OptimizeTab';
import type { GameInfo, GameRules, Param } from '../lib/types';

afterEach(cleanup);

function param(overrides: Partial<Param> = {}): Param {
  return {
    kind: 'env',
    value: 'DXVK_HUD=1',
    title: 'Parámetro',
    why: 'Porque sí.',
    impact: 'medio',
    requires: [],
    default_on: false,
    evidence: 'community',
    essential: true,
    proton_only: false,
    ...overrides,
  };
}

const info: GameInfo = { name: 'Juego', native: false, errors: [] };

function renderTab(params: Param[], enabled: boolean[], overrides: Partial<Parameters<typeof OptimizeTab>[0]> = {}) {
  const rules = { name: 'Juego', appid: 1, bottleneck: 'cpu', params, has_rules: true } as GameRules;
  const onToggleRule = vi.fn();
  render(
    <OptimizeTab
      rules={rules}
      info={info}
      engine={null}
      engineRecommendations={[]}
      communityRecommendations={[]}
      enabled={enabled}
      selectedCommunityRecs={new Set()}
      selectedEngineRecs={new Set()}
      expertMode={false}
      onToggleExpertMode={vi.fn()}
      forceProton={false}
      showUnavailable
      onToggleRule={onToggleRule}
      onToggleCommunityRec={vi.fn()}
      onToggleEngineRec={vi.fn()}
      onToggleProton={vi.fn()}
      missing={() => null}
      getRecommendationStatus={() => 'ok'}
      impactColor={() => ''}
      evidenceLabel={() => ''}
      {...overrides}
    />
  );
  return { onToggleRule };
}

describe('OptimizeTab: distinción entre comando y ajustes del juego', () => {
  it('marca los ajustes internos como «En el juego»', () => {
    renderTab([param({ kind: 'config', title: 'Usa FSR en Calidad' })], [false]);

    expect(screen.getByText('🎮 En el juego')).toBeTruthy();
  });

  it('etiqueta qué tipo de pieza del comando es cada parámetro', () => {
    renderTab(
      [
        param({ kind: 'env', title: 'HUD', value: 'DXVK_HUD=1' }),
        param({ kind: 'wrapper', title: 'Gamescope', value: 'gamescope' }),
        param({ kind: 'arg', title: 'Sin vídeo', value: '-novid' }),
      ],
      [false, false, false]
    );

    expect(screen.getByText('variable')).toBeTruthy();
    expect(screen.getByText('envoltorio')).toBeTruthy();
    expect(screen.getByText('argumento')).toBeTruthy();
  });

  it('un ajuste interno no se puede togglear', () => {
    // Es un consejo con espacios en el texto: si entrara en el comando lo rompería.
    const { onToggleRule } = renderTab([param({ kind: 'config', title: 'Usa FSR' })], [false]);

    fireEvent.click(screen.getByText('Usa FSR').closest('div[class*="rounded-lg"]')!);

    expect(onToggleRule).not.toHaveBeenCalled();
  });

  it('un parámetro normal sí se togglea con el ratón', () => {
    const { onToggleRule } = renderTab([param({ title: 'HUD' })], [false]);

    fireEvent.click(screen.getByText('HUD').closest('[role="checkbox"]')!);

    expect(onToggleRule).toHaveBeenCalledWith(0);
  });

  it('y también con el teclado', () => {
    const { onToggleRule } = renderTab([param({ title: 'HUD' })], [false]);

    fireEvent.keyDown(screen.getByRole('checkbox'), { key: ' ' });

    expect(onToggleRule).toHaveBeenCalledWith(0);
  });

  it('el aviso se lee una sola vez aunque el parámetro esté activo', () => {
    renderTab([param({ title: 'dxr', warn: 'Activa ray tracing.' })], [true]);

    expect(screen.getAllByText('Activa ray tracing.')).toHaveLength(1);
  });
});

describe('OptimizeTab: conmutador de modo experto', () => {
  it('muestra el modo guiado con el interruptor apagado', () => {
    renderTab([param()], [false]);

    const sw = screen.getByRole('switch', { name: /modo experto/i });
    expect(sw.getAttribute('aria-checked')).toBe('false');
    expect(screen.getByText('Modo guiado')).toBeTruthy();
  });

  it('pulsar el interruptor avisa a App (no cambia el estado por su cuenta)', () => {
    // El estado vive en App: el sidebar y esta pestaña deben moverse juntos.
    const rules = { name: 'Juego', appid: 1, bottleneck: 'cpu', params: [param()], has_rules: true } as GameRules;
    const onToggleExpertMode = vi.fn();
    render(
      <OptimizeTab
        rules={rules}
        info={info}
        engine={null}
        engineRecommendations={[]}
        communityRecommendations={[]}
        enabled={[false]}
        selectedCommunityRecs={new Set()}
        selectedEngineRecs={new Set()}
        expertMode={false}
        onToggleExpertMode={onToggleExpertMode}
        forceProton={false}
        showUnavailable
        onToggleRule={vi.fn()}
        onToggleCommunityRec={vi.fn()}
        onToggleEngineRec={vi.fn()}
        onToggleProton={vi.fn()}
        missing={() => null}
        getRecommendationStatus={() => 'ok'}
        impactColor={() => ''}
        evidenceLabel={() => ''}
      />
    );

    fireEvent.click(screen.getByRole('switch', { name: /modo experto/i }));

    expect(onToggleExpertMode).toHaveBeenCalledTimes(1);
  });
});

describe('OptimizeTab: buscador de ajustes', () => {
  const params = [
    param({ title: 'Activa FSR', value: 'WINE_FSR=1', impact: 'medio' }),
    param({ title: 'HUD de FPS', value: 'DXVK_HUD=1', impact: 'bajo' }),
    // Todos los juegos del catálogo traen mangohud entre sus parámetros.
    param({ title: 'Mangohud', value: 'mangohud', impact: 'medicion' }),
  ];

  it('filtra la lista por texto', () => {
    renderTab(params, [false, false]);

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'fsr' } });

    expect(screen.getByText('Activa FSR')).toBeTruthy();
    expect(screen.queryByText('HUD de FPS')).toBeNull();
  });

  it('avisa cuando el filtro no deja nada', () => {
    renderTab(params, [false, false]);

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'zzz' } });

    expect(screen.getByText(/Ningún ajuste coincide/)).toBeTruthy();
  });

  it('el filtro por impacto se puede quitar', () => {
    renderTab(params, [false, false]);

    const chip = screen.getByRole('button', { name: 'bajo' });
    fireEvent.click(chip);
    expect(screen.queryByText('Activa FSR')).toBeNull();

    fireEvent.click(screen.getByText('Quitar filtros'));
    expect(screen.getByText('Activa FSR')).toBeTruthy();
  });

  it('limpiar la búsqueda devuelve todos los ajustes', () => {
    renderTab(params, [false, false]);

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'fsr' } });
    fireEvent.click(screen.getByLabelText('Limpiar la búsqueda'));

    expect(screen.getByText('HUD de FPS')).toBeTruthy();
  });
});

describe('OptimizeTab: el filtro oculta lo que no coincide', () => {
  const params = [
    param({ title: 'Activa FSR', value: 'WINE_FSR=1', impact: 'medio' }),
    param({ title: 'HUD de FPS', value: 'DXVK_HUD=1', impact: 'bajo' }),
    param({ title: 'Sincronización', value: 'gamemoderun', impact: 'medio' }),
    param({ title: 'Mangohud', value: 'mangohud', impact: 'medicion' }),
  ];

  it('las secciones que no coinciden desaparecen, no solo las listas largas', () => {
    // Regresión: buscar «fsr» dejaba MangoHud, Proton y el resto en pantalla, así
    // que el usuario escribía y no veía ningún cambio sin bajar a mirar.
    renderTab(params, [false, false, false, false], { info: { name: 'J', native: true, errors: [] } });

    expect(screen.getByText('Herramientas de diagnóstico')).toBeTruthy();
    expect(screen.getByText('Probar con Proton')).toBeTruthy();

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'fsr' } });

    expect(screen.getByText('Activa FSR')).toBeTruthy();
    expect(screen.queryByText('Herramientas de diagnóstico')).toBeNull();
    expect(screen.queryByText('Probar con Proton')).toBeNull();
    expect(screen.queryByText('HUD de FPS')).toBeNull();
  });

  it('«proton» deja el interruptor de Proton, que no es un parámetro', () => {
    renderTab(params, [false, false, false, false], { info: { name: 'J', native: true, errors: [] } });

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'proton' } });

    expect(screen.getByText('Probar con Proton')).toBeTruthy();
    expect(screen.queryByText('Activa FSR')).toBeNull();
  });

  it('«mangohud» deja su herramienta de diagnóstico', () => {
    renderTab(params, [false, false, false, false]);

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'mangohud' } });

    expect(screen.getByText('Herramientas de diagnóstico')).toBeTruthy();
    expect(screen.queryByText('Activa FSR')).toBeNull();
  });

  it('el índice de secciones se oculta: con filtro no hay secciones que saltar', () => {
    renderTab(params, [false, false, false, false]);

    expect(screen.getByRole('navigation', { name: /Secciones de esta pestaña/ })).toBeTruthy();

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'fsr' } });

    expect(screen.queryByRole('navigation', { name: /Secciones de esta pestaña/ })).toBeNull();
  });

  it('al quitar el filtro vuelve todo', () => {
    renderTab(params, [false, false, false, false], { info: { name: 'J', native: true, errors: [] } });

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'fsr' } });
    fireEvent.click(screen.getByText('Quitar filtros'));

    expect(screen.getByText('HUD de FPS')).toBeTruthy();
    expect(screen.getByText('Probar con Proton')).toBeTruthy();
    expect(screen.getByText('Herramientas de diagnóstico')).toBeTruthy();
  });

  it('las recomendaciones de motor y hardware también se filtran', () => {
    renderTab(params, [false, false, false], {
      engine: { name: 'Unreal', version: '5', api: 'vulkan' },
      engineRecommendations: ['NTSYNC=1', 'LARGE_ADDRESS_AWARE=1'],
      communityRecommendations: [
        { parameter: 'RADV_PERFTEST=aco', description: 'Mejora shaders', optimized_for: [], recommended: true },
      ],
    });

    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'ntsync' } });

    expect(screen.getByText('NTSYNC=1')).toBeTruthy();
    expect(screen.queryByText('LARGE_ADDRESS_AWARE=1')).toBeNull();
    expect(screen.queryByText('RADV_PERFTEST=aco')).toBeNull();
  });
});

describe('OptimizeTab: las etiquetas de impacto filtran de verdad', () => {
  // `mangohud` va como parámetro, igual que en games.json: su impacto real es
  // «medicion» y por eso la etiqueta debe poder dejarlo a él solo en pantalla.
  const params = [
    param({ title: 'SoloAlto', value: 'ALTO=1', impact: 'alto' }),
    param({ title: 'SoloBajo', value: 'BAJO=1', impact: 'bajo' }),
    param({ title: 'Mangohud', value: 'mangohud', impact: 'medicion' }),
  ];

  const withExtras = {
    info: { name: 'J', native: true, errors: [] } as GameInfo,
    engine: { name: 'UE', version: '5', api: 'vulkan' },
    engineRecommendations: ['NTSYNC=1'],
    communityRecommendations: [
      { parameter: 'RADV_PERFTEST=aco', description: 'd', optimized_for: [], recommended: true },
    ],
  };

  it('un impacto oculta los parámetros de otro nivel', () => {
    renderTab(params, [false, false, false], withExtras);

    fireEvent.click(screen.getByRole('button', { name: 'alto' }));

    expect(screen.getByText('SoloAlto')).toBeTruthy();
    expect(screen.queryByText('SoloBajo')).toBeNull();
  });

  it('un impacto también oculta lo que no es un parámetro', () => {
    // Regresión: Proton, MangoHud, motor y hardware solo miraban el texto, así que
    // con una etiqueta pulsada seguían todos en pantalla.
    renderTab(params, [false, false, false], withExtras);

    fireEvent.click(screen.getByRole('button', { name: 'alto' }));

    expect(screen.queryByText('Herramientas de diagnóstico')).toBeNull();
    expect(screen.queryByText('Probar con Proton')).toBeNull();
    expect(screen.queryByText('NTSYNC=1')).toBeNull();
    expect(screen.queryByText('RADV_PERFTEST=aco')).toBeNull();
  });

  it('sin filtro de impacto sí se ven las filas que no son parámetros', () => {
    renderTab(params, [false, false, false], withExtras);

    expect(screen.getByText('Herramientas de diagnóstico')).toBeTruthy();
    expect(screen.getByText('Probar con Proton')).toBeTruthy();
    expect(screen.getByText('NTSYNC=1')).toBeTruthy();
  });

  it('una recomendación de motor sale si su impacto coincide', () => {
    // El impacto de una recomendación es el del parámetro equivalente del juego.
    const conImpacto = [
      param({ title: 'Sync', value: 'NTSYNC=1', impact: 'medio' }),
    ];
    renderTab(conImpacto, [false], { ...withExtras, engineRecommendations: ['NTSYNC=1'] });

    fireEvent.click(screen.getByRole('button', { name: 'medio' }));

    expect(screen.getByText('NTSYNC=1')).toBeTruthy();
  });

  it('el impacto y el texto se combinan', () => {
    renderTab(params, [false, false, false], withExtras);

    fireEvent.click(screen.getByRole('button', { name: 'alto' }));
    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'bajo' } });

    expect(screen.queryByText('SoloAlto')).toBeNull();
    expect(screen.queryByText('SoloBajo')).toBeNull();
    expect(screen.getByText(/Ningún ajuste coincide/)).toBeTruthy();
  });

  it('volver a pulsar la etiqueta la quita', () => {
    renderTab(params, [false, false, false], withExtras);

    const chip = screen.getByRole('button', { name: 'alto' });
    fireEvent.click(chip);
    expect(screen.queryByText('SoloBajo')).toBeNull();

    fireEvent.click(chip);
    expect(screen.getByText('SoloBajo')).toBeTruthy();
  });
});

describe('OptimizeTab: la etiqueta «medicion» deja MangoHud (y no lo oculta)', () => {
  const params = [
    param({ title: 'Activa FSR', value: 'WINE_FSR=1', impact: 'medio' }),
    param({ title: 'Mangohud', value: 'mangohud', impact: 'medicion' }),
  ];

  it('MangoHud es un parámetro con impacto propio, así que la etiqueta lo deja ver', () => {
    // Regresión: MangoHud se ocultaba siempre al pulsar cualquier etiqueta, aunque
    // su impacto fuera justo «medicion» (el de games.json).
    renderTab(params, [false, false]);

    fireEvent.click(screen.getByRole('button', { name: 'medicion' }));

    expect(screen.getByText('Herramientas de diagnóstico')).toBeTruthy();
    expect(screen.queryByText('Activa FSR')).toBeNull();
  });

  it('y desaparece con una etiqueta que no es la suya', () => {
    renderTab(params, [false, false]);

    fireEvent.click(screen.getByRole('button', { name: 'medio' }));

    expect(screen.queryByText('Herramientas de diagnóstico')).toBeNull();
    expect(screen.getByText('Activa FSR')).toBeTruthy();
  });

  it('un juego sin mangohud entre sus parámetros no lo muestra al filtrar por impacto', () => {
    // Sin el parámetro no hay impacto que comparar: no se puede afirmar que coincida.
    renderTab([param({ title: 'Activa FSR', value: 'FSR=1', impact: 'medio' })], [false]);

    fireEvent.click(screen.getByRole('button', { name: 'medio' }));

    expect(screen.queryByText('Herramientas de diagnóstico')).toBeNull();
  });

  it('«mangohud» por texto sigue funcionando aunque haya etiqueta puesta', () => {
    renderTab(params, [false, false]);

    fireEvent.click(screen.getByRole('button', { name: 'medicion' }));
    fireEvent.change(screen.getByLabelText('Buscar ajuste'), { target: { value: 'mangohud' } });

    expect(screen.getByText('Herramientas de diagnóstico')).toBeTruthy();
  });
});

describe('OptimizeTab: «medicion» en ajustes del sistema', () => {
  it('game-performance es medición y sobrevive a su propia etiqueta', () => {
    // No es MangoHud, pero games.json también lo marca como «medicion» y vive en
    // «Ajustes del sistema»: al pulsar la etiqueta debe seguir viéndose.
    renderTab(
      [
        param({ title: 'Game Performance', value: 'game-performance', impact: 'medicion' }),
        param({ title: 'Activa FSR', value: 'WINE_FSR=1', impact: 'medio' }),
      ],
      [false, false]
    );

    fireEvent.click(screen.getByRole('button', { name: 'medicion' }));

    expect(screen.getByText('Ajustes del sistema')).toBeTruthy();
    // Ese bloque enseña el `value` del parámetro, no su título.
    expect(screen.getByText('game-performance')).toBeTruthy();
    expect(screen.queryByText('Activa FSR')).toBeNull();
  });
});

describe('OptimizeTab: no se ofrecen etiquetas que no llevan a nada', () => {
  // En games.json los 7 parámetros de impacto «bajo» son `essential: false`, así
  // que en modo guiado la etiqueta «bajo» siempre daba una pestaña vacía.
  const params = [
    param({ title: 'SoloAlto', value: 'ALTO=1', impact: 'alto', essential: true }),
    param({ title: 'SoloBajo', value: 'BAJO=1', impact: 'bajo', essential: false }),
  ];

  const chips = () =>
    screen
      .queryAllByRole('button')
      .map((b) => b.textContent)
      .filter((t): t is string => !!t && ['alto', 'medio', 'bajo', 'medicion', 'evitar'].includes(t));

  it('en modo guiado no aparece «bajo» si sus ajustes no son esenciales', () => {
    renderTab(params, [false, false]);

    expect(chips()).toContain('alto');
    expect(chips()).not.toContain('bajo');
  });

  it('en modo experto sí aparece, porque ahí esos ajustes se ven', () => {
    renderTab(params, [false, false], { expertMode: true });

    expect(chips()).toContain('bajo');
    fireEvent.click(screen.getByRole('button', { name: 'bajo' }));
    expect(screen.getByText('SoloBajo')).toBeTruthy();
  });

  it('no aparece «evitar»: esos ajustes nunca se listan', () => {
    renderTab([...params, param({ title: 'Malo', value: 'MALO=1', impact: 'evitar' })], [false, false, false]);

    expect(chips()).not.toContain('evitar');
  });

  it('«alto» de tipo config sí enseña su ajuste', () => {
    // Los 6 ajustes de impacto alto del catálogo son consejos `config`.
    renderTab(
      [param({ kind: 'config', title: 'Usa FSR dentro del juego', value: 'Usa el FSR', impact: 'alto' })],
      [false]
    );

    fireEvent.click(screen.getByRole('button', { name: 'alto' }));

    expect(screen.getByText('Usa FSR dentro del juego')).toBeTruthy();
    expect(screen.queryByText(/Ningún ajuste coincide/)).toBeNull();
  });

  it('toda etiqueta ofrecida deja al menos un ajuste visible', () => {
    // Propiedad: si hay chip, al pulsarlo no puede salir el aviso de vacío.
    const varios = [
      param({ title: 'A', value: 'A=1', impact: 'alto', essential: true }),
      param({ title: 'B', value: 'B=1', impact: 'medio', essential: true }),
      param({ title: 'C', value: 'C=1', impact: 'medicion', essential: true }),
      param({ title: 'D', value: 'D=1', impact: 'bajo', essential: false }),
    ];

    for (const level of ['alto', 'medio', 'medicion']) {
      renderTab(varios, [false, false, false, false]);
      fireEvent.click(screen.getByRole('button', { name: level }));
      expect(screen.queryByText(/Ningún ajuste coincide/)).toBeNull();
      cleanup();
    }
  });
});

describe('OptimizeTab: se ve por qué un ajuste no aplica', () => {
  it('la tarjeta deshabilitada dice el motivo, no queda en gris sin explicación', () => {
    // `missing()` ya calculaba el motivo («Necesita una GPU AMD») pero solo servía
    // para poner `disabled`: el usuario veía una tarjeta inerte y opaca.
    const params = [param({ title: 'Pipeline AMD', value: 'RADV_PERFTEST=gpl', requires: ['amd'] })];

    renderTab(params, [false], { missing: () => 'Necesita una GPU AMD' });

    expect(screen.getByText(/Necesita una GPU AMD/)).toBeTruthy();
  });

  it('si aplica, no se pinta ningún motivo', () => {
    const params = [param({ title: 'Pipeline AMD', value: 'RADV_PERFTEST=gpl', requires: ['amd'] })];

    renderTab(params, [false]);

    expect(screen.queryByText(/Necesita una GPU/)).toBeNull();
  });
});
