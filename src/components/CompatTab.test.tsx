import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { CompatTab } from './CompatTab';
import type { AntiCheatInfo, Game, GameInfo, GameRules } from '../lib/types';

vi.mock('../lib/api', () => ({ api: { openUrl: vi.fn().mockResolvedValue(undefined) } }));

afterEach(cleanup);

const game = { appid: 2807960, name: 'Battlefield 6' } as Game;
const rules = { name: 'Battlefield 6', appid: 2807960, bottleneck: '', summary: '', params: [], tips: [], has_rules: false } as GameRules;
const info: GameInfo = { name: 'Battlefield 6', native: false, tier: 'borked', errors: [] };

function renderTab(antiCheat: AntiCheatInfo | null) {
  return render(
    <CompatTab
      current={game}
      rules={rules}
      info={info}
      engine={null}
      antiCheat={antiCheat}
    />
  );
}

describe('CompatTab: anti-cheat', () => {
  it('muestra el bloqueo cuando el anti-cheat lo impide', () => {
    renderTab({
      status: 'Denied',
      anticheats: ['EA anticheat'],
      reference: 'https://example.com/bf',
    });

    expect(screen.getByText('Anti-cheat')).toBeTruthy();
    expect(screen.getByText('Bloqueado por el anti-cheat')).toBeTruthy();
    expect(screen.getByText(/Usa: EA anticheat/)).toBeTruthy();
    // Y deja claro que ningún ajuste lo arregla.
    expect(screen.getByText(/No hay nada que configurar/)).toBeTruthy();
    expect(screen.getByText(/Ver la fuente/)).toBeTruthy();
  });

  it('avisa, sin bloquear, cuando funciona sin soporte oficial', () => {
    renderTab({ status: 'Running', anticheats: ['Easy Anti-Cheat'] });

    expect(screen.getByText('Funciona, sin soporte oficial')).toBeTruthy();
    expect(screen.queryByText(/No hay nada que configurar/)).toBeNull();
  });

  it('sin datos lo dice en vez de dar el juego por bueno', () => {
    renderTab(null);

    expect(screen.getByText('Sin datos')).toBeTruthy();
  });

  it('el tier de ProtonDB se muestra en español', () => {
    renderTab(null);

    expect(screen.getByText('No funciona')).toBeTruthy();
  });
});
