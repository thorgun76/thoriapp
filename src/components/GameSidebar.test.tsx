import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { GameSidebar } from './GameSidebar';
import type { Hardware } from '../lib/types';

afterEach(cleanup);

const hardware: Hardware = {
  gpu: 'AMD RX 6800',
  vendor: 'Amd',
  cpu: 'Ryzen 5 5600',
  ram_gb: 16,
  kernel: '6.9.1',
  distro: 'Fedora 40',
  session: 'wayland',
  mesa: '24.1',
  ntsync: true,
  game_performance: true,
  vram_mb: 16384,
  cpu_threads: 12,
  cpu_cores: 6,
  cpu_avx2: true,
  tags: [],
};

describe('GameSidebar', () => {
  it('titula el bloque como «Tu hardware», sin el «perfil de»', () => {
    render(<GameSidebar hardware={hardware} onMeasurePerformance={vi.fn()} />);

    expect(screen.getByText('Tu hardware')).toBeTruthy();
    expect(screen.queryByText(/Tu perfil de hardware/)).toBeNull();
  });

  it('no ofrece «Ajustar optimización»: ya se está en la vista del juego', () => {
    render(<GameSidebar hardware={hardware} onMeasurePerformance={vi.fn()} />);

    expect(screen.queryByText(/Ajustar optimización/)).toBeNull();
  });

  it('no muestra el bloque de fuentes verificables', () => {
    render(<GameSidebar hardware={hardware} onMeasurePerformance={vi.fn()} />);

    expect(screen.queryByText(/Fuentes verificables/)).toBeNull();
  });

  it('mantiene la acción de medir y comparar', () => {
    const onMeasurePerformance = vi.fn();
    render(<GameSidebar hardware={hardware} onMeasurePerformance={onMeasurePerformance} />);

    fireEvent.click(screen.getByText(/Medir y comparar/));

    expect(onMeasurePerformance).toHaveBeenCalledTimes(1);
  });
});
