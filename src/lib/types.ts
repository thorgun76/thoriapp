// ==================== TIPOS COMPARTIDOS ====================
// Tipos que espejan las estructuras serializadas desde src-tauri/src/*.rs
// Deben coincidir con los #[derive(Serialize)] del backend.

export interface Hardware {
  gpu: string;
  vendor: string;
  cpu: string;
  ram_gb: number;
  kernel: string;
  distro: string;
  session: string;
  mesa: string;
  ntsync: boolean;
  game_performance: boolean;
  /** VRAM dedicada en MiB. `null` en iGPU o si `vulkaninfo` no la reporta. */
  vram_mb: number | null;
  cpu_threads: number | null;
  cpu_cores: number | null;
  /** `null` = no se pudo determinar (no es lo mismo que "sin AVX2"). */
  cpu_avx2: boolean | null;
  tags: string[];
}

export interface Game {
  appid: number;
  name: string;
  installed?: boolean;
  launcher?: string; // "steam", "lutris", "heroic", "heronite"
  install_path?: string;
  runner?: string;
  /** Clave con la que el launcher guarda la carátula en disco. */
  cover_key?: string;
}

/** Ejecutable encontrado en el sistema (lo devuelve `get_runtime_env`). */
export interface BinaryInfo {
  name: string;
  path: string;
}

/** Estado real de la máquina, para avisar antes de lanzar el juego. */
export interface RuntimeEnv {
  binaries: BinaryInfo[];
  mangohud_binary: string | null;
  mangohud_vulkan_layer: string | null;
  steam_flatpak: boolean;
  ntsync: boolean;
  kernel: string;
}

export interface Param {
  kind: string;
  value: string;
  title: string;
  why: string;
  impact: string;
  requires: string[];
  default_on: boolean;
  warn?: string;
  evidence: string;
  source?: string;
  proton_only: boolean;
  recommended_for?: string[];
  tested_on?: string;
  validated_date?: string;
  measured_improvement?: string;
  experimental?: boolean;
  not_recommended_for?: string[];
  essential?: boolean;
  simple_why?: string;
}

export interface GameRules {
  name: string;
  appid: number;
  bottleneck: string;
  summary: string;
  params: Param[];
  tips: string[];
  has_rules: boolean;
}

/**
 * De dónde salió el comando que se guarda con una grabación.
 *
 * Existe para poder auditar una prueba después: ver una prueba con `%command%` no dice si
 * de verdad no había ajustes o si el dato se leyó en el momento equivocado.
 *
 * - `instantanea`: tomado al detectar la grabación. Es el bueno.
 * - `comando_actual`: la instantánea estaba vacía y se usó el comando del editor al
 *   guardar. Puede no ser el de la grabación.
 * - `desconocido`: no había ningún comando; la prueba queda como «no registrado».
 */
export type OrigenComando = 'instantanea' | 'comando_actual' | 'desconocido';

/**
 * Opciones de lanzamiento leídas de Steam (`userdata/<id>/config/localconfig.vdf`).
 *
 * Solo lectura: Thoriapp nunca escribe en Steam (Steam reescribe `LaunchOptions` al
 * cerrarse y perdería el cambio).
 */
export interface OpcionesSteam {
  /** Texto exacto de `LaunchOptions`. `''` = Steam las tiene vacías. */
  opciones: string;
  /** Cuenta de Steam de la que se leyeron. */
  cuenta: string;
}

export interface GameInfo {
  name?: string;
  native?: boolean;
  tier?: string;
  score?: number;
  total?: number;
  confidence?: string;
  /**
   * AppID de Steam resuelto por nombre, solo cuando el juego no venía de Steam
   * (Lutris, Heroic…). Es el que enlaza la etiqueta de ProtonDB con su análisis.
   */
  appid?: number;
  errors: string[];
}

export interface GameEngine {
  name: string;
  version: string | null;
  api: string;
}

export interface CommunityRecommendation {
  parameter: string;
  description: string;
  optimized_for: string[];
  notes?: string;
  recommended?: boolean;
}

export interface Profile {
  id: string;
  name: string;
  appid: number;
  game_name: string;
  enabled_params: boolean[];
  selected_community_recs: number[];
  selected_engine_recs: number[];
  force_proton: boolean;
  expert_mode: boolean;
  manual_command?: string;
  use_manual_command: boolean;
  notes?: string;
  created_at: string;
  updated_at: string;
  order?: number;
  /**
   * Versión del formato de perfil (la 1 es la actual). Ausente = archivo antiguo,
   * que se trata como 1. Si llega mayor que `PROFILE_SCHEMA_VERSION` (Rust) el
   * backend rechaza la importación en vez de interpretarla a medias.
   */
  version?: number;
}

export type View = 'recommendations' | 'tests' | 'compatibility' | 'benchmarks';

/** Un perfil del archivo, tal como se ve en la vista previa de importación. */
export interface ImportedProfilePreview {
  id: string;
  name: string;
  game_name: string;
  appid: number;
  /** Nombres de las opciones activadas (no índices). */
  enabled_params: string[];
  manual_command?: string;
  /** `false` si Thoriapp le va a quitar el comando al importarlo. */
  keeps_manual_command: boolean;
  /** Motivo del rechazo del comando (vacío si se conserva). */
  manual_command_reason: string;
  duplicate: boolean;
}

/**
 * Resultado de analizar un archivo de perfiles sin escribir nada.
 *
 * Espeja `ImportPreview` en `profiles.rs`. La pantalla lo enseña antes de
 * importar para que el usuario vea qué entra y qué se descarta.
 */
export interface ImportPreview {
  profiles: ImportedProfilePreview[];
  duplicates: number;
  stripped_commands: number;
}
export type SidebarView = 'library' | 'tests' | 'profiles' | 'benchmarks';
export type LauncherFilter = 'all' | 'steam' | 'lutris' | 'heroic' | 'heronite';

// ==================== PRUEBAS DE RENDIMIENTO ====================
// Espejan los structs de src-tauri/src/benchmarks.rs

export interface Benchmark {
  id: string;
  profile_id: string;
  game_name: string;
  appid: number;
  timestamp: string;
  duration_seconds: number;
  avg_fps: number;
  fps_1_percent_low: number;
  fps_0_1_percent_low: number;
  max_fps: number;
  min_fps: number;
  frame_time_avg: number;
  frame_time_1_percent_high: number;
  total_frames: number;
  notes: string | null;
  /**
   * Comando configurado en Thoriapp **cuando se capturó la grabación**. Texto original,
   * sin normalizar. Vacío en pruebas guardadas antes de que se registrara: nunca se
   * inventa `%command%`.
   */
  launch_command: string;
  /**
   * De dónde salió `launch_command`. Ausente en pruebas antiguas (= `desconocido`).
   * Sirve para auditar: una prueba con `%command%` y origen `instantanea` significa «de
   * verdad no había ajustes»; con origen `comando_actual`, que el dato se leyó al guardar.
   */
  origen_comando?: OrigenComando;
  /**
   * Opciones que **Steam** tenía configuradas cuando se guardó la prueba.
   *
   * Es el comando que de verdad se ejecutó al entrar al juego, así que es el que dice con
   * qué argumento se hizo la prueba. `launch_command` es lo que Thoriapp proponía en su
   * editor y puede no coincidir (el usuario copia a Steam a mano).
   *
   * `undefined`/`null` = no se pudo leer Steam. `''` = se leyó y estaba vacío, que es un
   * dato real: la prueba se lanzó sin argumentos.
   */
  steam_command?: string | null;
  /**
   * Número de presentación de la grabación dentro de su juego («Prueba N»).
   *
   * Lo asigna el backend una sola vez al guardar y no cambia nunca: sobrevive a borrar
   * otras pruebas y al recorte del historial. Ausente/`null` en grabaciones anteriores a
   * este campo; no se les inventa una cronología, así que la interfaz las llama «Prueba
   * sin número».
   */
  number?: number | null;
  // Solo existen en benchmarks guardados con la versión nueva
  spikes_per_min?: number | null;
  pct_over_33ms?: number | null;
  jitter_ms?: number | null;
  worst_frame_ms?: number | null;
  /** Segundos iniciales descartados (calentamiento). `null` = grabación completa. */
  warmup_seconds?: number | null;
  /** Margen de ruido de esta grabación (±% FPS promedio, 95 % de confianza). */
  noise_percent?: number | null;
  /**
   * Coste del arranque: los primeros segundos frente al resto de la grabación.
   * Es donde aparece la compilación de shaders la primera vez. `null` = sin datos.
   */
  shader_stutter?: ShaderStutter | null;
  /**
   * Ruta del CSV del que salió la grabación. Es su identidad: dos análisis del mismo
   * fichero dan las mismas cifras. Vacío en benchmarks guardados antes de este campo.
   */
  log_path?: string;
  /**
   * Número de muestras del log. Es lo que `total_frames` contiene de verdad: MangoHud
   * guarda ~10 filas por segundo, no un frame por fila.
   */
  sample_count?: number | null;
  /**
   * Serie temporal para el gráfico de frametime. Ausente en las pruebas guardadas antes
   * de que existiera este campo (no se inventan datos para ellas).
   */
  muestras_temporales?: MuestrasTemporales | null;
}

/** Serie temporal de una grabación (frametime medio por muestra). */
export interface MuestrasTemporales {
  /** Frametime de cada muestra, en milisegundos. */
  frame_time_ms: number[];
  /** Instante de cada muestra en segundos desde el inicio. Vacío si el log no traía `elapsed`. */
  elapsed_s?: number[];
}

/** Arranque vs resto de una grabación (lo calcula el backend). */
export interface ShaderStutter {
  /** Segundos comparados al principio. */
  window_seconds: number;
  /** Peor 1 % de FPS en esa ventana inicial. */
  early_fps_1_percent_low: number;
  /** Peor 1 % de FPS en el resto de la grabación. */
  steady_fps_1_percent_low: number;
  /** % de frames por encima de 33 ms (menos de 30 FPS) al principio. */
  early_pct_over_33ms: number;
  /** Lo mismo en el resto. */
  steady_pct_over_33ms: number;
  /** Cuánto peor es el arranque que el resto, en %. 0 = no es peor. */
  stutter_penalty_percent: number;
}

/** Caché de shaders de un juego en el disco (lo calcula el backend). */
export interface ShaderCache {
  /** Bytes totales de las cachés encontradas. */
  bytes: number;
  locations: ShaderCacheEntry[];
  /** Steam ya tiene preparada la caché del juego (`shadercache/<appid>`). */
  steam_precached: boolean;
}

export interface ShaderCacheEntry {
  /** `steam`, `dxvk` o `prefix`. */
  kind: string;
  path: string;
  bytes: number;
}

export interface BenchmarkComparison {
  baseline: Benchmark;
  test: Benchmark;
  fps_change_percent: number;
  fps_1_percent_change_percent: number;
  fps_0_1_percent_change_percent: number;
  frame_time_change_percent: number;
  conclusion: string;
  /** Margen de ruido combinado de las dos grabaciones (%). */
  noise_percent?: number | null;
  /** `true` cuando el cambio de FPS entra dentro de ese margen. */
  within_noise?: boolean;
  /**
   * `true` solo si los comandos registrados de las dos grabaciones son exactamente el
   * mismo texto. `null`/`undefined` = alguno no está registrado (pruebas antiguas).
   */
  comandos_iguales?: boolean | null;
}

/**
 * Comparación de **grupos** de grabaciones (varias por configuración).
 *
 * La comparación de dos grabaciones sueltas no distingue una mejora real del ruido
 * del juego; con 3 o más por lado el backend da un intervalo de confianza.
 */
export interface GroupComparison {
  /** `avg_fps` | `fps1_percent_low` | `fps01_percent_low`. */
  metric: string;
  antes_runs: number;
  despues_runs: number;
  antes_median: number;
  despues_median: number;
  change_percent: number;
  ci_low_percent: number;
  ci_high_percent: number;
  /** Margen mínimo exigido a esta métrica (%), que crece con los percentiles. */
  required_margin_percent: number;
  /** `mejora` | `empeora` | `no_concluyente`. */
  verdict: string;
  conclusion: string;
}

/** Datos de anti-cheat de un juego (fuente: AreWeAntiCheatYet). */
export interface AntiCheatInfo {
  status: string;
  anticheats: string[];
  reference?: string | null;
  note?: string | null;
  updated?: string | null;
}

/** Reporte de un usuario de Proton Pulse, ya recortado (espeja `proton_pulse::ReportSummaryEntry`). */
export interface ProtonPulseEntry {
  gpu: string;
  /** `platinum` | `gold` | `silver` | `bronze` | `borked`. */
  rating: string;
  proton_version: string;
  launch_options?: string | null;
}

/**
 * Resumen de Proton Pulse para la GPU del equipo (espeja `proton_pulse::ReportSummary`).
 * `null` = el juego no tiene reportes; sin dato no se muestra tarjeta.
 */
export interface ProtonPulseSummary {
  /** Vendor que se filtró (`amd`/`nvidia`/`intel`); `null` si no se supo. */
  vendor: string | null;
  /** Reportes del juego de todos los vendors. */
  total: number;
  /** Reportes del vendor del equipo. */
  matching: number;
  /** De los anteriores, cuántos son platinum/gold. */
  good: number;
  /** Cuántos son `borked` (no arrancan con esa GPU). */
  borked: number;
  /** Cuántos son de la misma generación que el modelo de referencia. */
  same_family: number;
  reports: ProtonPulseEntry[];
}

export interface AntiCheatReport {
  /** `null` = el juego no está en el catálogo: no se sabe nada. */
  info: AntiCheatInfo | null;
  catalog_size: number;
  source: string;
}

// ==================== REQUISITOS DEL SISTEMA ====================
// Espejan `requirements.rs`

/** `ok` cumple con margen, `tight` cumple justo, `fail` no cumple, `unknown` sin dato. */
export type RequirementVerdict = 'ok' | 'tight' | 'fail' | 'unknown';

export interface RequirementCheck {
  /**
   * `ram`, `vram`, `storage`, `cpu` y `gpu` (con veredicto por tabla de rendimiento)
   * y `hardware` (nota informativa cuando no hay tabla para ese modelo).
   */
  kind: string;
  tier: string;
  /** Lo que pide el juego, textual ("12 GB RAM"). */
  required: string;
  /** Lo que tiene la máquina, ya formateado ("15.6 GB"). */
  actual: string;
  verdict: RequirementVerdict;
  detail: string;
}

export interface Requirement {
  tier: string;
  kind: string;
  text: string;
}

export interface RequirementsReport {
  checks: RequirementCheck[];
  overall: RequirementVerdict;
  /** Margen extra aplicado por correr en Proton (0/10/20 %). */
  proton_margin: number;
  margin_note: string;
  raw: Requirement[];
}

/**
 * Estado de la configuración **en el momento de grabar**.
 *
 * El comando de lanzamiento hay que apuntarlo al empezar la grabación, no al guardarla:
 * si se lee al guardar, refleja lo que el usuario tiene ahora en la app (que puede haber
 * cambiado, o ser el `%command%` por defecto si no hay reglas cargadas) y la comparación
 * acaba diciendo que las dos grabaciones tuvieron el mismo comando. Aquí se guarda lo que
 * de verdad estaba puesto cuando se pulsó Shift+F2.
 */
export interface RecordedSetup {
  /** Comando completo tal como estaba (p. ej. `MANGOHUD=1 gamemoderun mangohud %command%`). */
  command: string;
  /** Nombre del perfil activo en ese momento, si había uno. */
  profileName?: string;
  /** Segundos que duró (o se esperaba que durara) la grabación, si se conocía. */
  expectedSeconds?: number;
  /** Cuándo se pulsó grabar, en ISO. */
  at: string;
}

/**
 * Grabación de MangoHud que ya terminó (MangoHud escribió su `_summary.csv`).
 *
 * Se detecta vigilando la carpeta de logs: así el usuario graba con `Shift+F2` dentro del
 * juego y no tiene que ir a buscar el fichero a mano después.
 */
export interface FinishedLog {
  /** Ruta del CSV de datos, no del `_summary.csv`. */
  path: string;
  /** Ejecutable tal cual lo nombra MangoHud (`7DaysToDie.x86_64`, `Palworld-Win64-Shipping`). */
  program: string;
  /** Marca de tiempo del nombre, `YYYY-MM-DD_HH-MM-SS`. */
  timestamp: string;
}
