import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SaveProfileModal } from './SaveProfileModal';

afterEach(cleanup);

const base = {
  open: true,
  name: 'Rendimiento',
  notes: '',
  onNameChange: vi.fn(),
  onNotesChange: vi.fn(),
  onCancel: vi.fn(),
  onSave: vi.fn(),
};

describe('SaveProfileModal', () => {
  it('sin perfil cargado solo ofrece guardar uno nuevo', () => {
    render(<SaveProfileModal {...base} />);

    expect(screen.getByText('Guardar')).toBeTruthy();
    // «Actualizar» sin perfil cargado no tiene sentido: no hay nada que actualizar.
    expect(screen.queryByText(/Actualizar/)).toBeNull();
  });

  it('con un perfil cargado ofrece actualizarlo y guardar como nuevo', () => {
    const onUpdate = vi.fn();
    render(<SaveProfileModal {...base} perfilCargado="Rendimiento" onUpdate={onUpdate} />);

    // Se dice cuál está cargado: no puede haber duda de qué se sobrescribe.
    expect(screen.getByText(/Hay un perfil cargado/)).toBeTruthy();
    expect(screen.getByText('Guardar como nuevo')).toBeTruthy();

    fireEvent.click(screen.getByText('Actualizar «Rendimiento»'));
    expect(onUpdate).toHaveBeenCalledTimes(1);
    expect(base.onSave).not.toHaveBeenCalled();
  });

  it('«Guardar como nuevo» crea en vez de actualizar', () => {
    const onSave = vi.fn();
    const onUpdate = vi.fn();
    render(
      <SaveProfileModal {...base} onSave={onSave} perfilCargado="Viejo" onUpdate={onUpdate} />
    );

    fireEvent.click(screen.getByText('Guardar como nuevo'));
    expect(onSave).toHaveBeenCalledTimes(1);
    // Nunca actualiza por su cuenta: son dos botones distintos a propósito.
    expect(onUpdate).not.toHaveBeenCalled();
  });

  it('sin nombre no deja guardar', () => {
    render(<SaveProfileModal {...base} name="   " />);
    expect(
      (screen.getByText('Guardar').closest('button') as HTMLButtonElement).disabled
    ).toBe(true);
  });
});
