import { describe, expect, it } from 'vitest';
import type { EvaluacionPersistida, SavedComparison } from './api';
import type { BenchmarkComparison } from './types';
import {
  datosDescriptivos,
  estadoDeEvaluacionGuardada,
  presentarEvaluacion,
  presentarGuardada,
  presentarSinEvaluacion,
  textoDeEtiqueta,
} from './presentacionEvaluacion';

/**
 * Pruebas del **modelo único** de presentación [C.3.b].
 *
 * Lo que se defiende aquí es que la preferencia solo pueda venir del motor: si alguna de
 * estas pruebas falla, hay una ruta por la que una vista puede deducir un veredicto de las
 * cifras, y eso es justo lo que C.3 cierra.
 *
 * Son pruebas de contrato, no de evaluación: comprueban que se copia `estado` y que no se
 * inventa nada cuando el motor no dijo nada.
 */

function comparacion(a = 100, b = 120): BenchmarkComparison {
  return {
    baseline: {
      id: 'a',
      game_name: 'Palworld',
      avg_fps: a,
      fps_1_percent_low: a * 0.7,
      fps_0_1_percent_low: a * 0.6,
      duration_seconds: 30,
    },
    test: {
      id: 'b',
      game_name: 'Palworld',
      avg_fps: b,
      fps_1_percent_low: b * 0.7,
      fps_0_1_percent_low: b * 0.6,
      duration_seconds: 30,
    },
  } as unknown as BenchmarkComparison;
}

function evaluacion(over: Partial<EvaluacionPersistida> = {}): EvaluacionPersistida {
  return {
    metodologia_interpretacion: 'v1',
    evaluada_en: '2026-01-02 11:00:00',
    referencia: { id: 'a' },
    candidata: { id: 'b' },
    estado: 'mejora',
    confianza: 'provisional',
    condiciones: {
      margen_porcentaje: 5,
      ruido_combinado_porcentaje: 3.2,
      compatibilidad_metodologica: 'establecida',
    },
    motivos: [],
    limitaciones: [],
    metricas: [],
    ...over,
  } as unknown as EvaluacionPersistida;
}

describe('la preferencia solo la decide el motor', () => {
  const conGanadora: Array<[string, 'candidata' | 'referencia' | null]> = [
    ['mejora', 'candidata'],
    ['empeora', 'referencia'],
  ];

  it.each(conGanadora)('estado %s favorece a %s', (estado, favor) => {
    const m = presentarEvaluacion(evaluacion({ estado }), comparacion());
    expect(m.quienFavorece).toBe(favor);
    expect(m.titulo).toBe(estado);
  });

  const sinGanadora = ['mixto', 'sin_cambio', 'datos_insuficientes', 'solo_descriptiva'];

  it.each(sinGanadora)('estado %s no tiene ganadora, por mucho que las cifras suban', (estado) => {
    // La comparación sube de 100 a 120 FPS. Aun así, si el motor no dijo «mejora», la
    // vista no puede decir quién gana.
    const m = presentarEvaluacion(evaluacion({ estado }), comparacion(100, 120));
    expect(m.quienFavorece).toBeNull();
  });

  it('sin cambio no se presenta como equivalencia', () => {
    const m = presentarEvaluacion(evaluacion({ estado: 'sin_cambio' }), comparacion());
    expect(m.veredicto).toEqual({ tipo: 'sin_ganadora', motivo: 'sin_cambio' });
    // Ni «iguales» ni «equivalente»: el motor no midió equivalencia.
    expect(JSON.stringify(m)).not.toMatch(/equivale|iguales/i);
  });

  it('un estado desconocido no cae en un veredicto por defecto', () => {
    const m = presentarEvaluacion(evaluacion({ estado: 'mejora_dudosa' }), comparacion());
    expect(m.veredicto.tipo).toBe('no_legible');
    expect(m.quienFavorece).toBeNull();
    expect(textoDeEtiqueta(m.veredicto.tipo === 'no_legible' ? m.veredicto.detalle : null)).toBe(
      'mejora_dudosa'
    );
  });
});

describe('los cuatro estados de una evaluación registrada', () => {
  it('sin evaluación es histórica, no malformada', () => {
    expect(estadoDeEvaluacionGuardada({ evaluacion: null })).toBe('no_registrada');
  });

  it('con metodología v1 y completa está soportada', () => {
    expect(
      estadoDeEvaluacionGuardada({ evaluacion: evaluacion(), estado_de_la_evaluacion: undefined })
    ).toBe('soportada');
  });

  it('de otra versión está no soportada', () => {
    expect(
      estadoDeEvaluacionGuardada({
        evaluacion: evaluacion({ metodologia_interpretacion: 'v2' as 'v1' }),
        estado_de_la_evaluacion: undefined,
      })
    ).toBe('no_soportada');
  });

  it('v1 pero incompleta está malformada, no no-soportada', () => {
    // Un registro truncado no es «de otra versión»: está roto, y decir otra cosa miente
    // sobre su procedencia.
    expect(
      estadoDeEvaluacionGuardada({
        evaluacion: { metodologia_interpretacion: 'v1', estado: 'mejora' } as EvaluacionPersistida,
        estado_de_la_evaluacion: undefined,
      })
    ).toBe('malformada');
  });

  it('la clasificación de Rust manda cuando viene', () => {
    // Aunque el registro parezca completo, si Rust dice otra cosa se le cree a Rust.
    expect(
      estadoDeEvaluacionGuardada({
        evaluacion: evaluacion(),
        estado_de_la_evaluacion: 'malformada',
      })
    ).toBe('malformada');
  });
});

describe('la evaluación presentada es la misma se haya guardado o no', () => {
  it('vista previa y guardada producen el mismo modelo salvo el origen', () => {
    const c = comparacion();
    const ev = evaluacion();

    const previa = presentarEvaluacion(ev, c, { origen: 'evaluada', estado: 'soportada' });
    const guardada = presentarGuardada({
      id: 'c1',
      comparison: c,
      evaluacion: ev,
    } as unknown as SavedComparison)!;

    // El mismo dato, el mismo modelo: no hay una presentación para lo nuevo y otra para
    // lo guardado.
    expect({ ...guardada, origen: 'evaluada' }).toEqual(previa);
  });
});

describe('presentarGuardada no lanza nunca', () => {
  const base = { id: 'c1', comparison: comparacion() };

  it('con evaluación legible presenta el modelo', () => {
    const g = { ...base, evaluacion: evaluacion() } as unknown as SavedComparison;
    expect(presentarGuardada(g)?.titulo).toBe('mejora');
  });

  it('con evaluación ilegible dice «malformada» en vez de romper la pantalla', () => {
    const g = {
      ...base,
      evaluacion: { metodologia_interpretacion: 'v1', estado: 'mejora' },
    } as unknown as SavedComparison;
    const m = presentarGuardada(g);
    expect(m?.estado).toBe('malformada');
    expect(m?.quienFavorece).toBeNull();
  });

  it('histórica: sin veredicto y sin ganadora', () => {
    const g = { ...base, evaluacion: null } as unknown as SavedComparison;
    const m = presentarGuardada(g);
    expect(m?.estado).toBe('no_registrada');
    expect(m?.quienFavorece).toBeNull();
    expect(m?.veredicto).toEqual({ tipo: 'sin_evaluacion', motivo: 'historica' });
  });
});

describe('lo descriptivo no se convierte en veredicto', () => {
  it('la estabilidad se calcula de lo medido y no tiene preferencia', () => {
    const d = datosDescriptivos(comparacion(100, 60));
    // Existe y es un número: es un índice, no un juicio.
    expect(typeof d.estabilidad?.referencia?.indice).toBe('number');
    expect(typeof d.estabilidad?.candidata?.indice).toBe('number');
    expect(d).not.toHaveProperty('mejor');
    expect(d).not.toHaveProperty('ganadora');
  });

  it('magnitud y dirección no arrastran preferencia', () => {
    const m = presentarEvaluacion(
      evaluacion({
        estado: 'mixto',
        metricas: [
          {
            clave: 'avg_fps',
            antes: 100,
            despues: 120,
            diferencia: 20,
            diferencia_pct: 20,
            direccion: 'sube',
            preferencia: 'no_establecida',
            motivos: [],
            limitaciones: [],
          },
        ],
      }),
      comparacion()
    );
    const [metrica] = m.metricas;
    expect(metrica.direccion?.codigo).toBe('sube');
    expect(metrica.diferenciaPct).toBe(20);
    expect(metrica.preferencia?.codigo).toBe('no_establecida');
    // Aunque la métrica suba un 20 %, la comparación es mixta: no hay ganadora.
    expect(m.quienFavorece).toBeNull();
  });
});

describe('ruido combinado y margen aplicado son datos distintos', () => {
  it('se presentan por separado y no se confunden', () => {
    const m = presentarEvaluacion(evaluacion(), comparacion());
    expect(m.margenAplicado).toBe(5);
    expect(m.ruidoCombinado).toBe(3.2);
    expect(m.margenAplicado).not.toBe(m.ruidoCombinado);
  });

  it('si falta el margen no se asume el 5 % por defecto', () => {
    const m = presentarEvaluacion(
      evaluacion({ condiciones: { compatibilidad_metodologica: 'establecida' } as never }),
      comparacion()
    );
    expect(m.margenAplicado).toBeNull();
  });
});

describe('los códigos desconocidos se enseñan crudos', () => {
  it('un motivo que esta versión no conoce no se traduce a otro', () => {
    const m = presentarEvaluacion(
      evaluacion({ motivos: ['motivo_del_futuro'] as never }),
      comparacion()
    );
    expect(m.motivos[0].codigo).toBe('motivo_del_futuro');
    expect(m.motivos[0].texto).toBeNull();
    // El texto que se muestra es el código, no una traducción aproximada.
    expect(textoDeEtiqueta(m.motivos[0])).toBe('motivo_del_futuro');
  });

  it('la confianza usa los códigos de Rust, no alta/media/baja', () => {
    const m = presentarEvaluacion(evaluacion({ confianza: 'provisional' as never }), comparacion());
    expect(m.confianza?.texto).toBe('Provisional');
    expect(JSON.stringify(m)).not.toMatch(/\balta\b|\bbaja\b/);
  });
});

describe('histórica: se enseña lo medido, no se inventa narrativa', () => {
  it('el modelo histórico conserva las cifras y no tiene titulo de veredicto', () => {
    const m = presentarSinEvaluacion(comparacion(100, 120));
    expect(m.estado).toBe('no_registrada');
    expect(m.descriptivas.fpsAntes).toBe(100);
    expect(m.descriptivas.fpsDespues).toBe(120);
    expect(m.quienFavorece).toBeNull();
    expect(m.metricas).toEqual([]);
    expect(m.confianza).toBeNull();
  });
});

/**
 * Regresiones del **caso real ejecutado contra el motor** [C.3, sexto defecto].
 *
 * Cifras y códigos copiados de una ejecución de `evaluar()` sobre una copia del árbol
 * (no inventadas): FPS 100→120, 1 % low 80→96, 0,1 % low 60→30 con `sample_count=2000`,
 * que deja el 0,1 % low **no elegible**. El motor devuelve `mejora` con
 * `confianza=observada`, margen 5 %, ruido combinado 3 %, y las limitaciones
 * `no_elegible`, `apoyo_insuficiente`, `perdida_observada_sin_evaluar`, `evaluacion_parcial`.
 *
 * El estado lo decide Rust y aquí no se toca. Lo que se fija es que **la presentación no
 * corona** una decisión que solo cubrió parte de las métricas.
 */
describe('cobertura parcial: el cambio se afirma, la victoria global no', () => {
  /** Evaluación tal como la produjo el motor en el caso ejecutado. */
  function casoParcial(): EvaluacionPersistida {
    return evaluacion({
      estado: 'mejora',
      confianza: 'observada',
      limitaciones: [
        'no_elegible',
        'apoyo_insuficiente',
        'perdida_observada_sin_evaluar',
        'evaluacion_parcial',
      ],
      metricas: [
        {
          clave: 'avg_fps',
          antes: 100,
          despues: 120,
          diferencia: 20,
          diferencia_pct: 20,
          direccion: 'sube',
          preferencia: 'favor_de_candidata',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
        {
          clave: 'fps_1_percent_low',
          antes: 80,
          despues: 96,
          diferencia: 16,
          diferencia_pct: 20,
          direccion: 'sube',
          preferencia: 'favor_de_candidata',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
        {
          clave: 'fps_0_1_percent_low',
          antes: 60,
          despues: 30,
          diferencia: -30,
          diferencia_pct: -50,
          direccion: 'baja',
          preferencia: 'no_establecida',
          confianza: null,
          motivos: [],
          limitaciones: ['no_elegible', 'apoyo_insuficiente'],
        },
      ],
    });
  }

  it('no hay ganadora global ni corona con cobertura parcial', () => {
    const m = presentarEvaluacion(casoParcial(), comparacion(100, 120));
    expect(m.veredicto.tipo).toBe('cambio_parcial');
    expect(m.quienFavorece).toBeNull();
    // El tono no pinta una victoria que no se puede afirmar.
    expect(m.tono).toBe('neutro');
  });

  it('sí se afirma el cambio observado en lo evaluado', () => {
    const m = presentarEvaluacion(casoParcial(), comparacion(100, 120));
    expect(m.veredicto).toMatchObject({ tipo: 'cambio_parcial', favor: 'candidata' });
    // Las dos métricas elegibles favorecen a la candidata; se presentan como tales.
    expect(m.metricas[0].preferencia?.codigo).toBe('favor_de_candidata');
    expect(m.metricas[1].preferencia?.codigo).toBe('favor_de_candidata');
  });

  it('la cobertura es parcial y cuenta solo lo que entró en la decisión', () => {
    const m = presentarEvaluacion(casoParcial(), comparacion(100, 120));
    expect(m.cobertura.parcial).toBe(true);
    expect(m.cobertura.evaluadas).toBe(2);
    expect(m.cobertura.totales).toBe(3);
  });

  it('la pérdida en la métrica excluida se conserva y no desaparece', () => {
    const m = presentarEvaluacion(casoParcial(), comparacion(100, 120));
    expect(m.cobertura.perdidaEnMetricaExcluida).toBe(true);
    const low01 = m.metricas.find((x) => x.clave === 'fps_0_1_percent_low')!;
    // La magnitud se enseña (cayó un 50 %) aunque no haya preferencia.
    expect(low01.diferenciaPct).toBe(-50);
    expect(low01.preferencia?.codigo).toBe('no_establecida');
    // Y no lleva chip de ganadora.
    expect(low01.preferencia?.codigo).not.toBe('favor_de_referencia');
  });

  it('empeora con cobertura parcial tampoco corona (simétrico)', () => {
    const ev = { ...casoParcial(), estado: 'empeora' } as EvaluacionPersistida;
    const m = presentarEvaluacion(ev, comparacion(120, 100));
    expect(m.veredicto).toMatchObject({ tipo: 'cambio_parcial', favor: 'referencia' });
    expect(m.quienFavorece).toBeNull();
  });

  it('con cobertura completa SÍ hay ganadora', () => {
    const ev = evaluacion({
      estado: 'mejora',
      confianza: 'observada',
      limitaciones: [],
      metricas: [
        {
          clave: 'avg_fps',
          antes: 100,
          despues: 120,
          diferencia: 20,
          diferencia_pct: 20,
          direccion: 'sube',
          preferencia: 'favor_de_candidata',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
      ],
    });
    const m = presentarEvaluacion(ev, comparacion(100, 120));
    expect(m.cobertura.parcial).toBe(false);
    expect(m.veredicto.tipo).toBe('ganadora');
    expect(m.quienFavorece).toBe('candidata');
    expect(m.tono).toBe('positivo');
  });

  it('una métrica elegible e indistinta cuenta como evaluada', () => {
    // dentro del margen: no favorece a nadie, pero sí se juzgó.
    const ev = evaluacion({
      estado: 'mejora',
      limitaciones: [],
      metricas: [
        {
          clave: 'avg_fps',
          antes: 100,
          despues: 120,
          diferencia: 20,
          diferencia_pct: 20,
          direccion: 'sube',
          preferencia: 'favor_de_candidata',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
        {
          clave: 'fps_1_percent_low',
          antes: 80,
          despues: 81,
          diferencia: 1,
          diferencia_pct: 1.25,
          direccion: 'sube',
          preferencia: 'indistinto',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
      ],
    });
    const m = presentarEvaluacion(ev, comparacion(100, 120));
    // Contar solo las que favorecen a alguien daría 1; decidieron 2.
    expect(m.cobertura.evaluadas).toBe(2);
  });

  it('mixto con 0,1 % low elegible sigue sin ganadora', () => {
    // Del motor con `sample_count=20000`: el 0,1 % low pasa a elegible y decide en contra.
    const ev = evaluacion({
      estado: 'mixto',
      limitaciones: [],
      metricas: [
        {
          clave: 'avg_fps',
          antes: 100,
          despues: 120,
          diferencia: 20,
          diferencia_pct: 20,
          direccion: 'sube',
          preferencia: 'favor_de_candidata',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
        {
          clave: 'fps_0_1_percent_low',
          antes: 60,
          despues: 30,
          diferencia: -30,
          diferencia_pct: -50,
          direccion: 'baja',
          preferencia: 'favor_de_referencia',
          confianza: null,
          motivos: ['participo'],
          limitaciones: [],
        },
      ],
    });
    const m = presentarEvaluacion(ev, comparacion(100, 120));
    expect(m.veredicto.tipo).toBe('sin_ganadora');
    expect(m.quienFavorece).toBeNull();
    expect(m.cobertura.parcial).toBe(false);
  });
});
