// ==================== WRAPPER TIPADO DE INVOKE ====================
// Único punto donde viven los nombres de los comandos Rust y sus tipos.
// Si un comando cambia en main.rs, se cambia aquí y el compilador avisa.

import { invoke } from '@tauri-apps/api/core';
import type {
  AntiCheatReport,
  Benchmark,
  BenchmarkComparison,
  CommunityRecommendation,
  Game,
  GameEngine,
  GameInfo,
  GameRules,
  Hardware,
  ImportPreview,
  Profile,
  ProtonPulseSummary,
  RequirementsReport,
  RuntimeEnv,
  ShaderCache,
} from './types';

/** Forma cruda que devuelve el backend `scan_external_launchers`. */
export interface RawExternalGame {
  name: string;
  launcher: string;
  install_path: string;
  runner?: string;
  /** AppID de Steam si el juego de otro launcher es en realidad de Steam. */
  appid?: number | null;
  /** Clave con la que el launcher limpia/guarda su carátula en disco. */
  cover_key?: string | null;
}

/** Hardware tal cual lo entrega Rust (`HwInfo` con flatten de `Hardware`). */
export type HardwareResponse = Hardware;

/** Campos que espera `save_profile`; se envían planos y sin conversión de nombres. */
export interface SaveProfilePayload {
  name: string;
  appid: number;
  gameName: string;
  enabledParams: boolean[];
  selectedCommunityRecs: number[];
  selectedEngineRecs: number[];
  forceProton: boolean;
  expertMode: boolean;
  manualCommand: string | null;
  useManualCommand: boolean;
  notes: string | null;
}

/** Una comparación guardada, tal como la devuelve Rust. */
export interface SavedComparison {
  id: string;
  saved_at: string;
  game_name: string;
  before_fps: number;
  after_fps: number;
  fps_change_percent: number;
  comparison: BenchmarkComparison;
}

/** update_profile reutiliza los mismos campos salvo appid y gameName, que no cambia. */
/** Argumentos de save_benchmark: el propio benchmark ya calculado + metadatos. */
export interface SaveBenchmarkPayload {
  benchmark: Benchmark;
  profileId: string;
  gameName: string;
  appid: number;
  launchCommand: string;
  notes: string | null;
}

export type UpdateProfilePayload = Omit<SaveProfilePayload, 'appid' | 'gameName'>;

export const api = {
  // ---------- Hardware ----------
  getHardware: () => invoke<HardwareResponse>('get_hardware'),

  // ---------- Biblioteca ----------
  scanLibrary: () => invoke<Game[]>('scan_library'),
  scanExternalLaunchers: () => invoke<RawExternalGame[]>('scan_external_launchers'),
  /** Juegos de Heronite ocultos por estar desinstalados (`is_installed: false`). */
  heroniteUninstalledCount: () => invoke<number>('heronite_uninstalled_count'),
  searchStore: (term: string) => invoke<Game[]>('search_store', { term }),

  /**
   * Carátula que el launcher guarda en disco (Lutris, Heroic, Heronite),
   * devuelta como data URI base64 lista para un <img>. `null` si no tiene.
   */
  getLocalCover: (
    launcher: string,
    appid?: number | null,
    key?: string | null,
    runner?: string | null
  ) =>
    invoke<string | null>('get_local_cover', {
      launcher,
      appid: appid ?? null,
      key: key ?? null,
      runner: runner ?? null,
    }),

  /** Texto de diagnóstico: qué carpetas de carátulas existen y qué archivos hay. */
  diagnoseCovers: () => invoke<string>('diagnose_covers'),

  // ---------- Juego / reglas ----------
  getRules: (appid: number) => invoke<GameRules>('get_rules', { appid }),
  /**
   * Requisitos de Steam frente al hardware de esta máquina. `installPath` solo
   * sirve para medir el espacio libre en la partición correcta.
   */
  getRequirements: (appid: number, installPath?: string | null) =>
    invoke<RequirementsReport>('get_requirements', {
      appid,
      installPath: installPath ?? null,
    }),
  gameInfo: (appid: number) => invoke<GameInfo>('game_info', { appid }),
  detectEngine: (appid: number, gameName: string, installPath?: string | null) =>
    invoke<GameEngine | null>('detect_engine', {
      appid,
      gameName,
      installPath: installPath ?? null,
    }),
  /**
   * Parámetros recomendados para el motor, ya filtrados por la GPU.
   *
   * El nombre de la GPU es obligatorio en la práctica: sin él no se sabe si la
   * tarjeta aguanta ray tracing y los parámetros que lo activan (`VKD3D_CONFIG=dxr`)
   * se quedan fuera. Pasar `null` es la opción segura, no la completa.
   */
  getEngineRecommendations: (engine: GameEngine, gpuName?: string | null) =>
    invoke<string[]>('get_engine_recommendations_for_gpu', {
      engine,
      gpuName: gpuName ?? null,
    }),
  getCommunityRecommendations: (hwTags: string[]) =>
    invoke<CommunityRecommendation[]>('get_community_recommendations', { hwTags }),

  // ---------- Perfiles ----------
  loadProfiles: () => invoke<Profile[]>('load_profiles'),
  /**
   * Rust recibe los campos como argumentos planos. Pasar el objeto anidado bajo
   * una sola clave hace que Tauri no encuentre `name` ni el resto y devuelva
   * «invalid args», así que los campos se envían aplanados uno a uno.
   */
  saveProfile: (p: SaveProfilePayload) =>
    invoke<Profile>('save_profile', {
      name: p.name,
      appid: p.appid,
      gameName: p.gameName,
      enabledParams: p.enabledParams,
      selectedCommunityRecs: p.selectedCommunityRecs,
      selectedEngineRecs: p.selectedEngineRecs,
      forceProton: p.forceProton,
      expertMode: p.expertMode,
      manualCommand: p.manualCommand,
      useManualCommand: p.useManualCommand,
      notes: p.notes,
    }),
  updateProfile: (profileId: string, p: UpdateProfilePayload) =>
    invoke<Profile>('update_profile', {
      profileId,
      name: p.name,
      enabledParams: p.enabledParams,
      selectedCommunityRecs: p.selectedCommunityRecs,
      selectedEngineRecs: p.selectedEngineRecs,
      forceProton: p.forceProton,
      expertMode: p.expertMode,
      manualCommand: p.manualCommand,
      useManualCommand: p.useManualCommand,
      notes: p.notes,
    }),
  deleteProfile: (profileId: string) => invoke<void>('delete_profile', { profileId }),
  reorderProfiles: (profileIds: string[]) => invoke<void>('reorder_profiles', { profileIds }),
  exportProfile: (profileId: string, path: string) =>
    invoke<string>('export_profile', { profileId, path }),
  exportAllProfiles: (path: string) => invoke<string>('export_all_profiles', { path }),
  importProfileFromText: (content: string) =>
    invoke<Profile[]>('import_profile_from_text', { content }),
  /**
   * Mira qué se importaría, SIN escribir nada.
   *
   * Se llama antes de `importProfileFromText` para poder enseñar el resultado y
   * que el usuario confirme. La importación real solo corre si acepta.
   */
  previewImportProfile: (content: string) =>
    invoke<ImportPreview>('preview_import_profile', { content }),

  // ---------- Pruebas de rendimiento ----------
  loadBenchmarks: () => invoke<Benchmark[]>('load_benchmarks'),
  loadBenchmarksForGame: (appid: number) =>
    invoke<Benchmark[]>('load_benchmarks_for_game', { appid }),
  analyzeMangohudLog: (logPath: string, warmupSeconds: number) =>
    invoke<Benchmark>('analyze_mangohud_log_command', { logPath, warmupSeconds }),
  saveBenchmark: (p: SaveBenchmarkPayload) =>
    invoke<Benchmark>('save_benchmark', {
      benchmark: p.benchmark,
      profileId: p.profileId,
      gameName: p.gameName,
      appid: p.appid,
      launchCommand: p.launchCommand,
      notes: p.notes,
    }),
  deleteBenchmark: (benchmarkId: string) => invoke<void>('delete_benchmark', { benchmarkId }),
  deleteAllBenchmarks: () => invoke<void>('delete_all_benchmarks'),
  compareBenchmarks: (baselineId: string, testId: string) =>
    invoke<BenchmarkComparison>('compare_benchmarks', { baselineId, testId }),
  saveComparison: (baselineId: string, testId: string) =>
    invoke<unknown>('save_comparison', { baselineId, testId }),

  // ---------- Utilidades ----------
  openUrl: (url: string) => invoke<void>('open_url', { url }),
  detectMangohudLogPath: () => invoke<string>('detect_mangohud_log_path'),
  checkMangohudInstalled: () => invoke<boolean>('check_mangohud_installed'),
  /** Qué wrappers, capas Vulkan y sandbox hay de verdad en la máquina. */
  getRuntimeEnv: () => invoke<RuntimeEnv>('get_runtime_env_command'),
  /** Anti-cheat del juego según AreWeAntiCheatYet (caché de 24 h en el backend). */
  getAnticheatInfo: (appid: number) => invoke<AntiCheatReport>('get_anticheat_info', { appid }),
  /**
   * Reportes de Proton Pulse de ESTE juego, filtrados por la GPU del equipo.
   *
   * `null` = el juego no tiene reportes (o es de otro launcher, sin appid). El
   * filtro por hardware se hace en Rust a partir del nombre de la GPU: pasar
   * `null` deja todos los vendors mezclados, que es lo que NO se quiere.
   */
  getProtonPulseReports: (appid: number, gpu?: string | null) =>
    invoke<ProtonPulseSummary | null>('get_proton_pulse_reports', {
      appid,
      gpu: gpu ?? null,
    }),
  /** Caché de shaders del juego en disco (solo recorre directorios, no lanza nada). */
  getShaderCache: (appid: number) => invoke<ShaderCache>('get_shader_cache', { appid }),
  getMangohudLogDuration: () => invoke<number>('get_mangohud_log_duration'),
  setMangohudLogDuration: (seconds: number) =>
    invoke<number>('set_mangohud_log_duration', { seconds }),
  /** Deshace el último cambio de log_duration desde la copia .thoriapp.bak. */
  restoreMangohudConf: () => invoke<number>('restore_mangohud_conf'),
  loadComparisons: () => invoke<SavedComparison[]>('load_comparisons'),
  deleteComparison: (comparisonId: string) =>
    invoke<void>('delete_comparison', { comparisonId }),
  deleteAllComparisons: () => invoke<void>('delete_all_comparisons'),
};

export type Api = typeof api;
