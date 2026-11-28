// ==================== WRAPPER TIPADO DE INVOKE ====================
// Único punto donde viven los nombres de los comandos Rust y sus tipos.
// Si un comando cambia en main.rs, se cambia aquí y el compilador avisa.

import { invoke } from '@tauri-apps/api/core';
import type {
  AnalisisComando,
  CambioAjuste,
  EstadoAjuste,
  ResultadoSerializacion,
} from './motorComando';
import type {
  OpcionesSteam,
  AntiCheatReport,
  Benchmark,
  BenchmarkComparison,
  CommunityRecommendation,
  FinishedLog,
  Game,
  OrigenComando,
  GameEngine,
  GameInfo,
  GameRules,
  GroupComparison,
  Hardware,
  ImportPreview,
  Profile,
  ProtonPulseSummary,
  RequirementsReport,
  RuntimeEnv,
  ShaderCache,
  SesionDeEjecucion,
  EstadoObservador,
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
  /**
   * Campo heredado. Los modos experto/guiado ya no existen en la interfaz, pero el
   * `Profile` persistido lo tiene y no se migra: se sigue enviando (siempre `false`) para
   * que el formato guardado no cambie y los perfiles antiguos no se rompan al re-guardar.
   */
  expertMode?: boolean;
  manualCommand: string | null;
  useManualCommand: boolean;
  notes: string | null;
  /**
   * Comando **exacto** que el motor confirmó. Es lo que se guarda y lo que se enseña al
   * leer el perfil; sin él, la pestaña Perfiles lo reconstruía y podía salir otro texto.
   */
  command: string | null;
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
  /** Evaluación registrada al guardar. Ausente = no registrada (comparación histórica). */
  evaluacion?: EvaluacionPersistida | null;
  /**
   * Clasificación calculada por el backend al leer. Rust es la fuente de verdad: el
   * frontend la presenta, no la deduce.
   */
  estado_de_la_evaluacion?: EstadoDeLaEvaluacionLeida;
}

/**
 * Versión de la metodología de interpretación. Texto, no unión cerrada: un valor de una
 * versión futura (`v9`) tiene que poder conservarse tal cual.
 */
export type VersionDeInterpretacion = string;

/** Estado de una evaluación. Texto por el mismo motivo. */
export type CodigoDeEvaluacion = string;

/** Cómo se leyó la evaluación guardada. La decide el backend, no el frontend. */
export type EstadoDeLaEvaluacionLeida =
  | 'no_registrada'
  | 'soportada'
  | 'no_soportada'
  | 'malformada';

export type DireccionDeCambio = 'sube' | 'baja' | 'igual' | 'no_consta';

export type PreferenciaDeMetrica =
  | 'favor_de_candidata'
  | 'favor_de_referencia'
  | 'indistinto'
  | 'no_establecida';

export type CompatibilidadMetodologica = 'establecida' | 'no_establecida';

export interface IdDeBenchmark {
  id: string;
}

/** Apoyo estadístico de una grabación en una métrica (bloque A). */
export interface ApoyoDeMetrica {
  percentil?: {
    muestras: number;
    cola_nominal: number;
    observaciones_interpoladas: number;
    muestras_en_la_media?: number | null;
  } | null;
}

export interface EvaluacionDeMetrica {
  clave: string;
  antes?: number | null;
  despues?: number | null;
  diferencia?: number | null;
  diferencia_pct?: number | null;
  direccion: DireccionDeCambio;
  preferencia: PreferenciaDeMetrica;
  confianza?: CodigoDeEvaluacion | null;
  apoyo_referencia?: ApoyoDeMetrica | null;
  apoyo_candidata?: ApoyoDeMetrica | null;
  motivos?: CodigoDeEvaluacion[];
  limitaciones?: CodigoDeEvaluacion[];
}

export interface CondicionesDeComparacion {
  compatibilidad_metodologica: CompatibilidadMetodologica;
  margen_porcentaje?: number | null;
  ruido_combinado_porcentaje?: number | null;
  comandos_iguales?: boolean | null;
  variable_evaluada?: { codigo: string } | null;
}

/**
 * Declaración de qué variable de configuración se estaba midiendo.
 *
 * Opcional y **explícita**: si no se declara, la evaluación queda sin variable. No se
 * infiere del comando ni del nombre del perfil. Viaja **igual** en la vista previa y en
 * el guardado: si difieren, Rust rechaza el guardado por `significado_cambiado`.
 */
export interface VariableEvaluada {
  codigo: string;
}

/**
 * Lo que Rust devuelve al evaluar una comparación y lo que la interfaz debe devolver
 * intacto para guardarla.
 *
 * La comparación vivida **dentro** de la confirmación es deliberado: no existe un segundo
 * comando que devuelva la comparación sin evaluación.
 */
export interface ConfirmacionDeVistaPrevia {
  evaluacion: EvaluacionPersistida;
  comparacion: BenchmarkComparison;
}

/** Los cuatro motivos de rechazo que produce el motor. Códigos estables de `evaluacion.rs`. */
export type CodigoDeMotivoDeRechazo =
  | 'entradas_cambiadas'
  | 'significado_cambiado'
  | 'evaluacion_no_corresponde'
  | 'valor_no_persistible';

/**
 * Error del contrato IPC de comparación guardada.
 *
 * Tipado **por origen**: la interfaz elige qué hacer con el discriminante, nunca analizando
 * el texto de un mensaje. No es un formato en disco: los archivos no cambian.
 */
export type ErrorDeEvaluacion =
  | { tipo: 'confirmacion_rechazada'; motivo: CodigoDeMotivoDeRechazo | string }
  | { tipo: 'grabacion_inexistente'; mensaje: string }
  | { tipo: 'identificador_ambiguo'; mensaje: string }
  | { tipo: 'comparacion_ya_existente'; comparacion_id: string }
  | { tipo: 'lectura_fallida'; archivo: string; mensaje: string }
  | { tipo: 'escritura_fallida'; mensaje: string };

export interface EvaluacionPersistida {
  metodologia_interpretacion: VersionDeInterpretacion;
  evaluada_en: string;
  referencia: IdDeBenchmark;
  candidata: IdDeBenchmark;
  estado: CodigoDeEvaluacion;
  confianza: CodigoDeEvaluacion;
  metricas?: EvaluacionDeMetrica[];
  motivos?: CodigoDeEvaluacion[];
  limitaciones?: CodigoDeEvaluacion[];
  condiciones: CondicionesDeComparacion;
}

/** update_profile reutiliza los mismos campos salvo appid y gameName, que no cambia. */
/** Argumentos de save_benchmark: el propio benchmark ya calculado + metadatos. */
/**
 * Lo que la interfaz sabe de la configuración **de un juego concreto** al guardar.
 *
 * Todos los campos son opcionales: si el juego abierto no es el de la grabación se manda
 * `correspondeAlJuego: false` y el backend registra la configuración como desconocida.
 */
export interface ContextoDeCaptura {
  /** AppID real del juego de la prueba (no el `0` de las reglas genéricas). */
  appidReal?: number | null;
  comandoConfirmado?: string | null;
  revisionMotor?: number | null;
  opcionesSteam?: string | null;
  perfilId?: string | null;
  perfilNombre?: string | null;
  cambiosSinGuardar?: boolean | null;
  modoProton?: boolean | null;
  correspondeAlJuego: boolean;
}

export interface SaveBenchmarkPayload {
  benchmark: Benchmark;
  profileId: string;
  gameName: string;
  appid: number;
  launchCommand: string;
  /**
   * De dónde salió el comando. Se guarda con la prueba para poder auditarla después.
   * Opcional: si falta, el backend asume `desconocido`.
   */
  origenComando?: OrigenComando;
  /**
   * Opciones que **Steam** tenía configuradas (`localconfig.vdf`) al guardar la prueba.
   *
   * Es lo que de verdad se ejecutó, así que es el dato que responde «¿con qué argumento
   * hice esta prueba?». `launchCommand` es lo que Thoriapp proponía en su editor y puede
   * no coincidir. `null`/`undefined` = no se pudo leer; `''` es un dato real («sin
   * opciones»). Nunca se inventa.
   */
  steamCommand?: string | null;
  /**
   * Contexto de captura para la instantánea histórica (subfase 5.2).
   *
   * Va vacío cuando el editor tenía abierto otro juego que el de la grabación: es
   * preferible registrar la configuración como desconocida a atribuirle la de otro juego.
   */
  contexto?: ContextoDeCaptura | null;
  notes: string | null;
}

export type UpdateProfilePayload = Omit<SaveProfilePayload, 'appid' | 'gameName'>;

export const api = {
  // ---------- Hardware ----------
  getHardware: () => invoke<HardwareResponse>('get_hardware'),

  // ---------- Biblioteca ----------
  scanLibrary: () => invoke<Game[]>('scan_library'),
  scanExternalLaunchers: () => invoke<RawExternalGame[]>('scan_external_launchers'),
  /**
   * Igual que `scanExternalLaunchers`, pero además invalida la caché de catálogo del
   * motor de procesos: un juego recién instalado se reconoce sin esperar la
   * revalidación periódica. Es el botón de actualizar biblioteca.
   */
  refreshLibrary: () => invoke<RawExternalGame[]>('refresh_library'),
  /** Juegos de Heronite ocultos por estar desinstalados (`is_installed: false`). */
  heroniteUninstalledCount: () => invoke<number>('heronite_uninstalled_count'),
  searchStore: (term: string) => invoke<Game[]>('search_store', { term }),

  /**
   * Opciones de lanzamiento que Steam tiene configuradas para un juego.
   *
   * Solo lectura. Devuelve `null` cuando no se puede saber (sin Steam, sin
   * `localconfig.vdf`, o el juego no aparece), que es **distinto** de Steam tenerlas
   * vacías (`opciones: ''`).
   */
  steamLaunchOptions: (appid: number) =>
    invoke<OpcionesSteam | null>('steam_launch_options', { appid }),

  /**
   * Analiza un comando y devuelve su modelo estructurado.
   *
   * El resultado se reenvía tal cual a `serializeLaunchCommand` y
   * `launchCommandStates`: el análisis y el comando del que sale tienen que ser los
   * mismos, o los índices apuntarían a otro sitio.
   */
  analyzeLaunchCommand: (command: string, appid: number, extras: string[] = []) =>
    invoke<AnalisisComando>('analyze_launch_command', { command, appid, extras }),

  /**
   * Estado de cada ajuste del catálogo respecto al comando (activo, inactivo, parcial,
   * ambiguo, conflicto). La UI lo pinta sin deducir nada.
   */
  launchCommandStates: (analisis: AnalisisComando, appid: number, extras: string[] = []) =>
    invoke<EstadoAjuste[]>('launch_command_states', { analisis, appid, extras }),

  /**
   * Catálogo efectivo con el índice real de cada ajuste.
   *
   * La interfaz **no** puede calcular estos índices por su cuenta: el catálogo del motor
   * salta las recomendaciones que ya están en el perfil del juego, y ese salto no se ve
   * desde aquí. Calcular `params.length + indexOf` desplazaba todos los índices posteriores
   * y el toggle escribía en otro ajuste.
   */
  catalogoDeAjustes: (appid: number, extras: string[] = []) =>
    invoke<{ indice: number; valor: string }[]>('catalogo_de_ajustes', { appid, extras }),

  /**
   * Traduce «quiero estos ajustes» a los cambios concretos, decidiendo en Rust qué sobra
   * y qué falta. TS no implementa esta comparación.
   */
  launchCommandChanges: (
    analisis: AnalisisComando,
    appid: number,
    activos: number[],
    extras: string[] = [],
  ) => invoke<CambioAjuste[]>('launch_command_changes', { analisis, appid, activos, extras }),

  /** Quita un argumento conservado, si el motor considera seguro hacerlo. */
  launchCommandRemoveSegment: (analisis: AnalisisComando, indice: number) =>
    invoke<ResultadoSerializacion>('launch_command_remove_segment', { analisis, indice }),

  /**
   * Aplica los cambios sobre el comando original.
   *
   * No lanza excepción cuando el motor no puede: devuelve `aplicado: false` con el motivo
   * y el comando original intacto, para poder conservar el último estado bueno.
   */
  serializeLaunchCommand: (
    analisis: AnalisisComando,
    appid: number,
    cambios: CambioAjuste[],
    extras: string[] = [],
  ) =>
    invoke<ResultadoSerializacion>('serialize_launch_command', {
      analisis,
      appid,
      cambios,
      extras,
    }),

  /**
   * Valida un comando para lanzar. Solo gramática y reglas soportadas por Thoriapp: no
   * garantiza que el juego arranque.
   */
  validateLaunchCommand: (command: string) =>
    invoke<null>('validate_launch_command', { command }),

  /**
   * Envoltorios conocidos por el sistema y por el catálogo.
   *
   * Rust es la única fuente de verdad desde la fase 2: el frontend pregunta en vez de
   * mantener una copia que se pueda quedar corta (ese fue el bug de `prime-run`,
   * `vkbasalt`, `obs-gamecapture`, `game-performance` y `scx_lavd`, que acababan DETRÁS de
   * `%command%` como si fueran argumentos del juego).
   */
  knownWrappers: () => invoke<string[]>('known_wrappers'),

  /**
   * Igual que `steamLaunchOptions`, pero para una grabación.
   *
   * El AppID de un CSV de MangoHud llega a 0 (el fichero no lo trae), así que el backend lo
   * resuelve por nombre contra la biblioteca instalada de Steam. Devuelve `null` solo
   * cuando no se puede saber nada.
   */
  steamLaunchOptionsParaJuego: (juego: { appid?: number | null; gameName?: string | null }) =>
    invoke<OpcionesSteam | null>('steam_launch_options_para_juego', {
      appid: juego.appid || null,
      gameName: juego.gameName || null,
    }),

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
      // Heredado: el formato persistido lo conserva y siempre va en `false`.
      expertMode: p.expertMode ?? false,
      manualCommand: p.manualCommand,
      useManualCommand: p.useManualCommand,
      notes: p.notes,
      command: p.command,
    }),
  updateProfile: (profileId: string, p: UpdateProfilePayload) =>
    invoke<Profile>('update_profile', {
      profileId,
      name: p.name,
      enabledParams: p.enabledParams,
      selectedCommunityRecs: p.selectedCommunityRecs,
      selectedEngineRecs: p.selectedEngineRecs,
      forceProton: p.forceProton,
      // Heredado: el formato persistido lo conserva y siempre va en `false`.
      expertMode: p.expertMode ?? false,
      manualCommand: p.manualCommand,
      useManualCommand: p.useManualCommand,
      notes: p.notes,
      command: p.command,
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
  /**
   * Grabaciones de un juego.
   *
   * `gameName` hace falta de verdad: las grabaciones antiguas tienen `appid: 0` (MangoHud
   * no escribe el AppID en su CSV) y solo se identifican por nombre. Mandando solo el
   * appid, la ventana «Pruebas del juego» decía que no había pruebas aunque las hubiera.
   */
  loadBenchmarksForGame: (juego: { appid: number; gameName?: string }) =>
    invoke<Benchmark[]>('load_benchmarks_for_game', {
      appid: juego.appid,
      gameName: juego.gameName ?? null,
    }),
  analyzeMangohudLog: (logPath: string, warmupSeconds: number) =>
    invoke<Benchmark>('analyze_mangohud_log_command', { logPath, warmupSeconds }),
  saveBenchmark: (p: SaveBenchmarkPayload) =>
    invoke<Benchmark>('save_benchmark', {
      benchmark: p.benchmark,
      profileId: p.profileId,
      gameName: p.gameName,
      appid: p.appid,
      launchCommand: p.launchCommand,
      origenComando: p.origenComando ?? 'desconocido',
      steamCommand: p.steamCommand ?? null,
      notes: p.notes,
      contexto: p.contexto ?? null,
    }),
  /**
   * Último comando registrado de verdad en una prueba de este juego.
   *
   * Se usa cuando la grabación es de un juego que no está abierto: sin esto, la prueba se
   * guardaría sin comando aunque Thoriapp lo tuviera apuntado de antes. `null` = no hay
   * dato; no se inventa.
   *
   * El nombre hace falta porque `appid` llega a 0 al analizar un CSV (MangoHud no lo
   * escribe en el fichero): el nombre del juego es la única identidad disponible.
   */
  ultimoComandoRegistrado: (juego: { appid: number; gameName: string }) =>
    invoke<string | null>('ultimo_comando_registrado', {
      appid: juego.appid || null,
      gameName: juego.gameName || null,
    }),
  deleteBenchmark: (benchmarkId: string) => invoke<void>('delete_benchmark', { benchmarkId }),
  /**
   * Borra solo el historial de un juego.
   *
   * `deleteAllBenchmarks` vacía la lista entera: desde la ficha de un juego eso se
   * llevaría por delante las pruebas de todos los demás. Devuelve cuántas se borraron.
   */
  deleteBenchmarksForGame: (juego: { appid: number; gameName: string }) =>
    invoke<number>('delete_benchmarks_for_game', {
      appid: juego.appid || 0,
      gameName: juego.gameName || '',
    }),
  deleteAllBenchmarks: () => invoke<void>('delete_all_benchmarks'),
  /**
   * Vista previa **evaluada** de dos grabaciones: el único camino a una comparación
   * individual. Devuelve la evaluación y la comparación juntas.
   *
   * Rechaza con `ErrorDeEvaluacion` si un identificador falta o es ambiguo.
   */
  compareBenchmarksEvaluada: (
    baselineId: string,
    testId: string,
    variable: VariableEvaluada | null,
  ) =>
    invoke<ConfirmacionDeVistaPrevia>('compare_benchmarks_evaluada', {
      baselineId,
      testId,
      variable,
    }),
  /**
   * Guarda una comparación **con la evaluación confirmada**. Rust relee, recalcula y
   * comprueba que la confirmación sigue siendo válida antes de escribir.
   *
   * `variable` tiene que ser la misma con la que se pidió la vista previa; si no, se
   * rechaza con `significado_cambiado`.
   */
  saveComparisonEvaluada: (
    confirmacion: ConfirmacionDeVistaPrevia,
    variable: VariableEvaluada | null,
  ) =>
    invoke<SavedComparison>('save_comparison_evaluada', { confirmacion, variable }),

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
  /**
   * Compara dos grupos de grabaciones. Devuelve una entrada por métrica
   * (`avg_fps`, `fps1_percent_low`, `fps01_percent_low`).
   */
  compareBenchmarkGroups: (antesIds: string[], despuesIds: string[]) =>
    invoke<GroupComparison[]>('compare_benchmark_groups', { antesIds, despuesIds }),
  /** ¿Esta ruta de CSV ya está guardada en el historial? */
  isLogAlreadySaved: (logPath: string) =>
    invoke<boolean>('is_log_already_saved', { logPath }),
  /** Grabaciones nuevas (ya cerradas) que Thoriapp todavía no conoce. */
  scanNewMangohudLogs: (folder: string) =>
    invoke<FinishedLog[]>('scan_new_mangohud_logs', { folder }),
  /** Todas las grabaciones cerradas de la carpeta, procesadas o no. */
  listMangohudLogs: (folder: string) =>
    invoke<FinishedLog[]>('list_mangohud_logs', { folder }),
  markLogsProcessed: (paths: string[]) =>
    invoke<void>('mark_logs_processed', { paths }),
  processedLogsCount: () => invoke<number>('processed_logs_count'),
  forgetProcessedLogs: () => invoke<void>('forget_processed_logs'),
  loadComparisons: () => invoke<SavedComparison[]>('load_comparisons'),
  deleteComparison: (comparisonId: string) =>
    invoke<void>('delete_comparison', { comparisonId }),
  deleteAllComparisons: () => invoke<void>('delete_all_comparisons'),
  // ---- Sesiones de ejecución (subfase 5.3) ----
  /** Qué está haciendo el observador de procesos, y su coste medido. */
  estadoObservador: () => invoke<EstadoObservador>('estado_observador'),
  /** Activa o desactiva el observador. Apagado no se lee ni una vez `/proc`. */
  configurarObservador: (activo: boolean) =>
    invoke<boolean>('configurar_observador', { activo }),
  listarSesiones: () => invoke<SesionDeEjecucion[]>('listar_sesiones'),
  /** Sesión de una prueba guardada, si sigue existiendo. `null` es una respuesta válida. */
  sesionDeBenchmark: (benchmarkId: string) =>
    invoke<SesionDeEjecucion | null>('sesion_de_benchmark', { benchmarkId }),
  /** Asigna a mano el juego de una sesión que no se pudo identificar. */
  asignarJuegoASesion: (sesionId: string, appid: number, nombre: string) =>
    invoke<void>('asignar_juego_a_sesion', { sesionId, appid, nombre }),
  /** Qué juego mirar en `/proc`. No es evidencia de identidad. */
  fijarPistaDeJuego: (nombre: string | null) =>
    invoke<void>('fijar_pista_de_juego', { nombre }),
  /** Guarda la configuración vigente para capturarla con su instante si hay juego vivo. */
  fijarConfiguracionDeSesion: (cfg: {
    comando: string | null;
    procedencia: string | null;
    perfil: string | null;
    opcionesSteam: string | null;
  }) => invoke<void>('fijar_configuracion_de_sesion', cfg),
};

export type Api = typeof api;
