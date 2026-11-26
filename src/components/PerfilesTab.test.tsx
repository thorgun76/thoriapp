import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { PerfilesTab } from './PerfilesTab';
import type { Profile } from '../lib/types';

afterEach(cleanup);

const perfil = (over: Partial<Profile> = {}): Profile => ({
  id: 'p1',
  name: 'Rendimiento',
  appid: 1623730,
  game_name: 'Palworld',
  enabled_params: [true],
  selected_community_recs: [],
  selected_engine_recs: [],
  force_proton: true,
  expert_mode: false,
  use_manual_command: false,
  command: 'DXVK_HUD=1 %command%',
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-01-01T00:00:00Z',
  ...over,
});

const props = (over: Partial<Parameters<typeof PerfilesTab>[0]> = {}) => ({
  profiles: [perfil()],
  profileCommands: { p1: 'DXVK_HUD=1 %command%' },
  onAbrirJuego: vi.fn(),
  onCargarEnJuego: vi.fn(),
  onCopy: vi.fn(),
  onExport: vi.fn(),
  onDelete: vi.fn(),
  onReorder: vi.fn(),
  onImportar: vi.fn(),
  onExportarTodos: vi.fn(),
  ...over,
});

describe('PerfilesTab', () => {
  it('sin perfiles explica el estado vacío en vez de una lista en blanco', () => {
    render(<PerfilesTab {...props({ profiles: [] })} />);
    expect(screen.getByText('No tienes perfiles guardados')).toBeTruthy();
  });

  it('«Abrir juego» y «Cargar en el juego» son acciones distintas', () => {
    const p = props();
    render(<PerfilesTab {...p} />);

    // Desplegar la tarjeta para ver las acciones.
    fireEvent.click(screen.getByText('Rendimiento'));

    fireEvent.click(screen.getByText('Abrir juego'));
    expect(p.onAbrirJuego).toHaveBeenCalledWith(1623730);
    expect(p.onCargarEnJuego).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText('Cargar en el juego'));
    expect(p.onCargarEnJuego).toHaveBeenCalledWith(expect.objectContaining({ id: 'p1' }));
  });

  it('marca los perfiles antiguos sin comando', () => {
    render(<PerfilesTab {...props({ profiles: [perfil({ command: undefined })] })} />);
    expect(screen.getByText('Antiguo')).toBeTruthy();
  });

  it('un perfil con comando no lleva la marca de antiguo', () => {
    render(<PerfilesTab {...props()} />);
    expect(screen.queryByText('Antiguo')).toBeNull();
  });

  it('pinta todos los perfiles de la colección única que recibe', () => {
    // Dos juegos distintos: la biblioteca global no filtra por juego abierto.
    render(
      <PerfilesTab
        {...props({
          profiles: [
            perfil(),
            perfil({ id: 'p2', name: 'Calidad', appid: 251570, game_name: '7 Days to Die' }),
          ],
        })}
      />
    );

    expect(screen.getByText('Rendimiento')).toBeTruthy();
    expect(screen.getByText('Calidad')).toBeTruthy();
  });
});
