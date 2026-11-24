import { describe, expect, it } from 'vitest';
import {
  estabilidad,
  estabilidadDe,
  estabilidadLabel,
  puntosDeDiferencia,
  low01Reliability,
} from './estabilidad';

/**
 * El índice de estabilidad decide parte del veredicto, así que sus casos límite importan:
 * un `null` mal devuelto como 0 diría «pésimo» donde no hay medida.
 */

describe('estabilidad: fórmula', () => {
  it('reproduce la fórmula original', () => {
    // c1 = 18/60 = 0.30, c01 = 12/60 = 0.20 → (2·0.30 + 0.20)/3 · 100 = 26.67
    expect(estabilidad(60, 18, 12)).toBeCloseTo(26.67, 2);
  });

  it('un juego con percentiles iguales al promedio da 100', () => {
    expect(estabilidad(60, 60, 60)).toBe(100);
  });

  it('pondera el 1 % low el doble que el 0,1 % low', () => {
    // Con c1 = 0.5 y c01 = 0.8: (2·0.5 + 0.8)/3 = 0.6
    expect(estabilidad(100, 50, 80)).toBeCloseTo(60, 5);
  });
});

describe('estabilidad: valores inválidos no se presentan como medidas', () => {
  it('FPS medios cero o negativos no dan índice', () => {
    expect(estabilidad(0, 10, 5)).toBeNull();
    expect(estabilidad(-30, 10, 5)).toBeNull();
  });

  it('NaN e infinito no dan índice', () => {
    expect(estabilidad(NaN, 10, 5)).toBeNull();
    expect(estabilidad(Infinity, 10, 5)).toBeNull();
    expect(estabilidad(60, NaN, 5)).toBeNull();
    expect(estabilidad(60, 10, Infinity)).toBeNull();
  });

  it('percentiles ausentes o negativos no dan índice', () => {
    expect(estabilidad(60, null, 5)).toBeNull();
    expect(estabilidad(60, undefined, 5)).toBeNull();
    expect(estabilidad(60, -1, 5)).toBeNull();
    expect(estabilidad(60, 10, -5)).toBeNull();
  });

  it('nunca pasa de 100 aunque los percentiles superen el promedio', () => {
    // Percentiles bajos > promedio es imposible físicamente, pero un log corrupto
    // puede producirlo: se satura en vez de mostrar 143/100.
    expect(estabilidad(50, 90, 80)).toBe(100);
  });

  it('nunca baja de 0', () => {
    expect(estabilidad(60, 0, 0)).toBe(0);
  });

  it('estabilidadDe propaga los inválidos a null', () => {
    expect(estabilidadDe({ avg_fps: 0, fps_1_percent_low: 10, fps_0_1_percent_low: 5 })).toBeNull();
  });
});

describe('estabilidad: etiquetas', () => {
  it('sin datos no se inventa una etiqueta numérica', () => {
    expect(estabilidadLabel(null)).toBe('Sin datos');
  });

  it('no dice "fluido" aunque el número sea alto', () => {
    // Un juego a 30 FPS clavados da 100/100 pero no está "fluido": el índice es relativo.
    for (const s of [0, 34, 35, 50, 65, 100]) {
      expect(estabilidadLabel(s)).not.toMatch(/fluid/i);
    }
    expect(estabilidadLabel(100)).toBe('Muy estable');
    expect(estabilidadLabel(80)).toBe('Muy estable');
    expect(estabilidadLabel(55)).toBe('Estable');
    expect(estabilidadLabel(40)).toBe('Algo irregular');
    expect(estabilidadLabel(10)).toBe('Muy irregular');
  });
});

describe('puntos de diferencia', () => {
  it('es una resta en puntos, no un porcentaje', () => {
    expect(puntosDeDiferencia(28, 64)).toBe(36);
    expect(puntosDeDiferencia(64, 28)).toBe(-36);
  });

  it('con un lado sin datos no hay diferencia', () => {
    expect(puntosDeDiferencia(null, 64)).toBeNull();
    expect(puntosDeDiferencia(28, null)).toBeNull();
  });
});

describe('fiabilidad del 0,1 % low', () => {
  it('una grabación de 30 s no sostiene el 0,1 % low', () => {
    // ~300 muestras a 10 Hz → el 0,1 % es 0,3 muestras: se apoya en un único frame.
    const r = low01Reliability(300);
    expect(r.fiable).toBe(false);
    // Nunca 0: el percentil se interpola entre las dos peores muestras, así que como
    // poco depende de una. Decir «~0 muestras» era un redondeo que se leía como
    // «no hay datos».
    expect(r.muestrasExtremas).toBe(1);
    expect(r.aviso).toContain('1 sola muestra');
    expect(r.aviso).not.toContain('~0');
  });

  it('493 muestras se apoyan en 1 muestra, no en 0 ni en «~0»', () => {
    const r = low01Reliability(493);
    expect(r.fiable).toBe(false);
    expect(r.muestrasExtremas).toBe(1);
    expect(r.aviso).toContain('de 493');
  });

  it('con muchas muestras dentro de lo poco fiable, dice cuántas son', () => {
    // 5.000 muestras → 5 extremas: son pocas, pero no una.
    const r = low01Reliability(5_000);
    expect(r.muestrasExtremas).toBe(5);
    expect(r.aviso).toContain('5 de las 5000 muestras');
  });

  it('hacen falta muchísimas muestras para que sea fiable', () => {
    expect(low01Reliability(5_000).fiable).toBe(false);
    expect(low01Reliability(10_000).fiable).toBe(true);
    expect(low01Reliability(20_000).aviso).toBeNull();
  });

  it('sin dato de muestras se avisa en vez de suponer', () => {
    expect(low01Reliability(null).fiable).toBe(false);
    expect(low01Reliability(undefined).aviso).toMatch(/No se registró/);
    expect(low01Reliability(0).fiable).toBe(false);
  });

  it('valores corruptos no rompen el cálculo', () => {
    expect(low01Reliability(NaN).muestras).toBe(0);
    expect(low01Reliability(-5).muestras).toBe(0);
  });
});
