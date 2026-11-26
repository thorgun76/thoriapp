import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, renderHook, waitFor, act } from '@testing-library/react';

/**
 * Motor de mentira: imita el contrato de la IPC, no sus reglas.
 *
 * Reconoce `AMD_VULKAN_ICD=RADV`, `game-performance` y `mangohud` como ajustes (índices 0,
 * 1 y 2) para poder probar la orquestación: acumulación de peticiones, orden de respuestas,
 * conservación del último estado bueno. Las reglas de verdad se prueban en Rust.
 */
const ANALIZAR = vi.fn();
const ESTADOS = vi.fn();
const CAMBIOS = vi.fn();
const SERIALIZAR = vi.fn();

vi.mock('../lib/api', () => ({
  api: {
    analyzeLaunchCommand: (...a: unknown[]) => ANALIZAR(...a),
    launchCommandStates: (...a: unknown[]) => ESTADOS(...a),
    launchCommandChanges: (...a: unknown[]) => CAMBIOS(...a),
    serializeLaunchCommand: (...a: unknown[]) => SERIALIZAR(...a),
  },
}));

import { usarComando, activosDe, esAccionable } from './useComando';
import { estaEncendido as estaActivo } from '../lib/motorComando';

/** Los tres ajustes que el motor de mentira reconoce. */
const VALORES = ['AMD_VULKAN_ICD=RADV', 'game-performance', 'mangohud'];

/** Estado del ajuste `i` según si su valor aparece en el texto. */
function estadosDe(texto: string) {
  return VALORES.map((valor, indice) => ({
    indice,
    valor,
    kind: valor.includes('=') ? 'env' : 'arg',
    estado: texto.includes(valor) ? 'activo' : 'inactivo',
    ...(texto.includes(valor) ? { span: { inicio: 0, fin: valor.length } } : {}),
  }));
}

/** Análisis de mentira con la forma que espera TS. */
function analisisDe(texto: string) {
  return {
    original: texto,
    segmentos: [],
    avisos: [],
    editable: true,
    reconocidos: 0,
    conservados: 0,
    sintaxis_valida: true,
    motivo_sintaxis: null,
  };
}

/** Serializa de verdad: quita los valores ausentes y pone los presentes delante. */
function serializarDe(texto: string, cambios: { indice: number; activo: boolean }[]) {
  let piezas = texto.replace('%command%', '').trim().split(/\s+/).filter(Boolean);
  for (const c of cambios) {
    const valor = VALORES[c.indice];
    piezas = piezas.filter((p) => p !== valor);
    if (c.activo) piezas.unshift(valor);
  }
  // El orden canónico del motor: env, wrappers, args.
  piezas.sort((a, b) => VALORES.indexOf(a) - VALORES.indexOf(b));
  return piezas.length ? `${piezas.join(' ')} %command%` : '%command%';
}

const PARAMS = VALORES.map((value, i) => ({
  kind: i === 0 ? 'env' : 'wrapper',
  value,
  title: value,
  why: '',
  impact: 'high',
  requires: [],
  default_on: false,
  evidence: 'test',
  proton_only: false,
}));

beforeEach(() => {
  ANALIZAR.mockReset();
  ESTADOS.mockReset();
  CAMBIOS.mockReset();
  SERIALIZAR.mockReset();

  ANALIZAR.mockImplementation((texto: string) => Promise.resolve(analisisDe(texto)));
  ESTADOS.mockImplementation((a: { original: string }) =>
    Promise.resolve(estadosDe(a.original)),
  );
  // Firma real: (analisis, appid, activos, extras). El appid y los extras son los del
  // análisis, para que Rust construya el mismo catálogo; aquí no hacen falta.
  CAMBIOS.mockImplementation(
    (a: { original: string }, _appid: number, activos: number[]) =>
      Promise.resolve(
        VALORES.map((_valor, indice) => ({ indice, activo: activos.includes(indice) })).filter(
          ({ indice, activo }) => estadosDe(a.original)[indice].estado !== (activo ? 'activo' : 'inactivo'),
        ),
      ),
  );
  SERIALIZAR.mockImplementation(
    (
      a: { original: string },
      _appid: number,
      cambios: { indice: number; activo: boolean }[],
    ) =>
      Promise.resolve({ comando: serializarDe(a.original, cambios), aplicado: true, error: null }),
  );
});

afterEach(cleanup);

const IMPORTADO = 'AMD_VULKAN_ICD=RADV game-performance mangohud %command%';

/** Monta el hook ya cargado con un comando de partida. */
async function montar(comandoInicial = IMPORTADO) {
  const r = renderHook(() =>
    usarComando(730, PARAMS as never, comandoInicial, true),
  );
  await waitFor(() => expect(r.result.current.revision.analisis).not.toBeNull());
  return r;
}

describe('importar y seguir editando', () => {
  it('importar deja los ajustes reconocidos editables, sin modo manual', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });

    // Los tres ajustes del comando quedan activos y accionables.
    expect(activosDe(result.current.revision.estados)).toEqual([0, 1, 2]);
    for (const e of result.current.revision.estados) {
      expect(esAccionable(e)).toBe(true);
    }
    // Y el comando mostrado es el importado, byte a byte.
    expect(result.current.comando).toBe(IMPORTADO);
  });

  it('el original importado se conserva aunque se cambien ajustes', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });

    await act(async () => {
      result.current.fijarAjuste(2, false); // quitar mangohud
    });
    await waitFor(() => expect(result.current.comando).toBe('AMD_VULKAN_ICD=RADV game-performance %command%'));

    // El original sigue intacto: es contra lo que se mide qué cambió el usuario.
    expect(result.current.originalImportado).toBe(IMPORTADO);
  });

  it('desactivar y volver a activar deja el comando como estaba', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });

    await act(async () => {
      result.current.fijarAjuste(2, false);
    });
    await waitFor(() => expect(result.current.comando).not.toBe(IMPORTADO));

    await act(async () => {
      result.current.fijarAjuste(2, true);
    });
    await waitFor(() => expect(activosDe(result.current.revision.estados)).toContain(2));
    // Los tres vuelven a estar, sin duplicados.
    expect(result.current.comando.match(/mangohud/g)).toHaveLength(1);
  });

  it('restaurar devuelve exactamente el texto importado y sus estados', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });

    // Se estropea: se quitan dos ajustes.
    await act(async () => {
      result.current.fijarAjuste(0, false);
      result.current.fijarAjuste(1, false);
    });
    await waitFor(() => expect(result.current.comando).not.toBe(IMPORTADO));

    await act(async () => {
      await result.current.restaurarImportado();
    });

    expect(result.current.comando).toBe(IMPORTADO);
    expect(activosDe(result.current.revision.estados)).toEqual([0, 1, 2]);
    expect(result.current.revision.procedencia).toBe('original_restaurado');
  });
});

describe('cargar un perfil', () => {
  it('adopta el comando del perfil y lo dice en la procedencia', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });

    let confirmado: string | null = null;
    await act(async () => {
      confirmado = await result.current.cargarPerfil('DXVK_HUD=fps %command%');
    });

    expect(confirmado).toBe('DXVK_HUD=fps %command%');
    expect(result.current.comando).toBe('DXVK_HUD=fps %command%');
    expect(result.current.revision.procedencia).toBe('perfil');
  });

  it('NO pisa la instantánea original importada', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });
    await act(async () => {
      await result.current.cargarPerfil('DXVK_HUD=fps %command%');
    });

    // Cargar un perfil no es importar de Steam: «Restaurar el comando importado» tiene
    // que devolver lo que trajo Steam, no el perfil.
    expect(result.current.originalImportado).toBe(IMPORTADO);
  });

  it('si el motor falla no cambia nada y devuelve null', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });
    const antes = result.current.comando;

    // El análisis del perfil falla: es el primer paso de la carga.
    ANALIZAR.mockRejectedValueOnce(new Error('el motor no puede analizar esto'));
    let confirmado: string | null = 'sin tocar';
    await act(async () => {
      confirmado = await result.current.cargarPerfil('DXVK_HUD=fps %command%');
    });

    // Atomicidad: ni comando, ni procedencia, ni original se mueven.
    expect(confirmado).toBeNull();
    expect(result.current.comando).toBe(antes);
    expect(result.current.revision.procedencia).toBe('importado');
    expect(result.current.originalImportado).toBe(IMPORTADO);
  });

  it('un texto vacío no se carga ni toca el comando actual', async () => {
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });

    let confirmado: string | null = 'sin tocar';
    await act(async () => {
      confirmado = await result.current.cargarPerfil('   ');
    });

    expect(confirmado).toBeNull();
    expect(result.current.comando).toBe(IMPORTADO);
  });
});

describe('varias pulsaciones seguidas', () => {
  it('tres toggles rápidos acaban todos aplicados', async () => {
    // Sin acumulación, cada toggle se calculaba sobre el mismo comando viejo y el último
    // pisaba a los anteriores.
    const { result } = await montar('%command%');

    await act(async () => {
      result.current.fijarAjuste(0, true);
      result.current.fijarAjuste(1, true);
      result.current.fijarAjuste(2, true);
    });

    await waitFor(() => expect(activosDe(result.current.revision.estados)).toEqual([0, 1, 2]));
    expect(result.current.comando).toBe(IMPORTADO);
  });

  it('activar y desactivar repetidamente el mismo ajuste termina en el último estado', async () => {
    const { result } = await montar('%command%');

    for (const activo of [true, false, true, false]) {
      await act(async () => {
        result.current.fijarAjuste(2, activo);
      });
      await waitFor(() => expect(estaActivo(result.current.revision.estados[2])).toBe(activo));
    }
    expect(result.current.comando).not.toContain('mangohud');
  });

  it('con una petición en vuelo, el cambio siguiente se aplica después y no se pierde', async () => {
    const { result } = await montar('%command%');

    // La primera serialización se queda colgada hasta que la soltemos.
    let soltar: (v: unknown) => void = () => {};
    SERIALIZAR.mockImplementationOnce(
      (a: { original: string }, _appid: number, c: { indice: number; activo: boolean }[]) =>
        new Promise((res) => {
          soltar = () => res({ comando: serializarDe(a.original, c), aplicado: true, error: null });
        }),
    );

    await act(async () => {
      result.current.fijarAjuste(0, true);
    });
    await act(async () => {
      result.current.fijarAjuste(1, true);
    });

    await act(async () => {
      soltar(null);
    });

    // Las dos acaban dentro: no se ha perdido la segunda por llegar mientras la primera.
    await waitFor(() => expect(activosDe(result.current.revision.estados)).toEqual([0, 1]));
  });
});

describe('los fallos no destruyen el estado', () => {
  it('un error del motor conserva el último comando confirmado', async () => {
    const { result } = await montar(IMPORTADO);
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });
    const antes = result.current.comando;

    SERIALIZAR.mockResolvedValueOnce({
      comando: antes,
      aplicado: false,
      error: { tipo: 'texto_no_coincide', segmento: 0, esperado: 'a', recibido: 'b' },
    });

    await act(async () => {
      result.current.fijarAjuste(2, false);
    });

    await waitFor(() => expect(result.current.fallo).not.toBeNull());
    // El comando no cambió: se sigue viendo lo último bueno.
    expect(result.current.comando).toBe(antes);
    // Y la explicación es concreta, no un «error» genérico.
    expect(result.current.fallo?.detalle).toMatch(/vuelve a intentarlo|no encaja/i);
  });

  it('un fallo de análisis al importar no borra lo que había', async () => {
    const { result } = await montar(IMPORTADO);
    ANALIZAR.mockRejectedValueOnce(new Error('comando demasiado largo'));

    await act(async () => {
      await result.current.importar('x'.repeat(10));
    });

    expect(result.current.comando).toBe(IMPORTADO);
    expect(result.current.fallo?.mensaje).toBe('comando demasiado largo');
  });
});

describe('restablecer y copiar no se mezclan con una actualización en curso', () => {
  it('restaurar mientras hay una modificación pendiente termina en el original', async () => {
    // El usuario pulsa un toggle y, sin esperar, «Restaurar comando original». La petición
    // del toggle puede volver después: no puede dejar el comando a medias.
    const { result } = await montar();
    await act(async () => {
      await result.current.importar(IMPORTADO);
    });

    let soltar: (v: unknown) => void = () => {};
    SERIALIZAR.mockImplementationOnce(
      (a: { original: string }, _appid: number, c: { indice: number; activo: boolean }[]) =>
        new Promise((res) => {
          soltar = () => res({ comando: serializarDe(a.original, c), aplicado: true, error: null });
        }),
    );

    await act(async () => {
      result.current.fijarAjuste(2, false);
    });
    // Y ahora restaurar, sin soltar la anterior.
    await act(async () => {
      await result.current.restaurarImportado();
    });

    await act(async () => {
      soltar(null);
    });

    // Pase lo que pase con la petición vieja, el comando es el importado.
    await waitFor(() => expect(result.current.comando).toBe(IMPORTADO));
  });

  it('copiar durante una actualización devuelve el último confirmado, no el pedido', async () => {
    // Si `comando` cambiara al pedir el cambio, se copiaría algo que el motor podría
    // rechazar y el usuario pegaría en Steam un comando que no existe.
    const { result } = await montar('%command%');

    let soltar: (v: unknown) => void = () => {};
    SERIALIZAR.mockImplementationOnce(
      () => new Promise((res) => { soltar = res; }),
    );
    await act(async () => {
      result.current.fijarAjuste(2, true);
    });

    // Todavía sin confirmar: el comando sigue siendo el de antes.
    expect(result.current.comando).toBe('%command%');

    await act(async () => {
      soltar({ comando: 'mangohud %command%', aplicado: true, error: null });
    });
    await waitFor(() => expect(result.current.comando).toBe('mangohud %command%'));
  });

  it('importar otro comando durante una modificación pendiente gana el importado', async () => {
    const { result } = await montar('%command%');

    let soltar: (v: unknown) => void = () => {};
    SERIALIZAR.mockImplementationOnce(
      () => new Promise((res) => { soltar = res; }),
    );
    await act(async () => {
      result.current.fijarAjuste(2, true);
    });

    await act(async () => {
      await result.current.importar(IMPORTADO);
    });
    await act(async () => {
      soltar({ comando: 'mangohud %command%', aplicado: true, error: null });
    });

    await waitFor(() => expect(result.current.comando).toBe(IMPORTADO));
    expect(result.current.originalImportado).toBe(IMPORTADO);
  });
});

describe('cambiar de juego', () => {
  it('deja de aplicar respuestas del juego anterior', async () => {
    const r = renderHook(({ appid }) => usarComando(appid, PARAMS as never, IMPORTADO, true), {
      initialProps: { appid: 730 },
    });
    await waitFor(() => expect(r.result.current.revision.analisis).not.toBeNull());

    // Petición del juego 730 que se queda en vuelo.
    let soltar: (v: unknown) => void = () => {};
    SERIALIZAR.mockImplementationOnce(
      () => new Promise((res) => { soltar = res; }),
    );
    await act(async () => {
      r.result.current.fijarAjuste(2, false);
    });

    // Se cambia de juego antes de que vuelva.
    r.rerender({ appid: 570 });
    await waitFor(() => expect(r.result.current.revision.analisis).not.toBeNull());

    // Llega la respuesta vieja: no puede tocar el estado del juego nuevo.
    await act(async () => {
      soltar({ comando: '%command%', aplicado: true, error: null });
    });
    expect(r.result.current.comando).toBe(IMPORTADO);
  });
});
