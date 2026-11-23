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

    // «Anti-cheat» sale dos veces: el título de la tarjeta y el índice de secciones.
    expect(screen.getAllByText('Anti-cheat').length).toBeGreaterThan(0);
    expect(screen.getByRole('heading', { name: 'Anti-cheat' })).toBeTruthy();
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

  it('un juego sin reportes se enseña como «Pendiente» y en gris, no como roto', () => {
    const pending = { ...info, tier: 'pending' };
    render(<CompatTab current={game} rules={rules} info={pending} engine={null} antiCheat={null} />);

    const valor = screen.getByText('Pendiente');
    expect(valor).toBeTruthy();
    // Rojo está reservado a «no funciona»: falta de datos no es un fallo.
    expect(valor.style.color).toBe('var(--text-muted)');
  });
});
