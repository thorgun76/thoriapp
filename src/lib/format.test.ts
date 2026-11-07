import { describe, expect, it } from 'vitest';
import { fmtDuration, formatTimestamp } from './format';

describe('fmtDuration', () => {
  it('menos de un minuto en segundos', () => {
    expect(fmtDuration(0)).toBe('0 s');
    expect(fmtDuration(30)).toBe('30 s');
    expect(fmtDuration(59)).toBe('59 s');
  });

  it('minutos exactos sin resto de segundos', () => {
    expect(fmtDuration(60)).toBe('1 min');
    expect(fmtDuration(120)).toBe('2 min');
  });

  it('minutos con resto', () => {
    expect(fmtDuration(90)).toBe('1 min 30 s');
    expect(fmtDuration(125)).toBe('2 min 5 s');
  });

  it('redondea los segundos fraccionarios', () => {
    expect(fmtDuration(44.6)).toBe('45 s');
    expect(fmtDuration(59.4)).toBe('59 s');
  });
});

describe('formatTimestamp', () => {
  it('formatea una fecha ISO', () => {
    const out = formatTimestamp('2026-11-07T11:36:12');
    expect(out).toMatch(/07\/11\/2026/);
    expect(out).toMatch(/11:36/);
  });

  it('devuelve el texto original si no es una fecha válida', () => {
    expect(formatTimestamp('no es una fecha')).toBe('no es una fecha');
    expect(formatTimestamp('')).toBe('');
  });
});
