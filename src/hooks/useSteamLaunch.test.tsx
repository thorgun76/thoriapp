import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, renderHook, waitFor } from '@testing-library/react';

const steamLaunchOptions = vi.fn();

vi.mock('../lib/api', () => ({
  api: {
    steamLaunchOptions: (...args: unknown[]) => steamLaunchOptions(...args),
  },
}));

import { usarOpcionesSteam, type TresComandos } from './useSteamLaunch';

beforeEach(() => {
  steamLaunchOptions.mockReset();
});

afterEach(cleanup);

const PALWORLD = 'mesa_glthread=true game-performance mangohud %command%';

/** Los comandos que se pasan al hook: lo importado y lo actual. */
function comandos(actual: string, importado: string | null = null): TresComandos {
  return { importado, actual };
}

async function correr(appid: number | undefined, c: TresComandos) {
  const r = renderHook(() => usarOpcionesSteam(appid, c));
  await waitFor(() => expect(r.result.current.estado).not.toBe('cargando'));
  return r.result.current;
}

describe('usarOpcionesSteam: lectura', () => {
  it('detecta que Steam tiene otro comando', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '123' });
    const r = await correr(1623730, comandos('%command%'));

    expect(r.estado).toBe('ok');
    expect(r.comparacion.difiere).toBe(true);
    // El texto de Steam llega intacto: es lo que luego se importa.
    expect(r.opciones?.opciones).toBe(PALWORLD);
  });

  it('no avisa si el comando coincide', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '123' });
    const r = await correr(1623730, comandos(PALWORLD));
    expect(r.comparacion.difiere).toBe(false);
  });

  it('ignora espacios en los extremos al comparar', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: `  ${PALWORLD} `, cuenta: '123' });
    const r = await correr(1623730, comandos(PALWORLD));
    expect(r.comparacion.difiere).toBe(false);
  });

  it('sin poder leer Steam no dice nada: ni discrepancia ni igualdad', async () => {
    // `null` = no se pudo leer. Es distinto de Steam tenerlas vacías.
    steamLaunchOptions.mockResolvedValue(null);
    const r = await correr(1623730, comandos('%command%'));
    expect(r.estado).toBe('sin_datos');
    expect(r.opciones).toBeNull();
    expect(r.comparacion.difiere).toBeNull();
  });

  it('un error de la lectura tampoco inventa un comando', async () => {
    steamLaunchOptions.mockRejectedValue(new Error('boom'));
    const r = await correr(1623730, comandos('%command%'));
    expect(r.estado).toBe('error');
    expect(r.comparacion.difiere).toBeNull();
  });

  it('sin AppID no se consulta Steam', async () => {
    const r = await correr(undefined, comandos('%command%'));
    expect(r.estado).toBe('sin_datos');
    expect(steamLaunchOptions).not.toHaveBeenCalled();
  });

  it('Steam con las opciones vacías sí cuenta como dato', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: '', cuenta: '123' });
    const r = await correr(1623730, comandos('%command%'));
    expect(r.estado).toBe('ok');
    // Steam no añade nada y Thoriapp tampoco: no hay nada que importar.
    expect(r.comparacion.difiere).toBe(true);
    expect(r.opciones?.opciones).toBe('');
  });

  it('al cambiar de juego desaparece el aviso del anterior', async () => {
    // Regresión: al abrir otro juego se seguía viendo un instante el «Steam tiene otro
    // comando» del juego anterior, que hablaba de un comando que no era el suyo.
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '1' });

    const { rerender, result } = renderHook(
      ({ appid }: { appid: number }) => usarOpcionesSteam(appid, comandos('%command%')),
      { initialProps: { appid: 1623730 } },
    );
    await waitFor(() => expect(result.current.comparacion.difiere).toBe(true));

    // La lectura del juego nuevo queda pendiente: no debe quedar nada visible.
    steamLaunchOptions.mockImplementation(() => new Promise(() => {}));
    rerender({ appid: 1245620 });

    expect(result.current.estado).toBe('cargando');
    expect(result.current.comparacion.difiere).toBeNull();
  });

  it('una respuesta tardía del juego anterior no pisa la del actual', async () => {
    // El caso real: el usuario cambia de juego mientras la primera lectura (un
    // `localconfig.vdf` de varios MB) sigue en vuelo. Si la vieja llega después, no debe
    // sobrescribir la del juego que se está viendo.
    const pendientes: ((v: unknown) => void)[] = [];
    steamLaunchOptions.mockImplementation(
      () => new Promise((res) => { pendientes.push(res); }),
    );

    const { rerender, result } = renderHook(
      ({ appid }: { appid: number }) => usarOpcionesSteam(appid, comandos('%command%')),
      { initialProps: { appid: 1 } },
    );

    rerender({ appid: 2 });
    await waitFor(() => expect(pendientes.length).toBe(2));

    // Responde primero la del juego actual (2) y luego la vieja (1).
    pendientes[1]({ opciones: PALWORLD, cuenta: '9' });
    await waitFor(() => expect(result.current.estado).toBe('ok'));

    pendientes[0]({ opciones: 'otro-juego %command%', cuenta: '9' });
    // La vieja no puede cambiar nada.
    expect(result.current.opciones?.opciones).toBe(PALWORLD);
  });
});

describe('usarOpcionesSteam: comparación y lectura están separadas', () => {
  it('tocar un toggle NO vuelve a leer Steam', async () => {
    // Este es el motivo de separar lectura y comparación. Antes el efecto dependía de
    // `comandoActual`, así que cada pulsación leía `localconfig.vdf` otra vez y el aviso de
    // desincronización parpadeaba.
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '1' });

    const { rerender, result } = renderHook(
      ({ actual }: { actual: string }) =>
        usarOpcionesSteam(1623730, comandos(actual, PALWORLD)),
      { initialProps: { actual: PALWORLD } },
    );
    await waitFor(() => expect(result.current.estado).toBe('ok'));
    expect(steamLaunchOptions).toHaveBeenCalledTimes(1);

    // El usuario cambia ajustes cinco veces.
    for (const actual of [
      'mangohud %command%',
      'gamemoderun mangohud %command%',
      'DXVK_HUD=fps gamemoderun mangohud %command%',
      'DXVK_HUD=fps %command%',
      '%command%',
    ]) {
      rerender({ actual });
    }

    // Una sola lectura: la del `appid`. Nada de releer por cada toggle.
    expect(steamLaunchOptions).toHaveBeenCalledTimes(1);
    // Y la lectura sigue disponible para comparar.
    expect(result.current.opciones?.opciones).toBe(PALWORLD);
  });

  it('distingue «Steam cambió» de «Thoriapp cambió»', async () => {
    // Son dos preguntas distintas: si el usuario toca ajustes, el comando actual deja de
    // coincidir con lo importado, pero Steam sigue teniendo lo mismo.
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '1' });

    const { rerender, result } = renderHook(
      ({ actual }: { actual: string }) =>
        usarOpcionesSteam(1623730, comandos(actual, PALWORLD)),
      { initialProps: { actual: PALWORLD } },
    );
    await waitFor(() => expect(result.current.estado).toBe('ok'));

    // Sin tocar nada: todo coincide.
    expect(result.current.comparacion.difiere).toBe(false);
    expect(result.current.comparacion.difiereDeLoImportado).toBe(false);

    // El usuario activa un ajuste: deja de coincidir con lo importado, pero NO con Steam.
    rerender({ actual: 'mangohud %command%' });
    expect(result.current.comparacion.difiereDeLoImportado).toBe(true);
    expect(result.current.comparacion.difiere).toBe(true);
    // Lo importante: Steam no se ha releído, así que la lectura sigue siendo la original.
    expect(steamLaunchOptions).toHaveBeenCalledTimes(1);
  });

  it('sin importación previa no hay nada que comparar con lo importado', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '1' });
    const r = await correr(1623730, comandos('%command%', null));
    expect(r.comparacion.difiereDeLoImportado).toBe(false);
  });

  it('cambiar de juego sí vuelve a leer Steam', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '1' });

    const { rerender, result } = renderHook(
      ({ appid }: { appid: number }) => usarOpcionesSteam(appid, comandos('%command%')),
      { initialProps: { appid: 1 } },
    );
    await waitFor(() => expect(result.current.estado).toBe('ok'));
    expect(steamLaunchOptions).toHaveBeenCalledTimes(1);

    rerender({ appid: 2 });
    await waitFor(() => expect(steamLaunchOptions).toHaveBeenCalledTimes(2));
  });
});
