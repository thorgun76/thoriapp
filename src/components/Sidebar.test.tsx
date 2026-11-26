import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

afterEach(cleanup);
import { Sidebar } from './Sidebar';

/**
 * Subfase 5.3: el interruptor del observador de procesos.
 *
 * La prueba que importa aquí es de regresión visual: la primera versión usaba un carril de
 * 36×20 px con un desplazamiento de 18 px y el círculo se salía del carril al activarse.
 * Eso no lo detecta un test de comportamiento, solo comprobar las clases que colocan el
 * círculo dentro del carril.
 *
 * No se mide el píxel (no hay motor de maquetación en jsdom), pero sí se fija el
 * invariante que causaba el desbordamiento: carril `w-11` (44 px) y círculo `w-4` (16 px)
 * con desplazamiento máximo 24 px, que deja el círculo justo dentro del borde.
 */
describe('Sidebar · interruptor del observador (5.3)', () => {
  const base = {
    sidebarView: 'library' as const,
    hasCurrentGame: false,
    savedTestsCount: 0,
    profilesCount: 0,
    distro: 'CachyOS',
    onNavigate: vi.fn(),
  };

  const estado = (activo: boolean) => ({
    activo,
    estado: activo ? ('buscando' as const) : ('apagado' as const),
    puede_afirmar_que_no_corre: false,
    pasadas: 3,
    coste_ms: 1.6,
    error: null,
    juegos: [],
  });

  it('el carril y el círculo tienen medidas compatibles en ambos estados', () => {
    const { container, rerender } = render(
      <Sidebar {...base} observador={estado(true)} onCambiarObservador={vi.fn()} />
    );
    const carril = () => container.querySelector('[role="switch"]') as HTMLElement;
    const circulo = () => carril().querySelector('span') as HTMLElement;

    // Activado: carril de 44 px, círculo de 16 px desplazado 24 px → 24+16 = 40 < 44.
    expect(carril().className).toContain('w-11');
    expect(carril().className).toContain('h-6');
    expect(circulo().className).toContain('w-4');
    expect(circulo().className).toContain('translate-x-6');
    expect(carril().getAttribute('aria-checked')).toBe('true');

    // Desactivado: sin desplazamiento, el círculo queda a la izquierda del carril.
    rerender(<Sidebar {...base} observador={estado(false)} onCambiarObservador={vi.fn()} />);
    expect(circulo().className).toContain('translate-x-1');
    expect(circulo().className).not.toContain('translate-x-6');
    expect(carril().getAttribute('aria-checked')).toBe('false');
  });

  it('no usa desplazamientos que saquen el círculo del carril', () => {
    render(<Sidebar {...base} observador={estado(true)} onCambiarObservador={vi.fn()} />);
    const texto = document.body.innerHTML;
    // El valor que causaba el defecto: 18 px en un carril de 36 px con círculo de 16 px.
    expect(texto).not.toContain('translate-x-[18px]');
    expect(texto).not.toContain('w-9 h-5');
  });

  it('al pulsar avisa con el valor nuevo, no con el que tenía', () => {
    const onCambiar = vi.fn();
    render(
      <Sidebar {...base} observador={estado(true)} onCambiarObservador={onCambiar} />
    );
    fireEvent.click(screen.getByRole('switch'));
    expect(onCambiar).toHaveBeenCalledWith(false);
  });

  /**
   * Estados diferenciados exigidos por el usuario.
   *
   * Todos vienen decididos de Rust. React solo elige la frase: no adivina el nombre ni usa
   * el juego seleccionado en la interfaz como si fuera el observado.
   */
  describe('estados diferenciados', () => {
    const conEstado = (extra: Record<string, unknown>) => ({
      ...estado(true),
      ...extra,
    });
    const juego = (nombre: string | null, appid: number | null = 1) => ({
      clave: '1@0',
      appid,
      nombre,
    });

    const pinta = (o: Record<string, unknown>) =>
      render(<Sidebar {...base} observador={o as never} onCambiarObservador={vi.fn()} />);

    it('observador desactivado', () => {
      pinta(estado(false));
      expect(screen.getByText('Observador desactivado')).toBeTruthy();
    });

    it('buscando un juego', () => {
      pinta(conEstado({ estado: 'buscando' }));
      expect(screen.getByText('Buscando un juego…')).toBeTruthy();
    });

    it('Palworld en ejecución, con su nombre comercial', () => {
      pinta(conEstado({ estado: 'observando', juegos: [juego('Palworld', 1623730)] }));
      expect(screen.getByText('Palworld en ejecución')).toBeTruthy();
    });

    it('7 Days to Die en ejecución', () => {
      pinta(
        conEstado({ estado: 'observando', juegos: [juego('7 Days to Die', 251570)] })
      );
      expect(screen.getByText('7 Days to Die en ejecución')).toBeTruthy();
    });

    it('juego detectado pero sin identidad conocida', () => {
      pinta(conEstado({ estado: 'observando', juegos: [juego(null, null)] }));
      expect(screen.getByText('Juego detectado · Identidad desconocida')).toBeTruthy();
    });

    it('varios juegos a la vez: se dicen todos', () => {
      pinta(
        conEstado({
          estado: 'observando',
          juegos: [juego('Palworld', 1623730), juego('7 Days to Die', 251570)],
        })
      );
      const t = screen.getByText(/Varios juegos en ejecución/).textContent;
      expect(t).toContain('Palworld');
      expect(t).toContain('7 Days to Die');
    });

    it('sin visibilidad de procesos', () => {
      pinta(conEstado({ estado: 'sin_visibilidad' }));
      expect(screen.getByText('Sin visibilidad de procesos')).toBeTruthy();
    });

    it('no confunde «desactivado» con «sin visibilidad»', () => {
      // Son cosas distintas: apagado es una decisión, sin visibilidad es un límite.
      const { rerender } = render(
        <Sidebar {...base} observador={estado(false)} onCambiarObservador={vi.fn()} />
      );
      expect(screen.queryByText('Sin visibilidad de procesos')).toBeNull();
      rerender(
        <Sidebar
          {...base}
          observador={conEstado({ estado: 'sin_visibilidad' }) as never}
          onCambiarObservador={vi.fn()}
        />
      );
      expect(screen.queryByText('Observador desactivado')).toBeNull();
    });
  });

  it('arranca apagado mientras el backend no diga lo contrario', () => {
    // Regresión: el valor inicial era `true`, así que el interruptor aparecía encendido en
    // la primera pintura aunque el observador estuviera apagado.
    render(<Sidebar {...base} observador={null} onCambiarObservador={vi.fn()} />);
    const carril = document.querySelector('[role="switch"]') as HTMLElement;
    expect(carril.getAttribute('aria-checked')).toBe('false');
  });

  it('refleja lo que diga el backend, activado o no', () => {
    const { rerender } = render(
      <Sidebar {...base} observador={estado(false)} onCambiarObservador={vi.fn()} />
    );
    const carril = () => document.querySelector('[role="switch"]') as HTMLElement;
    expect(carril().getAttribute('aria-checked')).toBe('false');
    // Llega el estado real de Rust: encendido.
    rerender(
      <Sidebar {...base} observador={estado(true)} onCambiarObservador={vi.fn()} />
    );
    expect(carril().getAttribute('aria-checked')).toBe('true');
  });

  it('dice el estado en palabras y no enseña los milisegundos de forma permanente', () => {
    render(<Sidebar {...base} observador={estado(true)} onCambiarObservador={vi.fn()} />);
    expect(screen.getByText('Buscando un juego…')).toBeTruthy();
    // El coste sigue en el IPC como dato técnico, pero no ocupa la barra lateral.
    expect(screen.queryByText(/ms por pasada/)).toBeNull();
  });

  it('apagado no enseña coste, porque no hay pasadas', () => {
    render(<Sidebar {...base} observador={estado(false)} onCambiarObservador={vi.fn()} />);
    const pie = document.body.textContent ?? '';
    expect(pie).toContain('Observador desactivado');
    expect(pie).not.toContain('ms por pasada');
  });

  it('sin datos del backend el interruptor no miente', () => {
    render(<Sidebar {...base} observador={null} onCambiarObservador={vi.fn()} />);
    expect(document.body.textContent).toContain('Sin datos');
  });
});
