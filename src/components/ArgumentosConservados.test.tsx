import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';

import { ArgumentosConservados } from './ArgumentosConservados';
import type { SegmentoComando } from '../lib/motorComando';

/** Segmento conservado por un motivo concreto, con un span cualquiera. */
function segmento(
  texto: string,
  motivo: 'desconocido' | 'ambiguo' | 'duplicado' | 'no_seguro' | 'arg_juego' = 'desconocido',
  inicio = 0,
): SegmentoComando {
  return {
    span: { inicio, fin: inicio + texto.length },
    texto,
    valor: texto,
    rol: 'extra',
    // Forma **real** que serializa Rust: el motivo va en `gestion.razon`, no en
    // `gestion.motivo`. Usar la forma equivocada en los tests fue lo que ocultó el bug.
    gestion: { tipo: 'conservado', razon: { motivo } },
  } as unknown as SegmentoComando;
}

// Sin `globals` activo no hay limpieza automática entre tests: sin esto, el DOM de un caso
// se queda montado y el siguiente encuentra botones que no son suyos.
afterEach(cleanup);

describe('ArgumentosConservados', () => {
  it('sin argumentos conservados no ocupa sitio', () => {
    // Un bloque vacío «0 argumentos» haría pensar que falta algo.
    const { container } = render(<ArgumentosConservados segmentos={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('enseña el texto exacto de cada argumento, con su contador', () => {
    render(
      <ArgumentosConservados
        segmentos={[segmento('--novid', 'desconocido', 0), segmento('PROTON_NO_ESYNC=1', 'desconocido', 8)]}
      />,
    );

    expect(screen.getByText('--novid')).toBeTruthy();
    expect(screen.getByText('PROTON_NO_ESYNC=1')).toBeTruthy();
    expect(screen.getByText('2')).toBeTruthy();
  });

  it('dice que desconocido no es lo mismo que incorrecto', () => {
    // Es la duda que quita el miedo a importar un comando de Steam.
    render(<ArgumentosConservados segmentos={[segmento('--novid')]} />);

    expect(screen.getAllByText(/no los toca|no lo toca/i).length).toBeGreaterThan(0);
  });

  it('solo ofrece quitar los que el motor marca como desconocidos', () => {
    const onQuitar = vi.fn();
    render(
      <ArgumentosConservados
        segmentos={[
          segmento('desconocido', 'desconocido', 0),
          segmento('ambiguo', 'ambiguo', 20),
          segmento('del-juego', 'arg_juego', 40),
        ]}
        onQuitar={onQuitar}
      />,
    );

    const botones = screen.queryAllByRole('button', { name: /Quitar «/ });
    expect(botones).toHaveLength(1);

    fireEvent.click(botones[0]);
    // Se manda el índice del segmento, no su span: el motor lo busca por posición.
    expect(onQuitar).toHaveBeenCalledWith(0);
  });

  it('sin manejador no aparece ningún botón de quitar', () => {
    // Si el motor no permite quitar nada, no se enseña un botón que no haría nada. El chip
    // sigue leyéndose como texto (`code`), sin acción asociada.
    render(<ArgumentosConservados segmentos={[segmento('--novid')]} />);
    expect(screen.queryByRole('button', { name: /Quitar «/ })).toBeNull();
    expect(screen.getByText('--novid')).toBeTruthy();
  });

  it('mientras se aplica un cambio no se aceptan más pulsaciones', () => {
    const onQuitar = vi.fn();
    render(
      <ArgumentosConservados
        segmentos={[segmento('--novid')]}
        onQuitar={onQuitar}
        deshabilitado
      />,
    );

    const boton = screen.getByRole('button', { name: /Quitar «--novid»/ });
    expect((boton as HTMLButtonElement).disabled).toBe(true);
    // Un botón deshabilitado no dispara `click` en el navegador; `fireEvent` sí, así que se
    // comprueba el estado real del botón además de que no se llame al manejador.
    fireEvent.click(boton);
    expect(onQuitar).not.toHaveBeenCalled();
  });

  it('explica cada motivo, no solo el de los desconocidos', () => {
    render(
      <ArgumentosConservados
        segmentos={[segmento('ambiguo', 'ambiguo'), segmento('repe', 'duplicado', 30)]}
      />,
    );

    // Al pasar el ratón o al leer, cada chip dice por qué no se gestiona.
    const chips = screen.getAllByTitle(/./);
    expect(chips.length).toBeGreaterThan(0);
  });
});
