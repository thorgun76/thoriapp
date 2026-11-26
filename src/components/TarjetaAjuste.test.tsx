import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';

import { TarjetaAjuste } from './TarjetaAjuste';
import type { EstadoAjuste } from '../lib/motorComando';
import type { Param } from '../lib/types';

afterEach(cleanup);

const PARAM = {
  title: 'MangoHud',
  value: 'mangohud',
  kind: 'wrapper',
  why: 'Muestra FPS y temperaturas.',
  impact: 'medio',
} as unknown as Param;

/** Estado del motor para un caso, con los campos que ese caso necesita. */
function estado(estado: string): EstadoAjuste {
  return { indice: 0, estado } as EstadoAjuste;
}

function pintar(props: Partial<Parameters<typeof TarjetaAjuste>[0]> = {}) {
  const onCambiar = vi.fn();
  render(
    <TarjetaAjuste
      param={PARAM}
      indice={0}
      estado={estado('inactivo')}
      encendido={false}
      accionable
      pendiente={false}
      onCambiar={onCambiar}
      {...props}
    />,
  );
  return { onCambiar };
}

describe('TarjetaAjuste', () => {
  it('enseña el nombre arriba y el texto que va al comando aparte', () => {
    // «MangoHud» y `mangohud` son cosas distintas: una se lee, la otra se ejecuta.
    pintar();

    expect(screen.getByText('MangoHud')).toBeTruthy();
    expect(screen.getByText('mangohud')).toBeTruthy();
    expect(screen.getByText('envoltorio')).toBeTruthy();
  });

  it('el estado va escrito, no solo en color', () => {
    pintar();
    expect(screen.getByText(/desactivado/i)).toBeTruthy();
  });

  it('un ajuste activo se anuncia como activo en el interruptor', () => {
    pintar({ estado: estado('activo'), encendido: true });

    const sw = screen.getByRole("switch");
    expect(sw.getAttribute('aria-checked')).toBe('true');
  });

  it('parcial se marca como «mixed», no como apagado', () => {
    // Pintarlo apagado invitaría a pulsar creyendo que se quita del todo.
    pintar({ estado: estado('parcial'), encendido: true });

    expect(screen.getByRole("switch").getAttribute('aria-checked')).toBe(
      'mixed',
    );
    expect(screen.getByText(/parcial|en parte/i)).toBeTruthy();
  });

  it('pulsar pide el cambio con el índice del ajuste', () => {
    const { onCambiar } = pintar();

    fireEvent.click(screen.getByRole("switch"));

    expect(onCambiar).toHaveBeenCalledWith(0, true);
  });

  it('ambiguo no deja pulsar: el motor no sabe cuál de los dos es', () => {
    const { onCambiar } = pintar({ estado: estado('ambiguo'), accionable: false });

    const sw = screen.getByRole("switch");
    expect((sw as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(sw);
    expect(onCambiar).not.toHaveBeenCalled();
  });

  it('parcial, ambiguo y conflicto ofrecen la explicación', () => {
    for (const caso of ['parcial', 'ambiguo', 'conflicto']) {
      const onExplicar = vi.fn();
      pintar({ estado: estado(caso), encendido: caso === 'parcial', onExplicar });

      fireEvent.click(screen.getByRole('button', { name: 'ver' }));
      expect(onExplicar).toHaveBeenCalledWith(0);
      cleanup();
    }
  });

  it('mientras se aplica, la tarjeta está ocupada pero no bloqueada', () => {
    // Bloquearla haría que un clic rápido se perdiera; el motor ya encola en orden.
    pintar({ pendiente: true });

    const tarjeta = screen.getByRole("switch").closest('[aria-busy]');
    expect(tarjeta?.getAttribute('aria-busy')).toBe('true');
  });

  it('dice por qué no aplica en vez de quedarse en gris', () => {
    pintar({ requisito: 'Necesita una GPU AMD', accionable: false, estado: estado('ambiguo') });

    expect(screen.getByText('Necesita una GPU AMD')).toBeTruthy();
  });

  it('conserva el aviso de seguridad del catálogo', () => {
    pintar({ param: { ...PARAM, warn: 'Puede bajar el rendimiento.' } as Param });

    expect(screen.getByText('Puede bajar el rendimiento.')).toBeTruthy();
  });

  it('sin análisis todavía, se pinta como inactivo sin romperse', () => {
    pintar({ estado: null });

    expect(screen.getByText('MangoHud')).toBeTruthy();
    expect(screen.getByRole("switch")).toBeTruthy();
  });
});

describe('TarjetaAjuste: sin pestañeo', () => {
  const base = {
    param: {
      kind: 'env',
      value: 'A=1',
      title: 'Ajuste',
      why: 'Por qué',
      impact: 'medio',
      requires: [],
      default_on: false,
      evidence: 'heuristica',
      proton_only: false,
    },
    indice: 0,
    estado: { indice: 0, valor: 'A=1', kind: 'env', estado: 'inactivo' as const },
    encendido: false,
    accionable: true,
    onCambiar: () => {},
  };

  it('con una operación en vuelo el toggle no se deshabilita', () => {
    // Deshabilitarlo pintaba `opacity-50` y hacía parpadear todas las tarjetas.
    const { container } = render(<TarjetaAjuste {...base} pendiente />);

    const toggle = container.querySelector('[role="switch"]') as HTMLButtonElement;
    expect(toggle.disabled).toBe(false);
  });

  it('no hay ningún punto morado junto al toggle', () => {
    // Lo hubo: aparecía a la izquierda del interruptor en cada activación y, al no
    // reservar su ancho, desplazaba el toggle unos píxeles.
    const { container } = render(<TarjetaAjuste {...base} pendiente />);

    expect(container.querySelector('.animate-latido')).toBeNull();
    // El único interruptor de la tarjeta es el de siempre: no hay punto añadido.
    expect(container.querySelectorAll('[role="switch"]')).toHaveLength(1);
  });

  it('el toggle no se desplaza al pasar de inactivo a activo', () => {
    // Propiedad: el bloque del toggle tiene que ser el mismo antes y después, o el
    // interruptor se mueve bajo el cursor al pulsarlo.
    const { container: apagado } = render(<TarjetaAjuste {...base} pendiente={false} />);
    const claseApagado = apagado.querySelector('[role="switch"]')!.parentElement!.className;

    cleanup();
    const { container: encendido } = render(
      <TarjetaAjuste {...base} pendiente={false} encendido estado={{ ...base.estado, estado: 'activo' }} />
    );
    const claseEncendido = encendido.querySelector('[role="switch"]')!.parentElement!.className;

    expect(claseEncendido).toBe(claseApagado);
  });
});
