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
    render(
      <GameSidebar
        hardware={hardware}
        onMeasurePerformance={vi.fn()}
        onShowGameTests={vi.fn()}
      />
    );

    expect(screen.getByText('Tu hardware')).toBeTruthy();
    expect(screen.queryByText(/Tu perfil de hardware/)).toBeNull();
  });

  it('no ofrece «Ajustar optimización»: ya se está en la vista del juego', () => {
    render(
      <GameSidebar
        hardware={hardware}
        onMeasurePerformance={vi.fn()}
        onShowGameTests={vi.fn()}
      />
    );

    expect(screen.queryByText(/Ajustar optimización/)).toBeNull();
  });

  it('no muestra el bloque de fuentes verificables', () => {
    render(
      <GameSidebar
        hardware={hardware}
        onMeasurePerformance={vi.fn()}
        onShowGameTests={vi.fn()}
      />
    );

    expect(screen.queryByText(/Fuentes verificables/)).toBeNull();
  });

  it('ya no ofrece «Medir y comparar»: la ventana de pruebas ya lleva a Benchmarks', () => {
    // Tenías dos botones para lo mismo. Ahora «Pruebas del juego» es la única acción y
    // desde dentro está «Medir una prueba nueva».
    render(
      <GameSidebar
        hardware={hardware}
        onMeasurePerformance={vi.fn()}
        onShowGameTests={vi.fn()}
      />
    );

    expect(screen.queryByText(/Medir y comparar/)).toBeNull();
  });

  // «Pruebas del juego» abre el historial filtrado a este juego. Vive aquí, en las
  // acciones rápidas de la ficha, y no en el ranking global de la barra lateral, que
  // mezclaba grabaciones de juegos distintos.
  it('ofrece «Pruebas del juego» como acción rápida', () => {
    const onShowGameTests = vi.fn();
    render(
      <GameSidebar
        hardware={hardware}
        onMeasurePerformance={vi.fn()}
        onShowGameTests={onShowGameTests}
      />
    );

    fireEvent.click(screen.getByText(/Pruebas del juego/));

    expect(onShowGameTests).toHaveBeenCalledTimes(1);
  });

  it('«Pruebas del juego» es la única acción rápida', () => {
    render(
      <GameSidebar hardware={hardware} onMeasurePerformance={vi.fn()} onShowGameTests={vi.fn()} />
    );

    // Un solo botón dentro del bloque de acciones rápidas.
    const acciones = screen.getByText('Acciones rápidas').parentElement!;
    expect(acciones.querySelectorAll('button')).toHaveLength(1);
  });
});
