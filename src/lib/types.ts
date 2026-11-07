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
}

export interface BenchmarkComparison {
  baseline: Benchmark;
  test: Benchmark;
  fps_change_percent: number;
  fps_1_percent_change_percent: number;
  fps_0_1_percent_change_percent: number;
  frame_time_change_percent: number;
  conclusion: string;
}
