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

export interface GameInfo {
  name?: string;
  native?: boolean;
  tier?: string;
  score?: number;
  total?: number;
  confidence?: string;
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
}

export type View = 'recommendations' | 'tests' | 'compatibility' | 'benchmarks';
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
  launch_command: string;
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
