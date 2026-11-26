import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';

import { MenuRestablecer, type AccionRestablecer } from './MenuRestablecer';

afterEach(cleanup);

const ACCIONES: AccionRestablecer[] = [
  {
    id: 'original',
    titulo: 'Restaurar comando importado',
    detalle: 'Vuelve al texto exacto de Steam.',
    disponible: true,
  },
  {
    id: 'perfil',
    titulo: 'Restaurar perfil guardado',
    detalle: 'Carga los ajustes de un perfil.',
    disponible: false,
    motivoNoDisponible: 'Disponible en la próxima versión.',
  },
  {
    id: 'limpiar',
    titulo: 'Limpiar todos los argumentos',
    detalle: 'Quita los ajustes de Thoriapp.',
    disponible: true,
  },
];

function abrir(acciones = ACCIONES, onElegir = vi.fn()) {
  render(<MenuRestablecer acciones={acciones} onElegir={onElegir} />);
  fireEvent.click(screen.getByRole('button', { name: /restablecer/i }));
  return onElegir;
}

describe('MenuRestablecer', () => {
  it('el menú empieza cerrado', () => {
    render(<MenuRestablecer acciones={ACCIONES} onElegir={vi.fn()} />);

    expect(screen.queryByRole('menu')).toBeNull();
    expect(screen.getByRole('button', { name: /restablecer/i }).getAttribute('aria-expanded')).toBe(
      'false',
    );
  });

  it('al abrir enseña las tres acciones con lo que hace cada una', () => {
    abrir();

    expect(screen.getByRole('menu')).toBeTruthy();
    expect(screen.getByText('Restaurar comando importado')).toBeTruthy();
    expect(screen.getByText('Vuelve al texto exacto de Steam.')).toBeTruthy();
  });

  it('la acción no disponible se ve deshabilitada y dice por qué', () => {
    abrir();
    const boton = screen.getByRole('menuitem', { name: /perfil guardado/i });

    expect((boton as HTMLButtonElement).disabled).toBe(true);
    // Sin el motivo, el usuario ve una entrada muerta y piensa que la app falla.
    expect(screen.getByText('Disponible en la próxima versión.')).toBeTruthy();
  });

  it('pulsar una acción disponible avisa con su id y cierra el menú', () => {
    const onElegir = abrir();

    fireEvent.click(screen.getByRole('menuitem', { name: /limpiar/i }));

    expect(onElegir).toHaveBeenCalledWith('limpiar');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('una acción deshabilitada no dispara nada', () => {
    const onElegir = abrir();

    fireEvent.click(screen.getByRole('menuitem', { name: /perfil guardado/i }));

    expect(onElegir).not.toHaveBeenCalled();
  });

  it('Escape cierra el menú sin ejecutar la acción enfocada', () => {
    const onElegir = abrir();

    fireEvent.keyDown(document, { key: 'Escape' });

    expect(onElegir).not.toHaveBeenCalled();
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('un clic fuera cierra el menú', () => {
    abrir();

    fireEvent.mouseDown(document.body);

    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('sin acciones no hay botón que abra un menú vacío', () => {
    const { container } = render(<MenuRestablecer acciones={[]} onElegir={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });
});
