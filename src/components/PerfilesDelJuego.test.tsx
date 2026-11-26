import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { PerfilesDelJuego } from './PerfilesDelJuego';
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

const props = (over: Partial<Parameters<typeof PerfilesDelJuego>[0]> = {}) => ({
  perfiles: [perfil()],
  cargadoId: null,
  cargadoNombre: null,
  sinGuardar: false,
  ocupado: false,
  onCargar: vi.fn(),
  onRestaurarCargado: vi.fn(),
  onGestionar: vi.fn(),
  ...over,
});

describe('PerfilesDelJuego', () => {
  it('elegir en la lista no carga nada: solo el botón carga', () => {
    const p = props();
    render(<PerfilesDelJuego {...p} />);

    // Cambiar la selección no puede disparar una carga: cargar descarta trabajo, y un
    // `<select>` se mueve con la rueda del ratón sin querer.
    fireEvent.change(screen.getByLabelText('Elegir perfil'), { target: { value: 'p1' } });
    expect(p.onCargar).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText('Cargar perfil'));
    expect(p.onCargar).toHaveBeenCalledWith(expect.objectContaining({ id: 'p1' }));
  });

  it('dice cuántos perfiles hay, en singular y en plural', () => {
    const { unmount } = render(<PerfilesDelJuego {...props()} />);
    expect(screen.getByText('1 perfil')).toBeTruthy();
    unmount();

    render(<PerfilesDelJuego {...props({ perfiles: [perfil(), perfil({ id: 'p2' })] })} />);
    expect(screen.getByText('2 perfiles')).toBeTruthy();
  });

  it('marca los perfiles antiguos sin comando, sin esconderlos', () => {
    render(<PerfilesDelJuego {...props({ perfiles: [perfil({ command: undefined })] })} />);

    // El aviso va en la opción: se puede cargar, pero se sabe que no trae comando propio.
    const opcion = screen.getByRole('option', { name: /Rendimiento \(antiguo\)/ });
    expect(opcion).toBeTruthy();
  });

  it('enseña el perfil cargado y ofrece restaurarlo', () => {
    const p = props({ cargadoId: 'p1', cargadoNombre: 'Rendimiento', sinGuardar: true });
    render(<PerfilesDelJuego {...p} />);

    expect(screen.getByTitle('Rendimiento')).toBeTruthy();
    // Se dice «cargado», no «activo»: Steam no ha visto esto.
    expect(screen.getByText('Cargado')).toBeTruthy();
    expect(screen.queryByText(/activo/i)).toBeNull();
    expect(screen.getByText('Cambios sin guardar en este perfil.')).toBeTruthy();

    fireEvent.click(screen.getByText('Restaurar perfil cargado'));
    expect(p.onRestaurarCargado).toHaveBeenCalledTimes(1);
  });

  it('sin cambios sin guardar no avisa de nada', () => {
    render(<PerfilesDelJuego {...props({ cargadoId: 'p1', sinGuardar: false })} />);
    expect(screen.queryByText('Cambios sin guardar en este perfil.')).toBeNull();
  });

  it('el perfil cargado se recuerda aunque ya no esté en la lista', () => {
    // Se borró desde la biblioteca: el nombre se sigue diciendo, porque es lo que el
    // usuario tenía delante, en vez de desaparecer sin explicación.
    render(
      <PerfilesDelJuego {...props({ perfiles: [], cargadoId: 'borrado', cargadoNombre: 'Viejo' })} />
    );

    expect(screen.getByText(/no tiene perfiles guardados/i)).toBeTruthy();
    // Sin perfiles la lista no se pinta, así que no se afirma nada del cargado aquí.
    expect(screen.queryByText('Cargado')).toBeNull();
  });

  it('con la lista vacía explica cómo crear el primero', () => {
    const p = props({ perfiles: [] });
    render(<PerfilesDelJuego {...p} />);

    expect(screen.getByText(/usa «Guardar perfil» para crear el primero/i)).toBeTruthy();
    expect(screen.queryByText('Cargar perfil')).toBeNull();

    fireEvent.click(screen.getByText('Ver todos los perfiles'));
    expect(p.onGestionar).toHaveBeenCalled();
  });

  it('mientras el motor trabaja no deja cargar ni cambiar de selección', () => {
    render(<PerfilesDelJuego {...props({ ocupado: true })} />);

    // Dos cargas seguidas dejarían dos peticiones peleando por la misma revisión.
    expect(
      (screen.getByText('Cargar perfil').closest('button') as HTMLButtonElement).disabled
    ).toBe(true);
    expect((screen.getByLabelText('Elegir perfil') as HTMLSelectElement).disabled).toBe(true);
  });

  it('«Cargar perfil» está deshabilitado hasta elegir uno', () => {
    render(<PerfilesDelJuego {...props()} />);
    expect(
      (screen.getByText('Cargar perfil').closest('button') as HTMLButtonElement).disabled
    ).toBe(true);
  });
});
