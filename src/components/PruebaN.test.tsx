import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { PruebaN } from './PruebaN';
import type { Benchmark } from '../lib/types';

/**
 * Etiqueta «Prueba N» y su tooltip con el comando.
 *
 * Lo que se fija aquí es lo que el usuario pidió explícitamente: que el comando completo
 * se pueda consultar al pasar el ratón, al llegar con el teclado y al pulsar (táctil), y
 * que cuando no hay comando se diga, en vez de inventar `%command%`.
 */

const bench = (extra: Partial<Benchmark> = {}): Benchmark =>
  ({
    id: 'b1',
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-23 15:10:13',
    duration_seconds: 30,
    avg_fps: 74.3,
    fps_1_percent_low: 50,
    fps_0_1_percent_low: 45,
    max_fps: 80,
    min_fps: 30,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 20,
    total_frames: 300,
    notes: null,
    launch_command: '%command%',
    origen_comando: 'instantanea',
    number: 2,
    ...extra,
  }) as Benchmark;

afterEach(cleanup);

describe('PruebaN', () => {
  it('muestra «Prueba N» con el número de la grabación', () => {
    render(<PruebaN bench={bench({ number: 3 })} />);
    expect(screen.getByRole('button', { name: 'Prueba 3' })).toBeTruthy();
  });

  it('dice «Prueba sin número» en grabaciones antiguas', () => {
    render(<PruebaN bench={bench({ number: null })} />);
    expect(screen.getByRole('button', { name: 'Prueba sin número' })).toBeTruthy();
  });

  it('el tooltip lleva el comando completo, sin recortar', () => {
    const cmd = `PROTON_ENABLE_NVAPI=1 gamemoderun mangohud %command% --arg="valor con espacios"`;
    render(<PruebaN bench={bench({ steam_command: cmd })} />);
    expect(screen.getByRole('tooltip').textContent).toContain(cmd);
  });

  it('conserva comillas y variables de entorno', () => {
    const cmd = `A=1 B=2 --x="dos palabras" %command%`;
    render(<PruebaN bench={bench({ steam_command: cmd })} />);
    const tip = screen.getByRole('tooltip').textContent ?? '';
    expect(tip).toContain('A=1 B=2');
    expect(tip).toContain('--x="dos palabras"');
  });

  it('no recorta comandos muy largos', () => {
    const largo = `A=1 ${'B=2 '.repeat(60)}%command%`;
    render(<PruebaN bench={bench({ steam_command: largo })} />);
    expect(screen.getByRole('tooltip').textContent).toContain(largo);
  });

  it('sin comando dice «Comando no registrado» y no inventa %command%', () => {
    render(<PruebaN bench={bench({ steam_command: null, launch_command: '' })} />);
    const tip = screen.getByRole('tooltip').textContent ?? '';
    expect(tip).toContain('Comando no registrado');
    expect(tip).not.toContain('%command%');
  });

  it('el tooltip está en el DOM y se anuncia con aria-describedby', () => {
    render(<PruebaN bench={bench()} />);
    const btn = screen.getByRole('button', { name: 'Prueba 2' });
    const id = btn.getAttribute('aria-describedby');
    expect(id).toBeTruthy();
    // El globo se pinta siempre (oculto por CSS), así que se puede enlazar y leer.
    expect(document.getElementById(id!)?.getAttribute('role')).toBe('tooltip');
  });

  it('se abre con el teclado (el botón es enfocable)', () => {
    render(<PruebaN bench={bench()} />);
    const btn = screen.getByRole('button', { name: 'Prueba 2' });
    btn.focus();
    expect(document.activeElement).toBe(btn);
    // La apertura por foco la resuelve `group-focus-within` en CSS.
    expect(btn.getAttribute('aria-expanded')).toBe('false');
  });

  it('se abre al pulsar (única vía en pantalla táctil)', () => {
    render(<PruebaN bench={bench()} />);
    const btn = screen.getByRole('button', { name: 'Prueba 2' });
    fireEvent.click(btn);
    expect(btn.getAttribute('aria-expanded')).toBe('true');
    fireEvent.click(btn);
    expect(btn.getAttribute('aria-expanded')).toBe('false');
  });

  it('Escape cierra el globo abierto con clic', () => {
    render(<PruebaN bench={bench()} />);
    const btn = screen.getByRole('button', { name: 'Prueba 2' });
    fireEvent.click(btn);
    expect(btn.getAttribute('aria-expanded')).toBe('true');
    fireEvent.keyDown(btn, { key: 'Escape' });
    expect(btn.getAttribute('aria-expanded')).toBe('false');
  });

  it('copia el comando al portapapeles', () => {
    const onCopy = vi.fn();
    const cmd = 'mesa_glthread=true mangohud %command%';
    render(<PruebaN bench={bench({ steam_command: cmd })} onCopy={onCopy} />);

    fireEvent.click(screen.getByRole('button', { name: /Copiar/ }));

    // Se copia el comando tal cual, sin el prefijo «Steam:» que solo es presentación.
    expect(onCopy).toHaveBeenCalledWith(cmd);
  });

  it('no ofrece copiar cuando Steam no tenía opciones: no habría nada válido que pegar', () => {
    render(<PruebaN bench={bench({ steam_command: '' })} onCopy={vi.fn()} />);
    expect(screen.getByRole('tooltip').textContent).toContain('sin opciones de lanzamiento');
    expect(screen.queryByRole('button', { name: /Copiar/ })).toBeNull();
  });

  it('sin comando no ofrece botón de copiar', () => {
    render(<PruebaN bench={bench({ steam_command: null, launch_command: '' })} onCopy={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /Copiar/ })).toBeNull();
  });

  it('sin handler de copia no pinta el botón', () => {
    render(<PruebaN bench={bench({ steam_command: 'x' })} />);
    expect(screen.queryByRole('button', { name: /Copiar/ })).toBeNull();
  });
});
