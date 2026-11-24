import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { SavedComparisonDetail } from './SavedComparisonDetail';

afterEach(cleanup);

// Comprobación de que no se cuelan anchos fijos que rompan en pantallas pequeñas:
// jsdom no hace layout, así que se revisan las clases que sí lo condicionan.

describe('responsive: sin anchos fijos peligrosos', () => {
  it('las tarjetas usan grid de una columna y permiten encoger', () => {
    const { container } = render(
      <SavedComparisonDetail
        comparison={{
          baseline: { id: 'a', profile_id: '', game_name: 'J', appid: 1, timestamp: '2026-01-01 10:00:00', duration_seconds: 60, avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60, max_fps: 1, min_fps: 1, frame_time_avg: 10, frame_time_1_percent_high: 1, total_frames: 600, notes: null, launch_command: 'mangohud %command%', spikes_per_min: 1, pct_over_33ms: 1, jitter_ms: 1, worst_frame_ms: 1, warmup_seconds: 10, noise_percent: 5, log_path: '' },
          test: { id: 'b', profile_id: '', game_name: 'J', appid: 1, timestamp: '2026-01-01 10:00:00', duration_seconds: 60, avg_fps: 130, fps_1_percent_low: 90, fps_0_1_percent_low: 80, max_fps: 1, min_fps: 1, frame_time_avg: 8, frame_time_1_percent_high: 1, total_frames: 600, notes: null, launch_command: 'gamemoderun mangohud %command%', spikes_per_min: 1, pct_over_33ms: 1, jitter_ms: 1, worst_frame_ms: 1, warmup_seconds: 10, noise_percent: 5, log_path: '' },
          fps_change_percent: 30, fps_1_percent_change_percent: 28, fps_0_1_percent_change_percent: 33,
          frame_time_change_percent: -20, conclusion: '', noise_percent: 5, within_noise: false,
        } as never}
        onBack={() => {}} onDelete={() => {}} onCopy={() => {}}
      />
    );
    const conAnchoFijo = Array.from(container.querySelectorAll('[class]')).filter((el) =>
      /min-w-\[(\d+)px\]/.test(el.getAttribute('class') ?? '')
    );
    expect(conAnchoFijo).toHaveLength(0);

    // Las columnas de métricas colapsan en pantallas estrechas.
    expect(container.innerHTML).toContain('grid-cols-1 md:grid-cols-2');
    expect(container.innerHTML).toContain('min-w-0');
  });
});
