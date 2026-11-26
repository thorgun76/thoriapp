import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

/**
 * Regresión de los tres fallos que reportó el usuario con 7 Days to Die.
 *
 * El escenario es el real: Steam tiene
 * `RADV_PERFTEST=sam PROTON_USE_NTSYNC=1 game-performance mangohud %command%` y el usuario
 * pulsa «Traer a Thoriapp». Se montan los componentes reales con los segmentos **en la forma
 * exacta que serializa Rust**; usar una forma «parecida» en el test fue justo lo que dejó
 * pasar el bug (`gestion.motivo` en vez de `gestion.razon`).
 */

import { ArgumentosConservados } from './ArgumentosConservados';
import { CommandBox } from './CommandBox';
import type { SegmentoComando } from '../lib/motorComando';

const COMANDO_STEAM = 'RADV_PERFTEST=sam PROTON_USE_NTSYNC=1 game-performance mangohud %command%';

/** Los segmentos tal y como los devuelve Rust para ese comando (JSON real). */
const SEGMENTOS: SegmentoComando[] = [
  {
    span: { inicio: 0, fin: 16 },
    texto: 'RADV_PERFTEST=sam',
    valor: 'RADV_PERFTEST=sam',
    rol: 'env',
    gestion: { tipo: 'conservado', razon: { motivo: 'desconocido' } },
    relleno: '',
  },
  {
    span: { inicio: 17, fin: 38 },
    texto: 'PROTON_USE_NTSYNC=1',
    valor: 'PROTON_USE_NTSYNC=1',
    rol: 'env',
    gestion: { tipo: 'conservado', razon: { motivo: 'desconocido' } },
    relleno: '',
  },
  {
    span: { inicio: 39, fin: 55 },
    texto: 'game-performance',
    valor: 'game-performance',
    rol: 'wrapper',
    gestion: { tipo: 'thoriapp', clave: 'game-performance', indice: 0 },
    relleno: '',
  },
  {
    span: { inicio: 56, fin: 64 },
    texto: 'mangohud',
    valor: 'mangohud',
    rol: 'wrapper',
    gestion: { tipo: 'thoriapp', clave: 'mangohud', indice: 2 },
    relleno: '',
  },
  {
    span: { inicio: 65, fin: 74 },
    texto: '%command%',
    valor: '%command%',
    rol: 'marcador',
    gestion: { tipo: 'marcador' },
    relleno: '',
  },
] as unknown as SegmentoComando[];

const PROPS_COMMAND_BOX = {
  gameName: '7 Days to Die',
  launchCommand: COMANDO_STEAM,
  activeParts: [],
  activeCount: 2,
  blocked: false,
  blockedReason: '',
  copied: false,
  expertMode: false,
  manualEditing: false,
  manualCommand: '',
  manualCommandError: '',
  info: { total: 0 },
  appid: 251570,
  env: null,
  launcher: 'steam',
} as unknown as React.ComponentProps<typeof CommandBox>;

afterEach(cleanup);

describe('regresión: pantalla negra al importar de Steam (7 Days to Die)', () => {
  it('ArgumentosConservados no revienta con el comando importado', () => {
    // El fallo era «undefined is not an object (evaluating 's.gestion.motivo.motivo')».
    expect(() =>
      render(<ArgumentosConservados segmentos={SEGMENTOS} onQuitar={() => {}} />),
    ).not.toThrow();
  });

  it('CommandBox pinta el comando importado entero', () => {
    render(<CommandBox {...PROPS_COMMAND_BOX} />);
    expect(screen.getAllByText(/RADV_PERFTEST=sam/).length).toBeGreaterThan(0);
  });

  it('%command% no se ofrece como argumento personalizado', () => {
    render(<ArgumentosConservados segmentos={SEGMENTOS} onQuitar={() => {}} />);
    expect(screen.queryByText('%command%')).toBeNull();
  });

  it('un ajuste reconocido no aparece a la vez como argumento conservado', () => {
    // `game-performance` estaba activo arriba y «desconocido» abajo: dos clasificaciones
    // del mismo token.
    render(<ArgumentosConservados segmentos={SEGMENTOS} onQuitar={() => {}} />);
    expect(screen.queryByText('game-performance')).toBeNull();
    expect(screen.queryByText('mangohud')).toBeNull();
  });

  it('los argumentos realmente desconocidos sí se enseñan', () => {
    render(<ArgumentosConservados segmentos={SEGMENTOS} onQuitar={() => {}} />);
    expect(screen.getByText('RADV_PERFTEST=sam')).toBeTruthy();
    expect(screen.getByText('PROTON_USE_NTSYNC=1')).toBeTruthy();
  });

  it('el contador cuenta solo los conservados, no el análisis entero', () => {
    render(<ArgumentosConservados segmentos={SEGMENTOS} onQuitar={() => {}} />);
    // Dos desconocidos de cinco segmentos.
    const contador = screen.getByText('2', { selector: 'span' });
    expect(contador).toBeTruthy();
  });

  it('sin argumentos conservados la tarjeta desaparece', () => {
    const soloGestionados = SEGMENTOS.filter((s) => s.gestion.tipo !== 'conservado');
    const { container } = render(<ArgumentosConservados segmentos={soloGestionados} />);
    expect(container.firstChild).toBeNull();
  });
});
