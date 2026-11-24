import { describe, expect, it } from 'vitest';
import { sufijoOrigen, textoComando } from '../components/BenchmarksView';
import type { Benchmark } from './types';

const bench = (origen?: Benchmark['origen_comando']): Benchmark =>
  ({
    id: 'x',
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-23 15:10:13',
    duration_seconds: 30,
    avg_fps: 60,
    fps_1_percent_low: 50,
    fps_0_1_percent_low: 45,
    max_fps: 80,
    min_fps: 30,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 20,
    total_frames: 300,
    notes: null,
    launch_command: '%command%',
    origen_comando: origen,
  }) as Benchmark;

describe('sufijoOrigen', () => {
  it('no añade nada cuando el comando salió de la grabación', () => {
    expect(sufijoOrigen(bench('instantanea'))).toBe('');
  });

  it('no añade nada en pruebas antiguas conformes... salvo marcarlas', () => {
    // Prueba antigua = no se sabe de dónde salió. Marcarlo es lo honesto.
    expect(sufijoOrigen(bench(undefined))).toBe(' (origen desconocido)');
  });

  it('avisa cuando el comando se leyó al guardar', () => {
    expect(sufijoOrigen(bench('comando_actual'))).toMatch(/leído al guardar/);
  });

  it('avisa cuando no hay origen', () => {
    expect(sufijoOrigen(bench('desconocido'))).toMatch(/origen desconocido/);
  });
});

describe('textoComando', () => {
  it('enseña lo que Steam tenía puesto aunque el editor dijera %command%', () => {
    // El caso real: la grabación se hizo con argumentos, pero el editor devolvía
    // `%command%` porque las reglas no estaban cargadas. Lo que se ejecutó es lo de Steam.
    const b = bench('comando_actual');
    b.launch_command = '%command%';
    b.steam_command = 'mesa_glthread=true gamemoderun mangohud %command%';
    expect(textoComando(b)).toBe('Steam: mesa_glthread=true gamemoderun mangohud %command%');
  });

  it('Steam mandando sobre el comando del editor', () => {
    const b = bench('instantanea');
    b.launch_command = 'PROTON_ENABLE_NVAPI=1 %command%';
    b.steam_command = 'mangohud %command%';
    expect(textoComando(b)).toMatch(/^Steam: mangohud/);
  });

  it('Steam leído y vacío es un dato, no un hueco', () => {
    const b = bench('instantanea');
    b.steam_command = '';
    expect(textoComando(b)).toMatch(/sin opciones/);
  });

  it('sin lectura de Steam cae al comando del editor, con su marca de origen', () => {
    const b = bench('comando_actual');
    b.launch_command = 'mangohud %command%';
    b.steam_command = null;
    expect(textoComando(b)).toMatch(/^Comando: mangohud %command%/);
    expect(textoComando(b)).toMatch(/leído al guardar/);
  });

  it('sin ningún dato devuelve null: nunca inventa %command%', () => {
    const b = bench('desconocido');
    b.launch_command = '';
    b.steam_command = null;
    expect(textoComando(b)).toBeNull();
  });

  it('una prueba antigua sin el campo tampoco inventa nada', () => {
    const b = bench(undefined);
    b.launch_command = '';
    expect(textoComando(b)).toBeNull();
  });
});
