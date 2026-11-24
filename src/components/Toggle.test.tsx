import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Toggle } from './Toggle';

afterEach(cleanup);

describe('Toggle', () => {
  it('se anuncia como interruptor, no como botón cualquiera', () => {
    render(<Toggle checked={false} onChange={() => {}} label="Guardado automático" />);
    const sw = screen.getByRole('switch', { name: 'Guardado automático' });
    expect(sw.getAttribute('aria-checked')).toBe('false');
  });

  it('refleja el estado encendido', () => {
    render(<Toggle checked onChange={() => {}} label="Guardado automático" />);
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('true');
  });

  it('al pulsarlo pide el valor contrario, no el mismo', () => {
    const onChange = vi.fn();
    render(<Toggle checked={false} onChange={onChange} label="X" />);
    fireEvent.click(screen.getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('apagado pide false', () => {
    const onChange = vi.fn();
    render(<Toggle checked onChange={onChange} label="X" />);
    fireEvent.click(screen.getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('se puede accionar con el teclado', () => {
    // Es un <button>, así que Enter y Espacio funcionan sin nada más.
    const onChange = vi.fn();
    render(<Toggle checked={false} onChange={onChange} label="X" />);
    const sw = screen.getByRole('switch');
    sw.focus();
    expect(document.activeElement).toBe(sw);
    fireEvent.click(sw); // lo que dispara Enter/Espacio en un button
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('deshabilitado no responde', () => {
    const onChange = vi.fn();
    render(<Toggle checked={false} onChange={onChange} label="X" disabled />);
    const sw = screen.getByRole('switch') as HTMLButtonElement;
    expect(sw.disabled).toBe(true);
    fireEvent.click(sw);
    expect(onChange).not.toHaveBeenCalled();
  });

  it('el nombre accesible no depende del texto visible, así que se lee siempre', () => {
    // El interruptor no lleva texto dentro: si no tuviera aria-label sería un botón
    // anónimo en el orden de tabulación.
    render(<Toggle checked={false} onChange={() => {}} label="Editar comando manualmente" />);
    expect(screen.getByRole('switch', { name: 'Editar comando manualmente' })).toBeTruthy();
  });
});
