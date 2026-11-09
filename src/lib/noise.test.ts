import { describe, expect, it } from 'vitest';
import { MIN_TIE, marginLabel, tieThreshold, withinNoise } from './noise';

describe('tieThreshold', () => {
  it('usa el mínimo cuando no hay dato de ruido', () => {
    expect(tieThreshold(undefined)).toBe(MIN_TIE);
    expect(tieThreshold(null)).toBe(MIN_TIE);
  });

  it('nunca baja del mínimo aunque el ruido medido sea pequeño', () => {
    expect(tieThreshold(0.5)).toBe(MIN_TIE);
    expect(tieThreshold(0)).toBe(MIN_TIE);
  });

  it('sube por encima del mínimo si la grabación era ruidosa', () => {
    expect(tieThreshold(9.4)).toBe(9.4);
    expect(tieThreshold(12)).toBe(12);
  });

  it('ignora valores imposibles', () => {
    expect(tieThreshold(-3)).toBe(MIN_TIE);
    expect(tieThreshold(Number.NaN)).toBe(MIN_TIE);
    expect(tieThreshold(Number.POSITIVE_INFINITY)).toBe(MIN_TIE);
  });
});

describe('marginLabel', () => {
  it('formatea el margen con un decimal', () => {
    expect(marginLabel(4.24)).toBe('±4.2%');
    expect(marginLabel(10)).toBe('±10.0%');
  });

  it('devuelve null cuando no hay margen que mostrar', () => {
    expect(marginLabel(null)).toBeNull();
    expect(marginLabel(undefined)).toBeNull();
    expect(marginLabel(0)).toBeNull();
  });
});

describe('withinNoise', () => {
  it('considera igual una diferencia menor que el mínimo', () => {
    expect(withinNoise(3.2, null)).toBe(true);
    expect(withinNoise(-4.9, 2)).toBe(true);
  });

  it('considera una diferencia real si supera el margen medido', () => {
    expect(withinNoise(8, 2)).toBe(false);
    expect(withinNoise(-11.5, 9)).toBe(false);
  });

  it('con grabaciones ruidosas exige más diferencia para no llamarlo ruido', () => {
    // 6 % es mejora con ruido 3 %, pero está dentro del ruido cuando el margen es 8 %.
    expect(withinNoise(6, 3)).toBe(false);
    expect(withinNoise(6, 8)).toBe(true);
  });
});
