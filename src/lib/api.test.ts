import { describe, expect, it, vi } from 'vitest';
import { api, type SaveProfilePayload } from './api';

/**
 * Regresión del bug «invalid type: null, expected a boolean».
 *
 * `enabled` en App.tsx nacía como array vacío y `toggleRule` escribía por índice,
 * dejando huecos. `JSON.stringify([,true])` produce `[null,true]`, y Rust
 * rechazaba el perfil. Estas comprobaciones no arrancan Tauri: solo miran lo que
 * se serializa para el backend.
 */
describe('payload de save_profile', () => {
  const payload = (enabledParams: boolean[]): SaveProfilePayload => ({
    name: 'Perfil',
    appid: 440,
    gameName: 'Team Fortress 2',
    enabledParams,
    selectedCommunityRecs: [],
    selectedEngineRecs: [],
    forceProton: false,
    expertMode: false,
    manualCommand: null,
    useManualCommand: false,
    notes: null,
  });

  it('un array disperso se serializa con null (el bug original)', () => {
    // Demuestra POR QUÉ el bug rompía: los huecos pasan a null.
    const sparse = [undefined, true];
    expect(JSON.parse(JSON.stringify({ v: sparse })).v).toEqual([null, true]);
  });

  it('un array dimensionado con Array.from no deja huecos', () => {
    const sparse = [undefined, true];
    const sane = Array.from({ length: 2 }, (_, i) => sparse[i] === true);
    expect(JSON.parse(JSON.stringify({ v: sane })).v).toEqual([false, true]);
  });

  it('el payload serializado solo contiene booleanos en enabledParams', () => {
    const json = JSON.parse(JSON.stringify(payload([true, false, true])));
    expect(json.enabledParams).toEqual([true, false, true]);
    for (const v of json.enabledParams) {
      expect(typeof v).toBe('boolean');
    }
  });

  it('un enabledParams vacío es un array válido, no null', () => {
    // Vec<bool> vacío es aceptable para Rust; null no.
    const json = JSON.parse(JSON.stringify(payload([])));
    expect(json.enabledParams).toEqual([]);
    expect(json.enabledParams).not.toBeNull();
  });

  it('los campos opcionales viajan como null explícito, no como undefined', () => {
    // undefined haría que la clave desapareciera del JSON.
    const json = JSON.parse(JSON.stringify(payload([])));
    expect(json).toHaveProperty('manualCommand', null);
    expect(json).toHaveProperty('notes', null);
  });

  it('api.saveProfile existe y es una función', () => {
    expect(typeof api.saveProfile).toBe('function');
  });
});

/**
 * Auditoría del patrón que rompió save_profile: un payload anidado cuando Rust
 * espera argumentos planos. save_benchmark recibe `benchmark` como uno de sus
 * argumentos planos (no el payload entero anidado), y así se comprueba.
 */
describe('payload de save_benchmark', () => {
  const benchmark = {
    id: 'bench_1',
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-07 11:36:12',
    duration_seconds: 60,
    avg_fps: 60,
    fps_1_percent_low: 48,
    fps_0_1_percent_low: 40,
    max_fps: 72,
    min_fps: 38,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 25,
    total_frames: 3600,
    notes: null,
    launch_command: 'gamemoderun %command%',
  };

  it('expone los comandos de benchmarks en el wrapper', () => {
    expect(typeof api.saveBenchmark).toBe('function');
    expect(typeof api.analyzeMangohudLog).toBe('function');
    expect(typeof api.compareBenchmarks).toBe('function');
    expect(typeof api.updateProfile).toBe('function');
  });

  it('el benchmark va como clave propia, no aplanado ni anidado dos veces', () => {
    // Rust: save_benchmark(mut benchmark: Benchmark, profile_id, game_name, ...)
    const payload = { benchmark, profileId: '', gameName: 'Palworld' };
    const json = JSON.parse(JSON.stringify(payload));
    expect(json).toHaveProperty('benchmark.id', 'bench_1');
    expect(json.benchmark).toHaveProperty('avg_fps', 60);
    // No debe existir una copia aplanada de los campos internos.
    expect(json).not.toHaveProperty('avg_fps');
  });

  it('los números enteros del benchmark no se convierten en null', () => {
    const json = JSON.parse(JSON.stringify(benchmark));
    expect(json.total_frames).toBe(3600);
    expect(json.appid).toBe(1623730);
  });
});

describe('get_local_cover', () => {
  it('manda launcher, appid, key y runner al backend', async () => {
    // El runner es imprescindible para Heronite: indexa el arte por
    // `<runner>:<appName>`. Si se deja de enviar, los juegos con arte en
    // art.json vuelven a salir sin carátula.
    const invoke = vi.fn().mockResolvedValue('data:image/png;base64,AAAA');
    vi.doMock('@tauri-apps/api/core', () => ({ invoke }));
    vi.resetModules();

    const { api: freshApi } = await import('./api');
    const uri = await freshApi.getLocalCover('heronite', 0, 'steam_2646460', 'sideload');

    expect(uri).toBe('data:image/png;base64,AAAA');
    expect(invoke).toHaveBeenCalledWith('get_local_cover', {
      launcher: 'heronite',
      appid: 0,
      key: 'steam_2646460',
      runner: 'sideload',
    });

    vi.doUnmock('@tauri-apps/api/core');
    vi.resetModules();
  });

  it('normaliza los opcionales a null (Tauri distingue null de undefined)', async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    vi.doMock('@tauri-apps/api/core', () => ({ invoke }));
    vi.resetModules();

    const { api: freshApi } = await import('./api');
    await freshApi.getLocalCover('steam');

    expect(invoke).toHaveBeenCalledWith('get_local_cover', {
      launcher: 'steam',
      appid: null,
      key: null,
      runner: null,
    });

    vi.doUnmock('@tauri-apps/api/core');
    vi.resetModules();
  });

  it('heroniteUninstalledCount llama a su comando', async () => {
    const invoke = vi.fn().mockResolvedValue(2);
    vi.doMock('@tauri-apps/api/core', () => ({ invoke }));
    vi.resetModules();

    const { api: freshApi } = await import('./api');
    expect(await freshApi.heroniteUninstalledCount()).toBe(2);
    expect(invoke).toHaveBeenCalledWith('heronite_uninstalled_count');

    vi.doUnmock('@tauri-apps/api/core');
    vi.resetModules();
  });
});
