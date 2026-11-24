import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import type { GroupComparison } from '../lib/types';
import { GroupComparisonCard } from './GroupComparisonCard';

afterEach(cleanup);

function makeRow(over: Partial<GroupComparison> = {}): GroupComparison {
  return {
    metric: 'avg_fps',
    antes_runs: 4,
    despues_runs: 4,
    antes_median: 58,
    despues_median: 70,
    change_percent: 20.7,
    ci_low_percent: 18.2,
    ci_high_percent: 22.4,
    required_margin_percent: 3,
    verdict: 'mejora',
    conclusion: 'Mejora en FPS promedio: +20.7 %.',
    ...over,
  };
}

const percentiles: GroupComparison[] = [
  makeRow({ metric: 'fps1_percent_low', verdict: 'no_concluyente', change_percent: 4 }),
  makeRow({ metric: 'fps01_percent_low', verdict: 'no_concluyente', change_percent: 6 }),
];

describe('GroupComparisonCard', () => {
  it('muestra el veredicto y el intervalo de confianza', () => {
    render(<GroupComparisonCard rows={[makeRow()]} />);
    // «Mejora» aparece en el titular y en la tabla de percentiles: se busca el titular.
    expect(screen.getAllByText(/Mejora/).length).toBeGreaterThan(0);
    expect(screen.getByText(/20,7 %/)).toBeTruthy();
    expect(screen.getByText(/Intervalo del 95 %/)).toBeTruthy();
  });

  it('un resultado no concluyente se dice sin disfrazar', () => {
    const conclusion =
      'No concluyente en FPS promedio: el cambio medido es +2,1 %. Repite la prueba para separar el efecto del ruido.';
    render(
      <GroupComparisonCard
        rows={[
          makeRow({
            verdict: 'no_concluyente',
            change_percent: 2.1,
            ci_low_percent: -1.4,
            ci_high_percent: 5.6,
            conclusion,
          }),
        ]}
      />
    );
    expect(screen.getAllByText(/No concluyente/).length).toBeGreaterThan(0);
    // El texto de la conclusión es el que llega del backend: se muestra tal cual.
    expect(screen.getByText(conclusion)).toBeTruthy();
  });

  it('enseña también los percentiles, no solo el promedio', () => {
    render(<GroupComparisonCard rows={[makeRow(), ...percentiles]} />);
    expect(screen.getByText('1 % de los peores frames')).toBeTruthy();
    expect(screen.getByText('0,1 % de los peores frames')).toBeTruthy();
  });

  it('con pocas grabaciones explica que no se puede separar el efecto del ruido', () => {
    render(<GroupComparisonCard rows={[makeRow({ antes_runs: 2, despues_runs: 2, verdict: 'no_concluyente' })]} />);
    expect(screen.getByText(/menos de 3 grabaciones/i)).toBeTruthy();
  });

  it('el cambio se escribe con coma decimal, como el resto de la app', () => {
    // Los percentiles son los que se pintan en la tabla (el promedio va aparte).
    render(
      <GroupComparisonCard
        rows={[
          makeRow({ change_percent: 4.25 }),
          makeRow({ metric: 'fps1_percent_low', antes_median: 58.5, despues_median: 61 }),
        ]}
      />
    );
    expect(screen.getByText('58,5')).toBeTruthy();
    // Aparece en el titular (+4,3 %) y en la fila de la tabla: basta con que esté.
    expect(screen.getAllByText('+4,3 %').length).toBeGreaterThan(0);
  });

  it('sin filas no pinta nada', () => {
    const { container } = render(<GroupComparisonCard rows={[]} />);
    expect(container.innerHTML).toBe('');
  });
});
