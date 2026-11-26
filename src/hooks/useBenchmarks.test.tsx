import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, renderHook, waitFor } from '@testing-library/react';
import type { Benchmark } from '../lib/types';

const analyzeMangohudLog = vi.fn();
const isLogAlreadySaved = vi.fn();
const saveBenchmark = vi.fn();
const loadBenchmarksForGame = vi.fn();
const deleteBenchmarksForGame = vi.fn();
const deleteAllBenchmarks = vi.fn();
const deleteBenchmark = vi.fn();

vi.mock('../lib/api', () => ({
  api: {
    analyzeMangohudLog: (...a: unknown[]) => analyzeMangohudLog(...a),
    isLogAlreadySaved: (...a: unknown[]) => isLogAlreadySaved(...a),
    saveBenchmark: (...a: unknown[]) => saveBenchmark(...a),
    loadBenchmarksForGame: (...a: unknown[]) => loadBenchmarksForGame(...a),
    loadBenchmarks: (...a: unknown[]) => loadBenchmarksForGame(...a),
    deleteBenchmarksForGame: (...a: unknown[]) => deleteBenchmarksForGame(...a),
    deleteAllBenchmarks: (...a: unknown[]) => deleteAllBenchmarks(...a),
    deleteBenchmark: (...a: unknown[]) => deleteBenchmark(...a),
  },
}));

import { useBenchmarks } from './useBenchmarks';
import { COMPARISONS_CHANGED_EVENT } from './useSavedComparisons';

const PALWORLD = 'mesa_glthread=true game-performance mangohud %command%';

function bench(): Benchmark {
  return {
    id: 'x',
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-23 15:10:13',
    duration_seconds: 30,
    avg_fps: 60,
    fps_1_percent_low: 50,
    fps_0_1_percent_low: 45,
    max_fps: 80,
    min_fps: 30,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 20,
    total_frames: 300,
    notes: null,
    launch_command: '',
    warmup_seconds: 0,
  };
}

beforeEach(() => {
  analyzeMangohudLog.mockReset().mockResolvedValue(bench());
  isLogAlreadySaved.mockReset().mockResolvedValue(false);
  saveBenchmark.mockReset().mockResolvedValue(bench());
  loadBenchmarksForGame.mockReset().mockResolvedValue([]);
});

afterEach(cleanup);

describe('useBenchmarks: comando registrado', () => {
  it('usa el comando de la instantánea cuando la hay', async () => {
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentLaunchCommand: PALWORLD })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    // La instantánea iba con otro comando (el usuario cambió de perfil después).
    await result.current.analyze('/tmp/log.csv', '', 0, {
      command: 'gamemoderun %command%',
      at: '2026-11-23T15:10:13Z',
    });

    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe('gamemoderun %command%');
  });

  it('sin instantánea usa el comando actual', async () => {
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentLaunchCommand: PALWORLD })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0);

    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe(PALWORLD);
  });

  /**
   * Caso reportado: la grabación se hizo con
   * `mesa_glthread=true game-performance mangohud %command%`, tanto en Steam como en
   * Thoriapp, y se guardó como `%command%`.
   */
  it('una instantánea vacía NO borra el comando que se está mostrando', async () => {
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentLaunchCommand: PALWORLD })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    // Instantánea tomada en un momento en que el comando estaba vacío (p. ej. el perfil
    // aún no había cargado). `??` deja pasar el `''`, y el guardado lo convierte en
    // «%command%» igual que habría hecho el bug original.
    await result.current.analyze('/tmp/log.csv', '', 0, { command: '', at: 'x' });

    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe(PALWORLD);
  });

  it('informa de dónde salió el comando, para poder auditarlo', async () => {
    const notify = vi.fn();
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentLaunchCommand: PALWORLD, onNotify: notify })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    // Con instantánea y vacía: el aviso tiene que decir que se usó el comando actual,
    // no la instantánea, porque es justo la duda que hay detrás del fallo.
    await result.current.analyze('/tmp/log.csv', '', 0, { command: '', at: 'x' });

    const msg = notify.mock.calls.map((c) => String(c[0])).join(' | ');
    expect(msg).toMatch(/comando tomado del actual, no de la grabación/i);
  });

  /**
   * Caso reportado: «voy a biblioteca → Palworld → traer comando de steam», luego a
   * Pruebas sin el juego abierto, graba, y la prueba sale «origen desconocido».
   *
   * Con la ficha abierta se usa su comando. Pero llegar a Pruebas desde la barra lateral
   * no abre ningún juego: ahí `currentAppid`/`currentLaunchCommand` no existen.
   */
  it('con un juego abierto usa su comando aunque la instantánea venga vacía', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 1623730 });
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentLaunchCommand: PALWORLD })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe(PALWORLD);
  });

  it('grabando sin juego abierto, recupera el comando del juego de la grabación', async () => {
    // La grabación dice de qué juego es (MangoHud la nombra y el CSV trae el appid).
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 1623730 });

    const comandoParaJuego = vi.fn().mockResolvedValue(PALWORLD);
    const { result } = renderHook(() =>
      useBenchmarks({ comandoParaJuego }) // sin currentAppid: no hay ficha abierta
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    // Se pregunta por el juego real, no por el que (no) está abierto.
    expect(comandoParaJuego).toHaveBeenCalledWith({ appid: 1623730, gameName: 'Palworld' });
    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe(PALWORLD);
    expect(saveBenchmark.mock.calls[0][0].appid).toBe(1623730);
  });

  it('sin juego abierto y con appid 0, pregunta por el nombre del juego', async () => {
    // Tu caso: el CSV no trae AppID (`appid: 0` viene siempre de `analyze_log`), así que
    // la identidad es el nombre. Antes se comparaba `0 !== currentAppid` y, sin ficha
    // abierta, la recuperación no se ejecutaba nunca → «Comando no registrado».
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 0, game_name: 'Palworld' });
    const comandoParaJuego = vi.fn().mockResolvedValue(PALWORLD);

    const { result } = renderHook(() => useBenchmarks({ comandoParaJuego }));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(comandoParaJuego).toHaveBeenCalledWith({ appid: 0, gameName: 'Palworld' });
    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe(PALWORLD);
  });

  it('con la ficha de otro juego abierta, también recupera por nombre', async () => {
    // Abierto Elden Ring, la grabación es de Palworld: no vale el comando del editor.
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 0, game_name: 'Palworld' });
    const comandoParaJuego = vi.fn().mockResolvedValue(PALWORLD);

    const { result } = renderHook(() =>
      useBenchmarks({
        currentAppid: 1245620,
        currentGameName: 'Elden Ring',
        currentLaunchCommand: 'gamemoderun %command%',
        comandoParaJuego,
      })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(comandoParaJuego).toHaveBeenCalled();
    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe(PALWORLD);
  });

  it('si el nombre coincide con el juego abierto, no se pregunta (ya se sabe)', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 0, game_name: 'Palworld' });
    const comandoParaJuego = vi.fn().mockResolvedValue('NO DEBERIA USARSE');

    const { result } = renderHook(() =>
      useBenchmarks({
        currentAppid: 1623730,
        currentGameName: 'Palworld',
        currentLaunchCommand: PALWORLD,
        comandoParaJuego,
      })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(comandoParaJuego).not.toHaveBeenCalled();
    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe(PALWORLD);
  });

  it('si no hay comando recuperable, no se inventa nada', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 1623730 });
    const comandoParaJuego = vi.fn().mockResolvedValue(null);

    const { result } = renderHook(() => useBenchmarks({ comandoParaJuego }));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe('');
    expect(saveBenchmark.mock.calls[0][0].origenComando).toBe('desconocido');
  });

  it('un fallo al recuperar no rompe el guardado', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 1623730 });
    const comandoParaJuego = vi.fn().mockRejectedValue(new Error('boom'));

    const { result } = renderHook(() => useBenchmarks({ comandoParaJuego }));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(saveBenchmark).toHaveBeenCalled();
    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe('');
  });

  it('la instantánea de la grabación sigue teniendo prioridad sobre todo', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 1623730 });
    const comandoParaJuego = vi.fn().mockResolvedValue('NO DEBERIA USARSE');

    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentLaunchCommand: PALWORLD, comandoParaJuego })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, {
      command: 'gamemoderun %command%',
      at: 'x',
    });

    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe('gamemoderun %command%');
    expect(comandoParaJuego).not.toHaveBeenCalled();
  });

  it('sin nada configurado se guarda vacío, no %command% inventado', async () => {
    const { result } = renderHook(() => useBenchmarks({ currentAppid: 1623730 }));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, { command: '', at: 'x' });

    // «No se sabe» se guarda como «no registrado», nunca como «sin ajustes».
    expect(saveBenchmark.mock.calls[0][0].launchCommand).toBe('');
  });
});

describe('useBenchmarks: lo que dice Steam', () => {
  it('guarda las opciones de Steam de la grabación', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 0, game_name: 'Palworld' });
    const steamParaJuego = vi.fn().mockResolvedValue(PALWORLD);

    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentLaunchCommand: '%command%', steamParaJuego })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, {
      command: '',
      at: 'x',
    });

    // El editor no sabía nada (`''`), pero Steam sí: es el dato que da sentido a la prueba.
    expect(steamParaJuego).toHaveBeenCalledWith({ appid: 1623730, gameName: 'Palworld' });
    expect(saveBenchmark.mock.calls[0][0].steamCommand).toBe(PALWORLD);
  });

  it('Steam sin opciones se guarda como cadena vacía, no como «no se sabe»', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 1623730 });
    const steamParaJuego = vi.fn().mockResolvedValue('');

    const { result } = renderHook(() => useBenchmarks({ steamParaJuego }));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(saveBenchmark.mock.calls[0][0].steamCommand).toBe('');
  });

  it('si Steam no se puede leer, no se inventa nada', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 1623730 });
    const steamParaJuego = vi.fn().mockRejectedValue(new Error('sin localconfig'));

    const { result } = renderHook(() => useBenchmarks({ steamParaJuego }));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    expect(saveBenchmark).toHaveBeenCalled();
    expect(saveBenchmark.mock.calls[0][0].steamCommand).toBeNull();
  });

  it('con AppID 0 se pregunta igual: el backend resuelve el juego por nombre', async () => {
    analyzeMangohudLog.mockResolvedValue({ ...bench(), appid: 0, game_name: 'Palworld' });
    const steamParaJuego = vi.fn().mockResolvedValue(PALWORLD);

    const { result } = renderHook(() => useBenchmarks({ steamParaJuego }));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0, null);

    // El CSV de MangoHud nunca trae AppID: si no se preguntara por nombre, una prueba
    // analizada sin la ficha abierta se quedaría siempre sin saber qué se ejecutó.
    expect(steamParaJuego).toHaveBeenCalledWith({ appid: 0, gameName: 'Palworld' });
    expect(saveBenchmark.mock.calls[0][0].steamCommand).toBe(PALWORLD);
  });
});

// ── Borrado ──────────────────────────────────────────────────────────────────
//
// La ventana «Pruebas de este juego» borra solo el historial del juego abierto. Usar
// `deleteAllBenchmarks` desde ahí vaciaría la lista entera y se llevaría por delante las
// pruebas de los otros juegos.

describe('useBenchmarks: carga filtrada al juego', () => {
  it('pasa el nombre del juego, no solo el appid', async () => {
    loadBenchmarksForGame.mockReset().mockResolvedValue([]);
    renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentGameName: 'Palworld' })
    );

    // Las grabaciones antiguas tienen appid 0: sin el nombre no se encuentran y la
    // ventana «Pruebas del juego» se quedaba vacía.
    await waitFor(() =>
      expect(loadBenchmarksForGame).toHaveBeenCalledWith({
        appid: 1623730,
        gameName: 'Palworld',
      })
    );
  });
});

describe('useBenchmarks: borrado filtrado al juego', () => {
  beforeEach(() => {
    loadBenchmarksForGame.mockReset().mockResolvedValue([]);
    deleteBenchmarksForGame.mockReset().mockResolvedValue(0);
    deleteAllBenchmarks.mockReset().mockResolvedValue(undefined);
    deleteBenchmark.mockReset().mockResolvedValue(undefined);
  });

  it('borra solo el historial del juego abierto, no todos', async () => {
    deleteBenchmarksForGame.mockResolvedValue(3);
    const onNotify = vi.fn();
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentGameName: 'Palworld', onNotify })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.removeAllForGame();

    expect(deleteBenchmarksForGame).toHaveBeenCalledWith({
      appid: 1623730,
      gameName: 'Palworld',
    });
    expect(deleteAllBenchmarks).not.toHaveBeenCalled();
    expect(onNotify).toHaveBeenCalledWith(expect.stringContaining('3 pruebas de este juego'));
  });

  it('dice claramente que no había nada que borrar', async () => {
    deleteBenchmarksForGame.mockResolvedValue(0);
    const onNotify = vi.fn();
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentGameName: 'Palworld', onNotify })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.removeAllForGame();

    // Antes se anunciaba «Se eliminaron 0 pruebas», que parece un error.
    expect(onNotify).toHaveBeenCalledWith('No había pruebas que borrar');
  });

  it('sin juego no llama al backend', async () => {
    const { result } = renderHook(() => useBenchmarks({}));
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.removeAllForGame();

    expect(deleteBenchmarksForGame).not.toHaveBeenCalled();
  });

  it('recarga el historial después de borrar', async () => {
    loadBenchmarksForGame.mockResolvedValue([bench()]);
    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentGameName: 'Palworld' })
    );
    await waitFor(() => expect(result.current.benchmarks).toHaveLength(1));

    await result.current.removeAllForGame();

    // Una llamada al montar y otra tras borrar.
    await waitFor(() => expect(loadBenchmarksForGame).toHaveBeenCalledTimes(2));
  });

  // ── Subfase 5.1: distinguir «no hay pruebas» de «no se pudo leer» ──

  it('s51 expone el error de lectura en vez de mostrar una lista vacía', async () => {
    loadBenchmarksForGame.mockReset().mockRejectedValue(
      'El historial de pruebas está dañado. No se ha modificado nada en disco.'
    );

    const { result } = renderHook(() => useBenchmarks({ currentAppid: 1623730 }));

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toMatch(/dañado/);
  });

  it('s51 conserva las pruebas ya cargadas si una recarga falla', async () => {
    const una = bench();
    loadBenchmarksForGame.mockReset().mockResolvedValueOnce([una]);

    const { result } = renderHook(() => useBenchmarks({ currentAppid: 1623730 }));
    await waitFor(() => expect(result.current.benchmarks).toHaveLength(1));

    // La segunda carga falla: los datos anteriores NO se borran de la pantalla, porque
    // pintar cero pruebas afirmaría que no hay ninguna.
    loadBenchmarksForGame.mockRejectedValueOnce('no se pudo leer');
    await result.current.load();

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.benchmarks).toHaveLength(1);
  });

  it('s51 sin error, error vuelve a null tras una carga correcta', async () => {
    loadBenchmarksForGame.mockReset().mockRejectedValueOnce('fallo').mockResolvedValueOnce([]);

    const { result } = renderHook(() => useBenchmarks({ currentAppid: 1623730 }));
    await waitFor(() => expect(result.current.error).not.toBeNull());

    await result.current.load();
    await waitFor(() => expect(result.current.error).toBeNull());
  });

  /**
   * Subfase 5.2: el contexto de captura viaja con el guardado.
   */
  it('s52 manda el contexto del juego al guardar', async () => {
    const contextoParaJuego = vi.fn().mockResolvedValue({
      appidReal: 1623730,
      comandoConfirmado: PALWORLD,
      perfilId: 'p1',
      perfilNombre: 'Rendimiento',
      modoProton: false,
      correspondeAlJuego: true,
    });

    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, currentGameName: 'Palworld', contextoParaJuego })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    await result.current.analyze('/tmp/log.csv', '', 0);

    // Se pide para el juego de la GRABACIÓN, no para «lo que haya abierto».
    expect(contextoParaJuego).toHaveBeenCalledWith({ appid: 1623730, gameName: 'Palworld' });
    expect(saveBenchmark.mock.calls[0][0].contexto).toMatchObject({
      perfilId: 'p1',
      correspondeAlJuego: true,
    });
  });

  it('s52 si el contexto falla, la prueba se guarda igual sin él', async () => {
    const contextoParaJuego = vi.fn().mockRejectedValue('boom');

    const { result } = renderHook(() =>
      useBenchmarks({ currentAppid: 1623730, contextoParaJuego })
    );
    await waitFor(() => expect(result.current.benchmarks).toEqual([]));

    const ok = await result.current.analyze('/tmp/log.csv', '', 0);

    expect(ok).toBe(true);
    expect(saveBenchmark).toHaveBeenCalledTimes(1);
    // Sin contexto: la instantánea se guardará con la configuración en desconocido, no con
    // datos de otro sitio.
    expect(saveBenchmark.mock.calls[0][0].contexto ?? null).toBeNull();
  });

  // Complemento final de 5.3: el backend purga las comparaciones guardadas que dependían de
  // las pruebas borradas. La interfaz tiene que enterarse, o «Mis pruebas» enseña informes
  // que ya no están en disco.
  it('avisa a las comparaciones guardadas al borrar el historial de un juego', async () => {
    deleteBenchmarksForGame.mockResolvedValue(2);
    const oyente = vi.fn();
    window.addEventListener(COMPARISONS_CHANGED_EVENT, oyente);
    try {
      const { result } = renderHook(() =>
        useBenchmarks({ currentAppid: 4623570, currentGameName: 'Deskrawl' })
      );
      await waitFor(() => expect(result.current.benchmarks).toEqual([]));
      await result.current.removeAllForGame();
      expect(oyente).toHaveBeenCalled();
    } finally {
      window.removeEventListener(COMPARISONS_CHANGED_EVENT, oyente);
    }
  });

  it('si no había nada que borrar, no molesta a las comparaciones', async () => {
    // Sin pruebas borradas no puede haber comparaciones huérfanas: recargar la lista sería
    // ruido, y un aviso de «algo ha cambiado» cuando no cambió nada es desinformar.
    deleteBenchmarksForGame.mockResolvedValue(0);
    const oyente = vi.fn();
    window.addEventListener(COMPARISONS_CHANGED_EVENT, oyente);
    try {
      const { result } = renderHook(() =>
        useBenchmarks({ currentAppid: 4623570, currentGameName: 'Deskrawl' })
      );
      await waitFor(() => expect(result.current.benchmarks).toEqual([]));
      await result.current.removeAllForGame();
      expect(oyente).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener(COMPARISONS_CHANGED_EVENT, oyente);
    }
  });
});
