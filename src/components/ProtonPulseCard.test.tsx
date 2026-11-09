import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ProtonPulseCard, protonPulseHeadline } from './ProtonPulseCard';
import type { ProtonPulseSummary } from '../lib/types';

afterEach(cleanup);

function makeSummary(overrides: Partial<ProtonPulseSummary> = {}): ProtonPulseSummary {
  return {
    vendor: 'amd',
    total: 40,
    matching: 12,
    good: 9,
    borked: 1,
    same_family: 5,
    reports: [
      {
        gpu: 'AMD Radeon RX 6800 XT (RADV NAVI21)',
        rating: 'platinum',
        proton_version: '10.0-3',
        launch_options: 'PROTON_FSR4_UPGRADE=1 %command%',
      },
      {
        gpu: 'AMD Radeon RX 6700 XT',
        rating: 'gold',
        proton_version: '9.0-4',
      },
    ],
    ...overrides,
  };
}

function renderCard(
  summary: ProtonPulseSummary | null,
  props: { loading?: boolean; onCopyOptions?: (o: string) => void } = {}
) {
  return render(
    <ProtonPulseCard
      summary={summary}
      loading={props.loading}
      onCopyOptions={props.onCopyOptions ?? vi.fn()}
    />
  );
}

describe('protonPulseHeadline: no afirma más de lo que dicen los datos', () => {
  it('un solo reporte se presenta como anécdota', () => {
    expect(protonPulseHeadline(makeSummary({ matching: 1, good: 1 }))).toMatch(/anécdota/);
  });

  it('avisa cuando la mayoría dice que no arranca', () => {
    expect(protonPulseHeadline(makeSummary({ matching: 10, good: 2, borked: 6 }))).toMatch(
      /no arranca/
    );
  });

  it('con todos buenos lo dice, pero con el número delante', () => {
    expect(protonPulseHeadline(makeSummary({ matching: 6, good: 6, borked: 0 }))).toBe(
      '6 reportes en AMD, todos entre platino y oro.'
    );
  });

  it('mezcla de notas: da el recuento, no un veredicto', () => {
    expect(protonPulseHeadline(makeSummary({ matching: 12, good: 9 }))).toBe(
      '9 de 12 reportes en AMD están entre platino y oro.'
    );
  });

  it('sin vendor conocido habla «en general»', () => {
    expect(protonPulseHeadline(makeSummary({ vendor: null }))).toContain('en general');
  });
});

describe('ProtonPulseCard', () => {
  it('sin reportes no pinta nada (ni un «0 reportes» que parezca un error)', () => {
    const { container } = renderCard(null);
    expect(container.innerHTML).toBe('');
  });

  it('muestra los reportes con su GPU, nota y versión de Proton', () => {
    renderCard(makeSummary());

    expect(screen.getByText('Reportes de Proton Pulse')).toBeTruthy();
    expect(screen.getByText(/RX 6800 XT/)).toBeTruthy();
    expect(screen.getByText(/Platino/)).toBeTruthy();
    expect(screen.getByText(/Proton 10.0-3/)).toBeTruthy();
    // Y el recuento total, para saber de qué tamaño es la muestra.
    expect(screen.getByText(/40 reportes del juego en total/)).toBeTruthy();
  });

  it('deja claro que los reportes son de otros y no una recomendación', () => {
    renderCard(makeSummary());
    expect(screen.getByText(/no una recomendación de esta app/)).toBeTruthy();
  });

  it('copia las opciones de lanzamiento de un reporte, tal cual', () => {
    const onCopyOptions = vi.fn();
    renderCard(makeSummary(), { onCopyOptions });

    fireEvent.click(screen.getByTitle(/Copiar estas opciones/));

    // Literal: fusionarlas con el comando actual daría dos `%command%`.
    expect(onCopyOptions).toHaveBeenCalledWith('PROTON_FSR4_UPGRADE=1 %command%');
  });

  it('no ofrece copiar opciones de reportes que no las traen', () => {
    renderCard(makeSummary({ reports: [makeSummary().reports[1]] }));
    expect(screen.queryByTitle(/Copiar estas opciones/)).toBeNull();
  });

  it('mientras busca dice que está buscando, no que no hay', () => {
    renderCard(null, { loading: true });
    expect(screen.getByText(/Buscando reportes de tu GPU/)).toBeTruthy();
  });

  it('lista como máximo 4 reportes', () => {
    const reports = Array.from({ length: 9 }, (_, i) => ({
      gpu: `AMD Radeon RX 7${i}00 XT`,
      rating: 'gold',
      proton_version: '10',
    }));
    renderCard(makeSummary({ reports, matching: 9, good: 9 }));
    expect(screen.getAllByText(/AMD Radeon RX/).length).toBe(4);
  });
});
