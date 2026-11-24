import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, renderHook, waitFor } from '@testing-library/react';

const steamLaunchOptions = vi.fn();

vi.mock('../lib/api', () => ({
  api: {
    steamLaunchOptions: (...args: unknown[]) => steamLaunchOptions(...args),
  },
}));

import { usarOpcionesSteam } from './useSteamLaunch';

beforeEach(() => {
  steamLaunchOptions.mockReset();
});

afterEach(cleanup);

const PALWORLD = 'mesa_glthread=true game-performance mangohud %command%';

async function correr(appid: number | undefined, comando: string) {
  const r = renderHook(() => usarOpcionesSteam(appid, comando));
  await waitFor(() => expect(r.result.current.estado).not.toBe('cargando'));
  return r.result.current;
}

describe('usarOpcionesSteam', () => {
  it('detecta que Steam tiene otro comando', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '123' });
    const r = await correr(1623730, '%command%');

    expect(r.estado).toBe('ok');
    expect(r.discrepancia?.difiere).toBe(true);
    // El texto de Steam llega intacto: es lo que luego se importa.
    expect(r.discrepancia?.steam.opciones).toBe(PALWORLD);
  });

  it('no avisa si el comando coincide', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '123' });
    const r = await correr(1623730, PALWORLD);
    expect(r.discrepancia?.difiere).toBe(false);
  });

  it('ignora espacios en los extremos al comparar', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: `  ${PALWORLD} `, cuenta: '123' });
    const r = await correr(1623730, PALWORLD);
    expect(r.discrepancia?.difiere).toBe(false);
  });

  it('sin poder leer Steam no dice nada: ni discrepancia ni igualdad', async () => {
    // `null` = no se pudo leer. Es distinto de Steam tenerlas vacías.
    steamLaunchOptions.mockResolvedValue(null);
    const r = await correr(1623730, '%command%');
    expect(r.estado).toBe('sin_datos');
    expect(r.discrepancia).toBeNull();
  });

  it('un error de la lectura tampoco inventa un comando', async () => {
    steamLaunchOptions.mockRejectedValue(new Error('boom'));
    const r = await correr(1623730, '%command%');
    expect(r.estado).toBe('error');
    expect(r.discrepancia).toBeNull();
  });

  it('sin AppID no se consulta Steam', async () => {
    const r = await correr(undefined, '%command%');
    expect(r.estado).toBe('sin_datos');
    expect(steamLaunchOptions).not.toHaveBeenCalled();
  });

  it('Steam con las opciones vacías sí cuenta como dato', async () => {
    steamLaunchOptions.mockResolvedValue({ opciones: '', cuenta: '123' });
    const r = await correr(1623730, '%command%');
    expect(r.estado).toBe('ok');
    // Steam no añade nada y Thoriapp tampoco: no hay nada que importar.
    expect(r.discrepancia?.difiere).toBe(true);
    expect(r.discrepancia?.steam.opciones).toBe('');
  });

  it('al cambiar de juego desaparece el aviso del anterior', async () => {
    // Regresión: al abrir otro juego se seguía viendo un instante el «Steam tiene otro
    // comando» del juego anterior, que hablaba de un comando que no era el suyo.
    steamLaunchOptions.mockResolvedValue({ opciones: PALWORLD, cuenta: '1' });

    const { rerender, result } = renderHook(
      ({ appid }: { appid: number }) => usarOpcionesSteam(appid, '%command%'),
      { initialProps: { appid: 1623730 } }
    );
    await waitFor(() => expect(result.current.discrepancia?.difiere).toBe(true));

    // La lectura del juego nuevo queda pendiente: no debe quedar nada visible.
    steamLaunchOptions.mockImplementation(() => new Promise(() => {}));
    rerender({ appid: 1245620 });

    expect(result.current.estado).toBe('cargando');
    expect(result.current.discrepancia).toBeNull();
  });

  it('una respuesta tardía del juego anterior no pisa la del actual', async () => {
    // El caso real: el usuario cambia de juego mientras la primera lectura (un
    // `localconfig.vdf` de varios MB) sigue en vuelo. Si la vieja llega después, no debe
    // sobrescribir la del juego que se está viendo.
    const pendientes: ((v: unknown) => void)[] = [];
    steamLaunchOptions.mockImplementation(
      () => new Promise((res) => { pendientes.push(res); })
    );

    const { rerender, result } = renderHook(
      ({ appid }: { appid: number }) => usarOpcionesSteam(appid, '%command%'),
      { initialProps: { appid: 1 } }
    );

    rerender({ appid: 2 });
    await waitFor(() => expect(pendientes.length).toBe(2));

    // Responde primero la del juego actual (2) y luego la vieja (1).
    pendientes[1]({ opciones: PALWORLD, cuenta: '9' });
    await waitFor(() => expect(result.current.estado).toBe('ok'));

    pendientes[0]({ opciones: 'del juego 1', cuenta: '1' });

    await waitFor(() => expect(result.current.estado).toBe('ok'));
    expect(result.current.discrepancia?.steam.opciones).toBe(PALWORLD);
  });
});
