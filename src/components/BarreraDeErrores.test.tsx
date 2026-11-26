import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';

import { BarreraDeErrores } from './BarreraDeErrores';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** Componente que revienta al pintarse, como el fallo real. */
function Rompe(): JSX.Element {
  throw new TypeError("Cannot read properties of undefined (reading 'map')");
}

describe('BarreraDeErrores', () => {
  it('sin errores deja pasar el contenido', () => {
    render(
      <BarreraDeErrores zona="Ficha del juego">
        <p>contenido normal</p>
      </BarreraDeErrores>,
    );

    expect(screen.getByText('contenido normal')).toBeTruthy();
  });

  it('un fallo de render no deja la pantalla en blanco', () => {
    // React escribe el error en consola durante el render de prueba; se silencia para no
    // ensuciar la salida, pero se comprueba aparte que la barrera SÍ lo registra.
    vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <BarreraDeErrores zona="Ficha del juego">
        <Rompe />
      </BarreraDeErrores>,
    );

    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/No se pudo mostrar Ficha del juego/)).toBeTruthy();
  });

  it('enseña el mensaje real del error, no un texto genérico', () => {
    // Es la diferencia entre una barrera útil y una que esconde el bug.
    vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <BarreraDeErrores zona="Ficha del juego">
        <Rompe />
      </BarreraDeErrores>,
    );

    // Aparece en el título y en el bloque de detalle: las dos veces es el mensaje real.
    expect(screen.getAllByText(/reading 'map'/).length).toBeGreaterThan(0);
  });

  it('deja constancia del error en la consola', () => {
    const consola = vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <BarreraDeErrores zona="Ficha del juego">
        <Rompe />
      </BarreraDeErrores>,
    );

    const registrado = consola.mock.calls.some((c) =>
      String(c[0]).includes('Fallo al pintar «Ficha del juego»'),
    );
    expect(registrado).toBe(true);
  });

  it('dice que el resto de la app sigue viva', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <BarreraDeErrores zona="Ficha del juego">
        <Rompe />
      </BarreraDeErrores>,
    );

    expect(screen.getByText(/El resto de la aplicación sigue funcionando/)).toBeTruthy();
  });

  it('reintentar repinta y vuelve a intentar el contenido', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const onReintentar = vi.fn();
    let romper = true;

    // Se monta con el hijo roto y, **después** del fallo, se deja de romper. Así el
    // reintento tiene algo que repintar: si el hijo siguiera roto, reintentar volvería a
    // dejar la tarjeta de error y el test no probaría nada.
    function AVecesRompe() {
      if (romper) throw new Error('fallo transitorio');
      return <p>ya funciona</p>;
    }

    render(
      <BarreraDeErrores zona="Ficha del juego" onReintentar={onReintentar}>
        <AVecesRompe />
      </BarreraDeErrores>,
    );

    romper = false;
    fireEvent.click(screen.getByRole('button', { name: /reintentar/i }));

    expect(onReintentar).toHaveBeenCalled();
    expect(screen.getByText('ya funciona')).toBeTruthy();
  });
});
