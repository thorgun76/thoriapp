import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { GameBadges, tierColor } from './GameBadges';

const openExternal = vi.hoisted(() => vi.fn());
vi.mock('../lib/links', () => ({ openExternal }));

afterEach(() => {
  cleanup();
  openExternal.mockClear();
});

describe('GameBadges', () => {
  it('pinta las dos etiquetas con su texto', () => {
    render(<GameBadges native tier="gold" appid={251570} />);

    expect(screen.getByText('Linux nativo')).toBeTruthy();
    expect(screen.getByText(/ProtonDB · Oro/)).toBeTruthy();
  });
  it('sin nivel no esconde la etiqueta: dice «sin datos»', () => {
    // Regresión: antes la etiqueta desaparecía y se leía como «este juego no
    // necesita Proton», cuando en realidad era un dato que faltaba.
    render(<GameBadges native={false} tier={null} name="Probably Stolen" />);

    expect(screen.getByText(/ProtonDB · sin datos/)).toBeTruthy();
    // Y explica por qué, con el nombre exacto con el que se buscó: es lo que
    // permite detectar que la biblioteca guarda otro nombre.
    expect(screen.getByText(/Steam no reconoce «Probably Stolen»/)).toBeTruthy();
  });

  it('con AppID conocido, «sin datos» habla de reportes, no de búsqueda', () => {
    render(<GameBadges tier={null} appid={251570} />);

    expect(screen.getByText(/no ha podido leer el nivel/i)).toBeTruthy();
    expect(screen.getByText(/todavía no haya ninguno/i)).toBeTruthy();
  });

  it('adapta el nivel de ProtonDB al valor real del juego', () => {
    render(<GameBadges tier="platinum" />);
    expect(screen.getByText(/ProtonDB · Platino/)).toBeTruthy();
    expect(screen.queryByText(/Oro/)).toBeNull();
  });

  it('colorea el nivel: platino casi blanco, plata gris, oro ámbar y no-funciona en rojo', () => {
    // Platino no es "verde bueno": es el metal más claro. Verde queda solo para
    // «Linux nativo», que es lo que de verdad significa compatible de serie.
    expect(tierColor('platinum')).toContain('platinum');
    expect(tierColor('platinum')).not.toContain('compatible');
    expect(tierColor('silver')).toContain('silver');
    expect(tierColor('gold')).toContain('caution');
    // Bronce es cobre apagado, no el mismo ámbar que oro.
    expect(tierColor('bronze')).toContain('bronze');
    expect(tierColor('bronze')).not.toBe(tierColor('gold'));
    // Un nivel desconocido cae en el aviso más conservador.
    expect(tierColor('borked')).toContain('error');
    expect(tierColor('loquesea')).toBe(tierColor('borked'));
  });

  it('«pending» no se pinta como fallo: es falta de datos, no un juego roto', () => {
    expect(tierColor('pending')).not.toContain('error');
    expect(tierColor('pending')).toContain('text-muted');
  });

  it('igual a ancho: las dos píldoras se estiran al ancho de la más larga', () => {
    render(<GameBadges native tier="gold" appid={251570} />);

    const linux = screen.getByText('Linux nativo');
    const proton = screen.getByText(/ProtonDB · Oro/).closest('span, button')!;
    // `w-full` dentro de una rejilla de una columna: el ancho lo fija la etiqueta
    // más larga, no un valor a ojo que se rompería con «No funciona».
    expect(linux.className).toMatch(/w-full/);
    expect(proton.className).toMatch(/w-full/);
  });

  it('la etiqueta de Linux nativo lleva el icono de Tux', () => {
    render(<GameBadges native />);

    const badge = screen.getByText('Linux nativo');
    const svg = badge.querySelector('svg');
    expect(svg).toBeTruthy();
    // Decorativo: el texto ya dice lo que significa.
    expect(svg?.getAttribute('aria-hidden')).toBe('true');
  });

  it('explica cada etiqueta con un tooltip al que apunta aria-describedby', () => {
    render(<GameBadges native tier="gold" appid={251570} />);

    const tooltips = screen.getAllByRole('tooltip');
    expect(tooltips).toHaveLength(2);

    // Linux nativo ≠ valoración: el texto no debe hablar de Proton ni de notas.
    expect(screen.getByText(/versión oficial para Linux/i)).toBeTruthy();
    expect(screen.getByText(/Valoración de la comunidad/i)).toBeTruthy();

    const describedBy = screen
      .getByText('Linux nativo')
      .getAttribute('aria-describedby');
    expect(describedBy).toBe(tooltips[0].id);
  });

  it('abre el análisis de ProtonDB al pulsar la etiqueta', () => {
    render(<GameBadges tier="gold" appid={251570} />);

    fireEvent.click(screen.getByRole('button', { name: /ProtonDB/ }));

    expect(openExternal).toHaveBeenCalledWith('https://www.protondb.com/app/251570');
  });

  it('sin appid no inventa un enlace: la etiqueta no es un botón', () => {
    render(<GameBadges tier="gold" />);

    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.getByText(/ProtonDB · Oro/)).toBeTruthy();
  });
});

describe('GameBadges: vincular a mano', () => {
  it('el «sin datos» sin AppID se puede pulsar para vincularlo', () => {
    const onLink = vi.fn();
    render(<GameBadges tier={null} name="NoManSky_v5.0" onLink={onLink} />);

    const pill = screen.getByRole('button', { name: /ProtonDB · sin datos/ });
    fireEvent.click(pill);

    expect(onLink).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/Pulsa para buscar el juego en Steam/i)).toBeTruthy();
  });

  it('con AppID conocido no ofrece vincular: lo que falta es el nivel, no el juego', () => {
    render(<GameBadges tier={null} appid={275850} onLink={() => {}} />);

    expect(screen.queryByRole('button', { name: /ProtonDB · sin datos/ })).toBeNull();
  });

  it('sin la función no es un botón: no promete una acción que no existe', () => {
    render(<GameBadges tier={null} />);

    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.getByText(/ProtonDB · sin datos/)).toBeTruthy();
  });
});
