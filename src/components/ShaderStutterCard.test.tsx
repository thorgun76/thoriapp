import { afterEach, describe, it, expect } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

afterEach(cleanup);
import { ShaderStutterCard } from './ShaderStutterCard';
import type { Benchmark, ShaderCache } from '../lib/types';

const base: Benchmark = {
  id: 'x',
  profile_id: '',
  game_name: '7 Days to Die',
  appid: 251570,
  timestamp: '2026-01-01 10:00:00',
  duration_seconds: 60,
  avg_fps: 60,
  fps_1_percent_low: 45,
  fps_0_1_percent_low: 30,
  max_fps: 120,
  min_fps: 10,
  frame_time_avg: 16.6,
  frame_time_1_percent_high: 22,
  total_frames: 3600,
  notes: null,
  launch_command: '%command%',
};

const withStutter = (penalty: number): Benchmark => ({
  ...base,
  shader_stutter: {
    window_seconds: 10,
    early_fps_1_percent_low: 30,
    steady_fps_1_percent_low: 60,
    early_pct_over_33ms: 4,
    steady_pct_over_33ms: 1,
    stutter_penalty_percent: penalty,
  },
});

const cache = (steam_precached: boolean, mb = 120): ShaderCache => ({
  bytes: mb * 1024 * 1024,
  locations: [],
  steam_precached,
});

describe('ShaderStutterCard', () => {
  it('no pinta nada si no hay grabaciones', () => {
    const { container } = render(<ShaderStutterCard benchmark={null} cache={null} />);
    expect(container.innerHTML).toBe('');
  });

  it('avisa de compilación de shaders y enseña los dos tramos', () => {
    render(<ShaderStutterCard benchmark={withStutter(50)} cache={cache(true)} />);
    expect(screen.getByText(/compilación de shaders/i)).toBeTruthy();
    // Los números de la ventana inicial y del resto aparecen.
    expect(screen.getByText('30')).toBeTruthy();
    expect(screen.getByText('60')).toBeTruthy();
    expect(screen.getByText(/4\.0 % de frames lentos/)).toBeTruthy();
  });

  it('dice que el arranque está limpio cuando no hay penalización', () => {
    render(<ShaderStutterCard benchmark={withStutter(0)} cache={cache(true)} />);
    expect(screen.getByText(/Arranque limpio/i)).toBeTruthy();
    // El aviso de shaders no aparece: solo se menciona en el pie explicativo.
    expect(screen.queryByText(/Tirones al arrancar/i)).toBeNull();
  });

  it('complementa con el estado de la caché en disco', () => {
    render(<ShaderStutterCard benchmark={withStutter(50)} cache={cache(true, 120)} />);
    expect(screen.getAllByText(/120 MB/).length).toBeGreaterThan(0);
    expect(screen.getByText(/Steam ya la tiene preparada/)).toBeTruthy();
  });

  it('si falta la grabación antigua, lo dice en vez de romperse', () => {
    render(<ShaderStutterCard benchmark={base} cache={null} />);
    expect(screen.getByText(/Sin datos del arranque/i)).toBeTruthy();
  });
});
