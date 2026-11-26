import { describe, it, expect } from 'vitest';
import { duracionDe, duracionEnSegundos, duraMenosDe, textoDeDuracion } from './duracion';
import type { Benchmark } from './types';

/**
 * Los tres estados del contrato de duración [5.4.1.A].
 *
 * El caso que más importa es el tercero: una grabación **v2 sin eje temporal** guarda
 * `duration_seconds = 0` por compatibilidad, y ese cero **no** significa «cero segundos».
 */
function bench(campos: Partial<Pick<Benchmark, 'duration_seconds' | 'duracion_medida' | 'metodologia_calculo'>>) {
  return {
    duration_seconds: 0,
    duracion_medida: null,
    metodologia_calculo: null,
    ...campos,
  } as Pick<Benchmark, 'duration_seconds' | 'duracion_medida' | 'metodologia_calculo'>;
}

describe('contrato de duración', () => {
  it('histórica sin metodología: usa duration_seconds tal cual', () => {
    const d = duracionDe(bench({ duration_seconds: 59.75, metodologia_calculo: null }));
    expect(d).toEqual({ tipo: 'historica', segundos: 59.75 });
    expect(duracionEnSegundos(bench({ duration_seconds: 59.75 }))).toBe(59.75);
  });

  it('v2 con duración medida: manda duracion_medida', () => {
    const b = bench({ duration_seconds: 0, duracion_medida: 59.85, metodologia_calculo: 'v2' });
    expect(duracionDe(b)).toEqual({ tipo: 'medida', segundos: 59.85 });
  });

  it('v2 sin duración medida: desconocida, NO cero ni respaldo a duration_seconds', () => {
    // El caso trampa: duration_seconds=0 y metodología v2 declarada.
    const b = bench({ duration_seconds: 0, duracion_medida: null, metodologia_calculo: 'v2' });
    expect(duracionDe(b)).toEqual({ tipo: 'desconocida' });
    expect(duracionEnSegundos(b)).toBeNull();
  });

  it('v2 sin duración medida no cae a un duration_seconds no nulo', () => {
    // Aunque alguien rellenara duration_seconds, en v2 sin eje no se usa como respaldo.
    const b = bench({ duration_seconds: 42, duracion_medida: null, metodologia_calculo: 'v2' });
    expect(duracionEnSegundos(b)).toBeNull();
  });

  it('una duración no positiva o no finita no es un dato utilizable', () => {
    expect(duracionEnSegundos(bench({ duration_seconds: 0 }))).toBeNull();
    expect(duracionEnSegundos(bench({ duration_seconds: -5 }))).toBeNull();
    expect(duracionEnSegundos(bench({ duration_seconds: Number.NaN }))).toBeNull();
    expect(duracionEnSegundos(bench({ duration_seconds: Number.POSITIVE_INFINITY }))).toBeNull();
    expect(
      duracionEnSegundos(bench({ duracion_medida: Number.NaN, metodologia_calculo: 'v2' }))
    ).toBeNull();
  });

  it('duraMenosDe no afirma nada cuando la duración no consta', () => {
    const b = bench({ duration_seconds: 0, metodologia_calculo: 'v2' });
    // Sin dato no se puede decir que dure menos de 20 s: sería inventarlo.
    expect(duraMenosDe(b, 20)).toBe(false);
    expect(duraMenosDe(bench({ duration_seconds: 15 }), 20)).toBe(true);
    expect(duraMenosDe(bench({ duration_seconds: 60 }), 20)).toBe(false);
  });

  it('el texto de la interfaz dice que no consta en vez de «0 s»', () => {
    const fmt = (s: number) => `${s.toFixed(1)} s`;
    expect(textoDeDuracion(bench({ duration_seconds: 59.75 }), fmt)).toBe('59.8 s');
    expect(textoDeDuracion(bench({ duration_seconds: 0, metodologia_calculo: 'v2' }), fmt)).toBe(
      'duración no registrada'
    );
  });
});
