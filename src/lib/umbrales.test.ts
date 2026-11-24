import { describe, expect, it } from 'vitest';
import { MIN_NOISE_PERCENT, MIN_TIE_PERCENT } from './umbrales';
import { MIN_TIE, marginLabel, tieThreshold, withinNoise } from './noise';

/**
 * Los dos umbrales representan **conceptos distintos** y hay que poder demostrarlo.
 *
 * - `MIN_NOISE_PERCENT` es el suelo de la estimación de ruido (cuánto ruido creemos que
 *   hay como mínimo).
 * - `MIN_TIE_PERCENT` es el listón para afirmar un cambio en la interfaz (cuánto rigor
 *   exigimos).
 *
 * Se probaba con dos constantes sueltas en dos ficheros; ahora están juntas y aquí se
 * comprueba que mantienen la relación que las hace coherentes.
 */

describe('relación entre los dos umbrales', () => {
  it('el listón para decidir es mayor que el suelo del ruido', () => {
    // Si el listón bajara por debajo del ruido, se afirmarían mejoras que el propio margen
    // declara indistinguibles.
    expect(MIN_TIE_PERCENT).toBeGreaterThan(MIN_NOISE_PERCENT);
  });

  it('el suelo de ruido coincide con el del backend (3 %)', () => {
    // Replicado en Rust (`benchmarks::MIN_NOISE`). Si allí cambia, esto falla y avisa.
    expect(MIN_NOISE_PERCENT).toBe(3);
  });

  it('el alias MIN_TIE apunta al listón, no al suelo de ruido', () => {
    expect(MIN_TIE).toBe(MIN_TIE_PERCENT);
  });
});

describe('tieThreshold', () => {
  it('por debajo del listón se queda en el listón', () => {
    expect(tieThreshold(1)).toBe(MIN_TIE_PERCENT);
    expect(tieThreshold(0)).toBe(MIN_TIE_PERCENT);
  });

  it('un ruido mayor manda sobre el listón', () => {
    expect(tieThreshold(8)).toBe(8);
  });

  it('sin dato se asume el listón', () => {
    expect(tieThreshold(null)).toBe(MIN_TIE_PERCENT);
    expect(tieThreshold(undefined)).toBe(MIN_TIE_PERCENT);
    expect(tieThreshold(Number.NaN)).toBe(MIN_TIE_PERCENT);
    expect(tieThreshold(-5)).toBe(MIN_TIE_PERCENT);
  });
});

describe('withinNoise', () => {
  it('un cambio pequeño entra dentro (sin dato de ruido)', () => {
    expect(withinNoise(2)).toBe(true);
  });

  it('un cambio grande queda fuera', () => {
    expect(withinNoise(25)).toBe(false);
    expect(withinNoise(-25)).toBe(false);
  });

  it('el signo no importa', () => {
    expect(withinNoise(-3, 10)).toBe(withinNoise(3, 10));
  });
});

describe('marginLabel', () => {
  it('formatea con un decimal', () => {
    expect(marginLabel(4.25)).toBe('±4.3%');
  });

  it('sin dato no devuelve texto', () => {
    expect(marginLabel(null)).toBeNull();
    expect(marginLabel(0)).toBeNull();
  });
});
