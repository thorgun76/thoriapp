import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { OptimizationSummary } from './OptimizationSummary';

afterEach(cleanup);

const base = {
  summary: 'Perfil base seguro para empezar.',
  impacts: { count: 0, worst: null, label: 'Sin ajustes activos', tone: 'none' as const },
};

describe('OptimizationSummary', () => {
  it('pinta el texto del perfil y el estado de los ajustes', () => {
    render(<OptimizationSummary {...base} />);

    expect(screen.getByText(/Perfil de optimización/i)).toBeTruthy();
    expect(screen.getByText('Perfil base seguro para empezar.')).toBeTruthy();
    expect(screen.getByText('Sin ajustes activos')).toBeTruthy();
  });

  it('añade motor, AppID y reportes solo cuando hay datos', () => {
    const { rerender } = render(<OptimizationSummary {...base} />);

    // Un juego de Lutris no tiene AppID de Steam y no debe pintar «Steam AppID undefined».
    expect(screen.queryByText(/Steam AppID/)).toBeNull();
    expect(screen.queryByText(/reportes/)).toBeNull();

    rerender(
      <OptimizationSummary {...base} engine="Unity" appid={251570} reports={225} />
    );

    expect(screen.getByText('Unity')).toBeTruthy();
    expect(screen.getByText('Steam AppID 251570')).toBeTruthy();
    expect(screen.getByText('225 reportes')).toBeTruthy();
  });

  it('no pinta reportes con cero (ProtonDB sin datos no es un dato)', () => {
    render(<OptimizationSummary {...base} reports={0} />);

    expect(screen.queryByText(/reportes/)).toBeNull();
  });
});
