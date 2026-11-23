import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { GAME_TABS, GameTabs } from './GameTabs';

afterEach(cleanup);

describe('GameTabs', () => {
  it('pinta una pestaña por sección con su resumen', () => {
    render(<GameTabs view="recommendations" onSelect={() => {}} />);

    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(GAME_TABS.length);
    expect(screen.getByText('Compatibilidad')).toBeTruthy();
    expect(screen.getByText('Optimización')).toBeTruthy();
    // El resumen es lo que hace la tarjeta legible sin abrirla.
    expect(screen.getByText(/anti-cheat y ProtonDB/i)).toBeTruthy();
    expect(screen.getByText(/ajustes avanzados/i)).toBeTruthy();
  });

  it('marca como seleccionada solo la pestaña activa', () => {
    render(<GameTabs view="compatibility" onSelect={() => {}} />);

    const [compat, optim] = screen.getAllByRole('tab');
    expect(compat.getAttribute('aria-selected')).toBe('true');
    expect(optim.getAttribute('aria-selected')).toBe('false');
  });

  it('refuerza solo la tarjeta activa con borde de color y fondo teñido', () => {
    render(<GameTabs view="recommendations" onSelect={() => {}} />);

    const [compat, optim] = screen.getAllByRole('tab');
    // La activa: borde de 2 px a color, degradado morado suave y cursor de dedo.
    expect(optim.className).toMatch(/border-2/);
    expect(optim.className).toMatch(/border-action/);
    expect(optim.className).toMatch(/from-action\/15/);
    expect(optim.className).toMatch(/cursor-pointer/);
    // La inactiva: borde discreto, y el morado solo al pasar el ratón.
    expect(compat.className).toMatch(/border-\[var\(--border\)\]/);
    expect(compat.className).toMatch(/hover:border-action/);
    expect(optim.className).not.toMatch(/hover:border-action/);
  });

  it('avisa del id de la pestaña pulsada', () => {
    const onSelect = vi.fn();
    render(<GameTabs view="compatibility" onSelect={onSelect} />);

    fireEvent.click(screen.getByText('Optimización'));

    expect(onSelect).toHaveBeenCalledWith('recommendations');
  });
});
