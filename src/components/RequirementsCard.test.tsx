import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { RequirementsCard, requirementsHeadline } from './RequirementsCard';
import type { RequirementCheck, RequirementsReport, RequirementVerdict } from '../lib/types';

afterEach(cleanup);

function check(over: Partial<RequirementCheck> = {}): RequirementCheck {
  return {
    kind: 'ram',
    tier: 'minimum',
    required: '12 GB RAM',
    actual: '16 GB',
    verdict: 'ok',
    detail: 'El juego pide 12 GB RAM y tienes 16 GB',
    ...over,
  };
}

function report(over: Partial<RequirementsReport> = {}): RequirementsReport {
  return {
    checks: [check()],
    overall: 'ok',
    proton_margin: 10,
    margin_note: 'Estos requisitos son de Windows. En Linux con Proton se pide un 10 % más de margen.',
    raw: [],
    ...over,
  };
}

describe('RequirementsCard', () => {
  it('dice en voz alta cuando el equipo no llega al mínimo', () => {
    render(
      <RequirementsCard
        report={report({
          overall: 'fail',
          checks: [check({ verdict: 'fail', actual: '8 GB', detail: 'El juego pide 12 GB RAM y tienes 8 GB.' })],
        })}
      />
    );

    expect(screen.getByText('Tu equipo no llega al mínimo de Steam')).toBeTruthy();
    expect(screen.getByText('No cumple')).toBeTruthy();
    expect(screen.getByText(/pide 12 GB RAM/)).toBeTruthy();
  });

  it('avisa cuando va justo, sin decir que falla', () => {
    render(
      <RequirementsCard
        report={report({ overall: 'tight', checks: [check({ verdict: 'tight', detail: 'justo. Baja la calidad.' })] })}
      />
    );

    expect(screen.getByText('Llegas al mínimo, pero justo')).toBeTruthy();
    expect(screen.getByText('Justo')).toBeTruthy();
  });

  it('muestra siempre el aviso del margen de Proton', () => {
    render(<RequirementsCard report={report()} />);

    expect(screen.getByText(/requisitos son de Windows/)).toBeTruthy();
  });

  it('sin requisitos publicados no dice que cumples', () => {
    render(<RequirementsCard report={report({ checks: [], overall: 'unknown' })} />);

    expect(screen.getByText(/No se han podido leer requisitos/)).toBeTruthy();
    expect(screen.queryByText(/supera el mínimo/)).toBeNull();
  });

  it("la GPU ahora si da veredicto cuando se puede comparar", () => {
    render(
      <RequirementsCard
        report={report({
          overall: "ok",
          checks: [
            check({
              kind: "gpu",
              verdict: "ok",
              required: "RX 5600 XT",
              actual: "RX 6800 XT",
              detail: "El juego pide una RX 5600 XT como mínimo y tu RX 6800 XT rinde un 128 % más.",
            }),
          ],
          raw: [{ tier: "minimum", kind: "gpu", text: "RTX 2060, RX 5600 XT" }],
        })}
      />
    );

    expect(screen.getByText("RX 5600 XT")).toBeTruthy();
    expect(screen.getByText("Cumple")).toBeTruthy();
    expect(screen.getByText(/rinde un 128 % más/)).toBeTruthy();
  });

  it("la fila de CPU/GPU es una nota, no un sin dato", () => {
    render(
      <RequirementsCard
        report={report({
          overall: "unknown",
          checks: [
            check({
              kind: "hardware",
              verdict: "unknown",
              required: "CPU: i5-8400 · GPU: RTX 2060",
              actual: "AMD Radeon RX 6800 XT · 6 nucleos / 12 hilos",
              detail: "Comparalo a mano.",
            }),
          ],
        })}
      />
    );

    expect(screen.getByText(/CPU: i5-8400/)).toBeTruthy();
    // Sin badge: "Sin dato" haria pensar que la app ha fallado.
    expect(screen.queryByText("Sin dato")).toBeNull();
  });

  it("avisa cuando Steam solo publica pocos campos", () => {
    // Con 2 checks la tarjeta parecia rota; hay que decir que Steam no da mas.
    render(
      <RequirementsCard
        report={report({
          overall: "unknown",
          checks: [check({ kind: "cpu", verdict: "unknown" }), check({ kind: "storage", verdict: "ok" })],
          raw: [
            { tier: "minimum", kind: "cpu", text: "i5" },
            { tier: "minimum", kind: "storage", text: "90 MB" },
          ],
        })}
      />
    );

    expect(screen.getByText(/Steam solo publica 2 datos de sistema/)).toBeTruthy();
  });

  it("no avisa de pocos campos cuando hay datos suficientes", () => {
    render(
      <RequirementsCard
        report={report({
          checks: [
            check({ kind: "os", verdict: "unknown" }),
            check({ kind: "cpu", verdict: "unknown" }),
            check({ kind: "gpu", verdict: "unknown" }),
            check({ kind: "ram", verdict: "ok" }),
          ],
          raw: [
            { tier: "minimum", kind: "os", text: "Windows 10" },
            { tier: "minimum", kind: "cpu", text: "i5" },
            { tier: "minimum", kind: "gpu", text: "RTX 2060" },
            { tier: "minimum", kind: "ram", text: "16 GB" },
          ],
        })}
      />
    );

    expect(screen.queryByText(/Steam solo publica/)).toBeNull();
  });

  it('distingue "cargando" de "sin datos"', () => {
    render(<RequirementsCard report={null} loading />);

    expect(screen.getByText(/Comprobando contra tu hardware/)).toBeTruthy();
    expect(screen.queryByText(/No se han podido leer requisitos/)).toBeNull();
  });

  it('enseña el error de red en vez de un veredicto', () => {
    render(<RequirementsCard report={null} error="sin conexión con Steam" />);

    expect(screen.getByText(/No se pudieron leer los requisitos: sin conexión con Steam/)).toBeTruthy();
  });

  it('una iGPU sin VRAM sale como "Sin dato", no como fallo', () => {
    render(
      <RequirementsCard
        report={report({
          overall: 'unknown',
          checks: [check({ kind: 'vram', required: '4 GB', actual: 'VRAM compartida', verdict: 'unknown' })],
        })}
      />
    );

    expect(screen.getByText('Sin dato')).toBeTruthy();
  });
});

describe('requirementsHeadline', () => {
  it('no inventa titular cuando no hay veredicto', () => {
    expect(requirementsHeadline('unknown')).toBeNull();
  });

  it('cada veredicto tiene su frase', () => {
    const seen = new Set<string>();
    for (const v of ['ok', 'tight', 'fail'] as RequirementVerdict[]) {
      const h = requirementsHeadline(v);
      expect(h).toBeTruthy();
      seen.add(h!);
    }
    expect(seen.size).toBe(3);
  });
});

describe('RequirementsCard: la CPU se compara como la GPU', () => {
  it('muestra el veredicto de la CPU, no una nota de «compáralo a mano»', () => {
    render(
      <RequirementsCard
        report={report({
          overall: 'tight',
          checks: [
            check({
              kind: 'cpu',
              tier: 'minimum',
              verdict: 'tight',
              required: 'i5-8400',
              actual: 'Ryzen 5 3600',
              detail: 'Tu Ryzen 5 3600 rinde un 4 % más por núcleo que el i5-8400 que pide, pero sin el margen extra de Proton.',
            }),
          ],
        })}
      />
    );

    // El modelo del equipo solo aparece con veredicto (antes el componente ni lo
    // pintaba, asi que un "Cumple" de CPU no decia con que comparaba).
    expect(screen.getByText('i5-8400')).toBeTruthy();
    // El modelo sale dos veces: en la fila (con flecha) y en el detalle.
    expect(screen.getAllByText(/Ryzen 5 3600/).length).toBeGreaterThan(0);
    expect(screen.getByText('Justo')).toBeTruthy();
    expect(screen.getByText(/por núcleo/)).toBeTruthy();
  });
});
