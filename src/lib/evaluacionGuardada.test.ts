import { describe, it, expect } from 'vitest';
import {
  estadoDeEvaluacionGuardada,
  veredictoGuardado,
  ganadorDeEstado,
  datosObservados,
  metricasGuardadas,
} from './evaluacionGuardada';
import type { EvaluacionPersistida, SavedComparison } from './api';
import type { BenchmarkComparison } from './types';

/**
 * Guardia contra la reinterpretación automática [5.4.1.B].
 *
 * Si alguien vuelve a llamar a `interpretar()` sobre una comparación guardada, el veredicto
 * cambiaría al actualizar las reglas y una prueba antigua aparecería con una conclusión que
 * nunca tuvo. Estas pruebas fijan que la presentación depende **solo** del dato guardado.
 */

function evaluacion(campos: Partial<EvaluacionPersistida> = {}): EvaluacionPersistida {
  return {
    metodologia_interpretacion: 'v1',
    evaluada_en: '2026-01-02 11:00:00',
    referencia: { id: 'b1' },
    candidata: { id: 't1' },
    estado: 'mejora',
    confianza: 'observada',
    condiciones: { compatibilidad_metodologica: 'establecida' },
    ...campos,
  };
}

const guardada = (campos: Partial<SavedComparison>): Pick<SavedComparison, 'evaluacion'> =>
  campos as Pick<SavedComparison, 'evaluacion'>;

describe('estado de la evaluación guardada', () => {
  it('sin evaluación es «no registrada», no un veredicto neutro', () => {
    expect(estadoDeEvaluacionGuardada({ evaluacion: null })).toBe('no_registrada');
    expect(veredictoGuardado({ evaluacion: null })).toEqual({ tipo: 'no_registrada' });
  });

  it('con evaluación legible es «soportada»', () => {
    expect(estadoDeEvaluacionGuardada({ evaluacion: evaluacion() })).toBe('soportada');
  });

  it('con metodología futura es «no soportada»', () => {
    const v9 = evaluacion({ metodologia_interpretacion: 'v9' });
    expect(estadoDeEvaluacionGuardada({ evaluacion: v9 })).toBe('no_soportada');
    expect(veredictoGuardado({ evaluacion: v9 })).toEqual({
      tipo: 'no_soportada',
      motivo: 'v9',
    });
  });

  it('un estado desconocido no se traduce a uno conocido', () => {
    const raro = evaluacion({ estado: 'estado_del_futuro' });
    expect(estadoDeEvaluacionGuardada({ evaluacion: raro })).toBe('no_soportada');
  });

  it('malformada es distinta de ausente y de insuficiente', () => {
    expect(estadoDeEvaluacionGuardada({ evaluacion: null })).toBe('no_registrada');
    expect(estadoDeEvaluacionGuardada({ evaluacion: evaluacion({ estado: 'datos_insuficientes' }) }))
      .toBe('soportada');
    expect(veredictoGuardado({ evaluacion: evaluacion({ estado: 'datos_insuficientes' }) }))
      .toMatchObject({ tipo: 'datos_insuficientes' });
  });

  it('la clasificación del backend manda sobre cualquier deducción local', () => {
    // El backend dice «malformada»; el frontend no la reconvierte en «no registrada».
    expect(
      estadoDeEvaluacionGuardada({
        evaluacion: null,
        estado_de_la_evaluacion: 'malformada',
      } as Pick<SavedComparison, 'evaluacion' | 'estado_de_la_evaluacion'>)
    ).toBe('malformada');
  });
});

describe('el veredicto guardado no se recalcula', () => {
  it('cada estado guardado produce su propio tipo, sin mirar las cifras', () => {
    const estados = ['mejora', 'empeora', 'mixto', 'sin_cambio', 'solo_descriptiva'];
    for (const estado of estados) {
      const v = veredictoGuardado({ evaluacion: evaluacion({ estado }) });
      expect(v.tipo).toBe(estado === 'solo_descriptiva' ? 'solo_descriptiva' : 'veredicto');
    }
  });

  it('un veredicto de mejora NO depende de que los FPS hayan subido', () => {
    // La evaluación dice «mejora» aunque las cifras digan lo contrario: manda lo guardado,
    // no una resta. Si alguien reintrodujera la interpretación, esto cambiaría.
    const ev = evaluacion({ estado: 'mejora' });
    expect(veredictoGuardado({ evaluacion: ev }).tipo).toBe('veredicto');
    const conCifras = {
      ...ev,
      metricas: [
        {
          clave: 'avg_fps',
          antes: 100,
          despues: 50,
          diferencia: -50,
          diferencia_pct: -50,
          direccion: 'baja' as const,
          preferencia: 'favor_de_referencia' as const,
        },
      ],
    };
    expect(veredictoGuardado({ evaluacion: conCifras }).tipo).toBe('veredicto');
  });

  it('sin evaluación no se deduce ninguna ganadora', () => {
    const v = veredictoGuardado({ evaluacion: null });
    expect(v.tipo).toBe('no_registrada');
    // «No registrada» no tiene estado del que sacar ganadora.
  });

  it('solo los estados de veredicto producen ganadora', () => {
    expect(ganadorDeEstado('mejora')).toBe('despues');
    expect(ganadorDeEstado('empeora')).toBe('antes');
    expect(ganadorDeEstado('sin_cambio')).toBe('ninguno');
    expect(ganadorDeEstado('mixto')).toBe('ninguno');
    expect(ganadorDeEstado('datos_insuficientes')).toBe('ninguno');
    expect(ganadorDeEstado('solo_descriptiva')).toBe('ninguno');
    expect(ganadorDeEstado('codigo_del_futuro')).toBe('ninguno');
  });
});

describe('datos observados', () => {
  it('expone solo cifras almacenadas, sin inventar ceros', () => {
    const c = {
      baseline: { avg_fps: 60 },
      test: { avg_fps: 66 },
      fps_change_percent: 10,
      fps_1_percent_change_percent: 5,
      fps_0_1_percent_change_percent: 2,
      frame_time_change_percent: -3,
    } as unknown as BenchmarkComparison;
    expect(datosObservados(c)).toEqual({
      fpsAntes: 60,
      fpsDespues: 66,
      diferenciaPct: 10,
      deltaLow1: 5,
      deltaLow01: 2,
      deltaFrameTime: -3,
    });
  });

  it('un dato que no consta es null, nunca 0', () => {
    const c = {
      baseline: { avg_fps: 60 },
      test: { avg_fps: 66 },
      fps_change_percent: 10,
    } as unknown as BenchmarkComparison;
    const d = datosObservados(c);
    expect(d.deltaLow1).toBeNull();
    expect(d.deltaLow01).toBeNull();
    expect(d.deltaFrameTime).toBeNull();
  });
});

describe('métricas guardadas', () => {
  it('conservan dirección y preferencia por separado', () => {
    const ev = evaluacion({
      metricas: [
        {
          clave: 'frame_time_avg',
          antes: 16,
          despues: 20,
          diferencia: 4,
          diferencia_pct: 25,
          direccion: 'sube',
          preferencia: 'favor_de_referencia',
        },
      ],
    });
    const [m] = metricasGuardadas(ev);
    // Subir el frametime es peor: la dirección y la preferencia no coinciden y no deben
    // colapsarse en un único «mejor/peor».
    expect(m.direccion).toBe('sube');
    expect(m.preferencia).toBe('favor_de_referencia');
  });
});

describe('interfaz de datos guardados', () => {
  it('una comparación guardada expone el estado pero no una Interpretacion', () => {
    const c = guardada({ evaluacion: evaluacion() });
    const v = veredictoGuardado(c);
    // Lo que se obtiene es un tipo de presentación, no el objeto del intérprete.
    expect(v).not.toHaveProperty('margen');
    expect(v).not.toHaveProperty('chipConfianza');
    expect(v).not.toHaveProperty('ganador');
  });
});
