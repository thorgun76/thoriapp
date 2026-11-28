import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, renderHook, waitFor } from '@testing-library/react';

/**
 * Pruebas de `useComparison` [C.3.c].
 *
 * Lo que se fija aquí son las garantías de **concurrencia**, que son las que hacen segura
 * la activación y las que ningún componente puede demostrar por sí solo:
 *
 *  - una respuesta de una selección anterior no se pinta (ni se puede guardar);
 *  - A→B→A no se cuela aunque los identificadores vuelvan a coincidir;
 *  - dos clics en el mismo tick envían **un** guardado;
 *  - un error de transporte no se confunde con «no se guardó»;
 *  - sin vista previa no hay comparación, y sin comparación no se pide nada.
 *
 * Todo con dobles del módulo `api`: esto demuestra el contrato del hook, **no** que Rust
 * evalúe bien. Esa evidencia está en las pruebas de Rust y en la validación real.
 */

const compareBenchmarksEvaluada = vi.fn();
const saveComparisonEvaluada = vi.fn();
const loadComparisons = vi.fn();

vi.mock('../lib/api', () => ({
  api: {
    compareBenchmarksEvaluada: (...a: unknown[]) => compareBenchmarksEvaluada(...a),
    saveComparisonEvaluada: (...a: unknown[]) => saveComparisonEvaluada(...a),
    loadComparisons: (...a: unknown[]) => loadComparisons(...a),
  },
}));

import { useComparison, claveDePar, claveDeSeleccion, puedeGuardar } from './useComparison';
import { COMPARISONS_CHANGED_EVENT } from './useSavedComparisons';
import type { ConfirmacionDeVistaPrevia, EvaluacionPersistida } from '../lib/api';

/** Evaluación mínima con la forma que produce Rust. */
function evaluacion(estado = 'mejora'): EvaluacionPersistida {
  return {
    metodologia_interpretacion: 'v1',
    evaluada_en: '2026-01-02 11:00:00',
    referencia: { id: 'a' },
    candidata: { id: 'b' },
    estado,
    confianza: 'provisional',
    condiciones: { compatibilidad_metodologica: 'establecida' },
    motivos: [],
    limitaciones: [],
    metricas: [],
  } as EvaluacionPersistida;
}

/** Confirmación como la que devuelve `compare_benchmarks_evaluada`. */
function confirmacion(baselineId: string, testId: string, estado = 'mejora'): ConfirmacionDeVistaPrevia {
  return {
    evaluacion: evaluacion(estado),
    comparacion: {
      baseline: { id: baselineId },
      test: { id: testId },
    },
  } as ConfirmacionDeVistaPrevia;
}

/** Promesa que se resuelve a mano, para controlar el orden de las respuestas. */
function diferida<T>() {
  let resolver!: (v: T) => void;
  let rechazar!: (e: unknown) => void;
  const promesa = new Promise<T>((res, rej) => {
    resolver = res;
    rechazar = rej;
  });
  return { promesa, resolver, rechazar };
}

beforeEach(() => {
  compareBenchmarksEvaluada.mockReset();
  saveComparisonEvaluada.mockReset();
  loadComparisons.mockReset();
  // La lista empieza **vacía**: si trajera el par, la comparación ya constaría como
  // guardada y el guardado ni se intentaría. Cada prueba que lo necesite la carga.
  loadComparisons.mockResolvedValue([]);
});

afterEach(cleanup);

describe('claves de identidad', () => {
  it('el par no colisiona con ids que contienen el separador', () => {
    // `("a|b", "c")` y `("a", "b|c")` tienen que ser pares distintos.
    expect(claveDePar('a|b', 'c')).not.toBe(claveDePar('a', 'b|c'));
  });

  it('cambiar la variable cambia la selección', () => {
    const conVariable = claveDeSeleccion('a', 'b', { codigo: 'mangohud' });
    const sinVariable = claveDeSeleccion('a', 'b', null);
    const otraVariable = claveDeSeleccion('a', 'b', { codigo: 'gamemode' });
    expect(conVariable).not.toBe(sinVariable);
    expect(conVariable).not.toBe(otraVariable);
  });
});

describe('useComparison: sin evaluación no hay comparación', () => {
  it('sin los dos ids no se pide nada y no hay comparación', () => {
    const { result } = renderHook(() => useComparison(undefined, undefined));
    expect(compareBenchmarksEvaluada).not.toHaveBeenCalled();
    expect(result.current.comparison).toBeNull();
    expect(result.current.evaluacion).toBeNull();
  });

  it('mientras la vista previa no llega, no hay comparación que pintar', () => {
    const d = diferida<ConfirmacionDeVistaPrevia>();
    compareBenchmarksEvaluada.mockReturnValue(d.promesa);

    const { result } = renderHook(() => useComparison('a', 'b'));
    expect(result.current.comparison).toBeNull();
    expect(result.current.confirmacion).toBeNull();
    // Y por tanto no se puede guardar nada.
    expect(puedeGuardar({ ...result.current, guardadoPerdido: null })).toBe(false);
  });

  it('la evaluación y la comparación llegan juntas', async () => {
    compareBenchmarksEvaluada.mockResolvedValue(confirmacion('a', 'b'));
    const { result } = renderHook(() => useComparison('a', 'b'));

    await waitFor(() => expect(result.current.comparison).not.toBeNull());
    expect(result.current.evaluacion).not.toBeNull();
    expect(result.current.confirmacion).not.toBeNull();
  });
});

describe('useComparison: respuestas fuera de orden', () => {
  it('una respuesta de la selección anterior se descarta', async () => {
    const primera = diferida<ConfirmacionDeVistaPrevia>();
    const segunda = diferida<ConfirmacionDeVistaPrevia>();
    compareBenchmarksEvaluada
      .mockReturnValueOnce(primera.promesa)
      .mockReturnValueOnce(segunda.promesa);

    const { result, rerender } = renderHook(({ a, b }) => useComparison(a, b), {
      initialProps: { a: 'a', b: 'b' },
    });
    rerender({ a: 'a', b: 'c' });

    // Llega primero la vieja, después la nueva: la vieja no puede ganar.
    primera.resolver(confirmacion('a', 'b'));
    await Promise.resolve();
    expect(result.current.comparison).toBeNull();

    segunda.resolver(confirmacion('a', 'c'));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());
    expect(result.current.comparison?.test.id).toBe('c');
  });

  it('A→B→A no acepta la respuesta de la primera A', async () => {
    const deA1 = diferida<ConfirmacionDeVistaPrevia>();
    const deB = diferida<ConfirmacionDeVistaPrevia>();
    const deA2 = diferida<ConfirmacionDeVistaPrevia>();
    compareBenchmarksEvaluada
      .mockReturnValueOnce(deA1.promesa)
      .mockReturnValueOnce(deB.promesa)
      .mockReturnValueOnce(deA2.promesa);

    const { result, rerender } = renderHook(({ a, b }) => useComparison(a, b), {
      initialProps: { a: 'a', b: 'b' },
    });
    rerender({ a: 'a', b: 'c' }); // B
    rerender({ a: 'a', b: 'b' }); // A otra vez

    // La respuesta de la primera A llega tarde: mismos ids, pero otro token.
    deA1.resolver(confirmacion('a', 'b', 'empeora'));
    await Promise.resolve();
    expect(result.current.comparison).toBeNull();

    // La de B tampoco vale.
    deB.resolver(confirmacion('a', 'c'));
    await Promise.resolve();
    expect(result.current.comparison).toBeNull();

    deA2.resolver(confirmacion('a', 'b', 'mejora'));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());
    expect(result.current.evaluacion?.estado).toBe('mejora');
  });

  it('cambiar la variable invalida la vista previa vigente', async () => {
    const sinVariable = diferida<ConfirmacionDeVistaPrevia>();
    const conVariable = diferida<ConfirmacionDeVistaPrevia>();
    compareBenchmarksEvaluada
      .mockReturnValueOnce(sinVariable.promesa)
      .mockReturnValueOnce(conVariable.promesa);

    const { result, rerender } = renderHook(
      ({ variable }) => useComparison('a', 'b', undefined, variable),
      { initialProps: { variable: null as { codigo: string } | null } }
    );
    rerender({ variable: { codigo: 'mangohud' } });

    sinVariable.resolver(confirmacion('a', 'b'));
    await Promise.resolve();
    expect(result.current.comparison).toBeNull();

    conVariable.resolver(confirmacion('a', 'b'));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());
  });
});

describe('useComparison: un guardado por gesto', () => {
  it('dos clics en el mismo tick envían un solo guardado', async () => {
    compareBenchmarksEvaluada.mockResolvedValue(confirmacion('a', 'b'));
    const guardado = diferida<unknown>();
    saveComparisonEvaluada.mockReturnValue(guardado.promesa);

    const { result } = renderHook(() => useComparison('a', 'b'));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    // Los dos clics ocurren antes de que React vuelva a renderizar: la protección tiene
    // que ser síncrona, no un `setState`.
    result.current.save();
    result.current.save();

    expect(saveComparisonEvaluada).toHaveBeenCalledTimes(1);
    guardado.resolver({});
    await waitFor(() => expect(result.current.saving).toBe(false));
  });

  it('la confirmación que se guarda es la de la vista previa aceptada', async () => {
    const conf = confirmacion('a', 'b');
    compareBenchmarksEvaluada.mockResolvedValue(conf);
    saveComparisonEvaluada.mockResolvedValue({});

    const { result } = renderHook(() => useComparison('a', 'b'));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    await result.current.save();
    expect(saveComparisonEvaluada).toHaveBeenCalledWith(conf, null);
  });
});

describe('useComparison: guardado perdido por transporte', () => {
  it('un error tipado afirma que no se guardó', async () => {
    const onNotify = vi.fn();
    compareBenchmarksEvaluada.mockResolvedValue(confirmacion('a', 'b'));
    saveComparisonEvaluada.mockRejectedValue({
      tipo: 'confirmacion_rechazada',
      motivo: 'entradas_cambiadas',
    });

    const { result } = renderHook(() => useComparison('a', 'b', onNotify));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    await result.current.save();
    expect(result.current.guardadoPerdido).toBeNull();
    expect(onNotify).toHaveBeenCalledWith(expect.stringContaining('Vuelve a comparar'));
  });

  it('si la relectura encuentra una guardada del par, NO se declara recuperado', async () => {
    const onNotify = vi.fn();
    compareBenchmarksEvaluada.mockResolvedValue(confirmacion('a', 'b'));
    saveComparisonEvaluada.mockRejectedValue(new Error('IPC caído'));
    // Primera lectura (al montar): nada guardado. Relectura tras el fallo: hay una
    // comparación de ese par… pero podría ser anterior a este intento.
    loadComparisons
      .mockResolvedValueOnce([])
      .mockResolvedValue([{ comparison: { baseline: { id: 'a' }, test: { id: 'b' } } }]);

    const { result } = renderHook(() => useComparison('a', 'b', onNotify));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    await result.current.save();
    await waitFor(() => expect(result.current.guardadoPerdido).toEqual({ tipo: 'par_existente' }));

    // El aviso nombra el registro existente y **no** afirma que sea este intento.
    const aviso = onNotify.mock.calls[onNotify.mock.calls.length - 1]?.[0] as string;
    expect(aviso).toContain('Ya hay una comparación guardada');
    expect(aviso).toContain('no se pudo confirmar');
    expect(aviso).not.toContain('recuperó');

    // Y no se ofrece repetir: el par ya tiene una guardada y no se sabe cuál es cuál.
    expect(
      puedeGuardar({ ...result.current, guardadoPerdido: result.current.guardadoPerdido! })
    ).toBe(false);
  });

  it('una comparación previa del mismo par no se presenta como el resultado del intento', async () => {
    const onNotify = vi.fn();
    compareBenchmarksEvaluada.mockResolvedValue(confirmacion('a', 'b'));
    saveComparisonEvaluada.mockRejectedValue(new Error('IPC caído'));
    // Ya estaba guardada **antes** del intento: la lista inicial ya la tiene.
    loadComparisons.mockResolvedValue([
      { comparison: { baseline: { id: 'a' }, test: { id: 'b' } } },
    ]);

    const { result } = renderHook(() => useComparison('a', 'b', onNotify));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    await result.current.save();
    // Ni «recuperado» ni «guardada»: el guardado ni siquiera debía ofrecerse.
    expect(result.current.guardadoPerdido).toBeNull();
  });

  it('si no se puede saber, se conserva la incertidumbre y no se ofrece guardar', async () => {
    const onNotify = vi.fn();
    compareBenchmarksEvaluada.mockResolvedValue(confirmacion('a', 'b'));
    saveComparisonEvaluada.mockRejectedValue(new Error('IPC caído'));
    loadComparisons
      .mockResolvedValueOnce([])
      .mockRejectedValue(new Error('la lista tampoco se puede leer'));

    const { result } = renderHook(() => useComparison('a', 'b', onNotify));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    await result.current.save();
    await waitFor(() => expect(result.current.guardadoPerdido).toEqual({ tipo: 'desconocido' }));

    // Repetir podría chocar con el duplicado y el usuario no sabría cuál es el estado real.
    expect(puedeGuardar({ ...result.current, guardadoPerdido: result.current.guardadoPerdido! })).toBe(
      false
    );
  });
});

describe('useComparison: concurrencia con orden de resolución controlado', () => {
  it('una relectura de la lista resuelta tarde no pisa a una más reciente', async () => {
    const onNotify = vi.fn();
    compareBenchmarksEvaluada.mockResolvedValue(confirmacion('a', 'b'));

    // Se controla **cada** relectura en orden: la de montaje y las dos que se disparan a
    // mano. `save()` hace su propia relectura, así que no se puede depender de cuántas
    // ocurren: se interceptan todas.
    const cola: Array<{ resolver: (v: unknown[]) => void }> = [];
    loadComparisons.mockImplementation(() => {
      const d = diferida<unknown[]>();
      cola.push({ resolver: d.resolver });
      return d.promesa;
    });

    const { result } = renderHook(() => useComparison('a', 'b', onNotify));
    await waitFor(() => expect(result.current.comparison).not.toBeNull());
    // Montaje: lista vacía.
    cola[0].resolver([]);
    await new Promise((r) => setTimeout(r, 10));

    // Dos relecturas disparadas por el evento del listado, sin resolver todavía.
    window.dispatchEvent(new Event(COMPARISONS_CHANGED_EVENT));
    await waitFor(() => expect(cola.length).toBe(2));
    window.dispatchEvent(new Event(COMPARISONS_CHANGED_EVENT));
    await waitFor(() => expect(cola.length).toBe(3));

    // Llega **primero** la más nueva (la tercera), que sí tiene el par.
    cola[2].resolver([{ comparison: { baseline: { id: 'a' }, test: { id: 'b' } } }]);
    await waitFor(() => expect(result.current.isSaved).toBe(true));

    // Y ahora la vieja (la segunda), que no lo tiene: no debe desmarcarlo.
    cola[1].resolver([]);
    await new Promise((r) => setTimeout(r, 20));
    expect(result.current.isSaved).toBe(true);
  });

  it('un fallo tardío de un guardado anterior no escribe su duda en la selección nueva', async () => {
    const onNotify = vi.fn();
    compareBenchmarksEvaluada.mockImplementation(async (a: string, b: string) =>
      confirmacion(a, b)
    );
    saveComparisonEvaluada.mockRejectedValue(new Error('IPC caído'));
    loadComparisons.mockResolvedValue([]);

    const { result, rerender } = renderHook(
      ({ a, b }: { a: string; b: string }) => useComparison(a, b, onNotify),
      { initialProps: { a: 'a', b: 'b' } }
    );
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    // Se lanza el guardado de (a,b) y se cambia de selección **antes** de que falle.
    const guardado = result.current.save();
    rerender({ a: 'c', b: 'd' });
    await waitFor(() =>
      expect(result.current.comparison?.baseline.id).toBe('c')
    );

    await guardado;

    // La duda era del intento sobre (a,b): no contamina (c,d).
    expect(result.current.guardadoPerdido).toBeNull();
    expect(puedeGuardar({ ...result.current, guardadoPerdido: result.current.guardadoPerdido })).toBe(
      true
    );
  });

  it('A→B→A durante un guardado: la respuesta de la primera A no se aplica', async () => {
    const onNotify = vi.fn();
    const guardadoA = diferida<void>();
    compareBenchmarksEvaluada.mockImplementation(async (a: string, b: string) =>
      confirmacion(a, b)
    );
    loadComparisons.mockResolvedValue([]);
    saveComparisonEvaluada.mockReturnValueOnce(guardadoA.promesa).mockResolvedValue(undefined);

    const { result, rerender } = renderHook(
      ({ a, b }: { a: string; b: string }) => useComparison(a, b, onNotify),
      { initialProps: { a: 'a', b: 'b' } }
    );
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    const primero = result.current.save();
    // B y luego otra vez A, con el guardado de la primera A todavía en vuelo.
    rerender({ a: 'c', b: 'd' });
    rerender({ a: 'a', b: 'b' });
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    guardadoA.resolver(undefined);
    await primero;

    // El guardado ocurrió (se refresca la lista), pero no se declara duda alguna sobre la
    // selección vigente ni se la marca como guardada por la respuesta vieja.
    expect(result.current.guardadoPerdido).toBeNull();
  });

  it('un guardado en curso no impide que la selección nueva pida su propia vista previa', async () => {
    const onNotify = vi.fn();
    const guardadoA = diferida<void>();
    const vistas: string[] = [];
    compareBenchmarksEvaluada.mockImplementation(async (a: string, b: string) => {
      vistas.push(`${a}/${b}`);
      return confirmacion(a, b);
    });
    loadComparisons.mockResolvedValue([]);
    saveComparisonEvaluada.mockReturnValueOnce(guardadoA.promesa);

    const { result, rerender } = renderHook(
      ({ a, b }: { a: string; b: string }) => useComparison(a, b, onNotify),
      { initialProps: { a: 'a', b: 'b' } }
    );
    await waitFor(() => expect(result.current.comparison).not.toBeNull());

    const primero = result.current.save();
    rerender({ a: 'c', b: 'd' });
    await waitFor(() => expect(vistas).toContain('c/d'));

    guardadoA.resolver(undefined);
    await primero;
    expect(vistas).toEqual(['a/b', 'c/d']);
  });
});

describe('puedeGuardar', () => {
  const base = {
    confirmacion: confirmacion('a', 'b'),
    isSaved: false,
    saving: false,
    guardadoPerdido: null,
  };

  it('con confirmación, sin guardar y sin duda: sí', () => {
    expect(puedeGuardar(base)).toBe(true);
  });

  it('sin confirmación: no', () => {
    expect(puedeGuardar({ ...base, confirmacion: null })).toBe(false);
  });

  it('ya guardada: no', () => {
    expect(puedeGuardar({ ...base, isSaved: true })).toBe(false);
  });

  it('guardando: no', () => {
    expect(puedeGuardar({ ...base, saving: true })).toBe(false);
  });

  it('con el resultado en duda: no', () => {
    expect(puedeGuardar({ ...base, guardadoPerdido: { tipo: 'desconocido' } })).toBe(false);
  });
});

/**
 * Regresión de C3-13: **selección estable no puede re-pedir la comparación**.
 *
 * El fallo venía de que `onNotify` viajaba en las dependencias del efecto y el componente
 * real lo recrea en cada render (`const notify = (m) => setToast(m)`). Cualquier
 * re-render (pintar el propio toast, abrir un desplegable, cualquier `setState`) cambiaba
 * la identidad del callback, el efecto se limpiaba y se volvía a lanzar: la vista previa se
 * invalidaba y se repetía el IPC indefinidamente.
 *
 * Los mocks no lo veían porque `renderHook` recibe `onNotify` una sola vez y no lo recrea.
 * Aquí se recrea en cada render, como hace `BenchmarksView`.
 */
describe('C3-13: selección estable con callback recreado', () => {
  beforeEach(() => {
    compareBenchmarksEvaluada.mockReset();
    loadComparisons.mockReset();
    loadComparisons.mockResolvedValue([]);
    compareBenchmarksEvaluada.mockImplementation(async () => confirmacion('a', 'b'));
  });

  it('renderizar de nuevo con la misma selección no repite el IPC', async () => {
    // `onNotify` nueva en cada render, como en el componente real.
    const { result, rerender } = renderHook(
      ({ antes, despues }: { antes: string; despues: string }) =>
        useComparison(antes, despues, (m: string) => void m),
      { initialProps: { antes: 'a', despues: 'b' } }
    );

    await waitFor(() => expect(result.current.evaluacion).not.toBeNull());
    const llamadasIniciales = compareBenchmarksEvaluada.mock.calls.length;

    // Diez re-renders con la MISMA selección: no debe salir ni una petición más.
    for (let i = 0; i < 10; i++) {
      rerender({ antes: 'a', despues: 'b' });
    }

    expect(compareBenchmarksEvaluada.mock.calls.length).toBe(llamadasIniciales);
    // Y la vista previa sigue disponible: es lo que permite guardar.
    expect(result.current.evaluacion).not.toBeNull();
    expect(result.current.comparison).not.toBeNull();
    expect(
      puedeGuardar({
        confirmacion: result.current.confirmacion,
        isSaved: result.current.isSaved,
        saving: result.current.saving,
        guardadoPerdido: result.current.guardadoPerdido,
      })
    ).toBe(true);
  });

  it('con el toast pintado (re-render continuo) la vista previa no se invalida', async () => {
    const { result, rerender } = renderHook(
      ({ tick }: { tick: number }) => {
        // `tick` fuerza un render nuevo, y el callback se recrea: es lo que hace el toast.
        void tick;
        return useComparison('a', 'b', () => undefined);
      },
      { initialProps: { tick: 0 } }
    );

    await waitFor(() => expect(result.current.evaluacion).not.toBeNull());

    for (let i = 1; i <= 5; i++) {
      rerender({ tick: i });
      // Entre render y render la evaluación tiene que seguir ahí.
      expect(result.current.evaluacion).not.toBeNull();
    }
    expect(compareBenchmarksEvaluada.mock.calls.length).toBe(1);
  });

  it('cambiar la candidata SÍ invalida y vuelve a pedir', async () => {
    const { result, rerender } = renderHook(
      ({ antes, despues }: { antes: string; despues: string }) =>
        useComparison(antes, despues, () => undefined),
      { initialProps: { antes: 'a', despues: 'b' } }
    );
    await waitFor(() => expect(result.current.evaluacion).not.toBeNull());
    const antesDe = compareBenchmarksEvaluada.mock.calls.length;

    rerender({ antes: 'a', despues: 'c' });
    await waitFor(() => expect(compareBenchmarksEvaluada.mock.calls.length).toBe(antesDe + 1));
    expect(compareBenchmarksEvaluada).toHaveBeenLastCalledWith('a', 'c', null);
  });

  it('cambiar la referencia SÍ invalida y vuelve a pedir', async () => {
    const { result, rerender } = renderHook(
      ({ antes, despues }: { antes: string; despues: string }) =>
        useComparison(antes, despues, () => undefined),
      { initialProps: { antes: 'a', despues: 'b' } }
    );
    await waitFor(() => expect(result.current.evaluacion).not.toBeNull());
    const antesDe = compareBenchmarksEvaluada.mock.calls.length;

    rerender({ antes: 'z', despues: 'b' });
    await waitFor(() => expect(compareBenchmarksEvaluada.mock.calls.length).toBe(antesDe + 1));
    expect(compareBenchmarksEvaluada).toHaveBeenLastCalledWith('z', 'b', null);
  });
});
