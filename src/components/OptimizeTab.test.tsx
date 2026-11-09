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

function renderTab(params: Param[], enabled: boolean[]) {
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
