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
  /**
   * Solo tiene efecto bajo Proton/Wine. Lo decide Rust (`marcar_proton_only`) con el mismo
   * predicado que el catálogo de reglas. Es opcional para que una respuesta antigua o un
   * fixture de test sin el campo sigan siendo válidos; `esSoloProton` cubre ese hueco.
   */
  proton_only?: boolean;
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
  /**
   * Comando exacto confirmado al guardar. Si está, se muestra tal cual: es la única forma
   * de que el perfil diga lo mismo que la caja del comando. Ausente en los perfiles
   * antiguos, que se siguen reconstruyendo como antes.
   */
  command?: string;
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

// ==================== Instantánea de configuración (subfase 5.2) ====================

/**
 * De dónde salió un dato de la instantánea.
 *
 * `'verificado_en_ejecucion'` sigue sin usarse: comprobar el **efecto** de una optimización
 * no es lo mismo que ver el proceso corriendo, y eso no está resuelto.
 */
export type Procedencia =
  | 'motor_rust'
  | 'configurado_en_thoriapp'
  | 'leido_de_steam'
  | 'registrado_por_mangohud'
  | 'detectado_en_sistema'
  | 'declarado_por_usuario'
  | 'verificado_en_ejecucion'
  /** Leído del proceso del juego mientras corría (subfase 5.3). */
  | 'observado_en_proceso'
  /** Asignado a mano por el usuario. */
  | 'confirmado_por_usuario'
  /** Deducido de una coincidencia de nombre: es una inferencia, no una observación. */
  | 'inferido'
  | 'desconocido';

export interface ConProcedencia<T> {
  valor: T;
  procedencia: Procedencia;
  capturado_en?: string | null;
}

export interface IdentidadInstantanea {
  /** AppID real del juego; `null` si no se conoce (nunca el `0` de las reglas genéricas). */
  appid_real: number | null;
  /**
   * De dónde sale `appid_real`. `observado_en_proceso` = leído del proceso del juego;
   * `desconocido` = no hay dato fiable. La interfaz lo muestra en vez de dar el número a
   * secas, para que no parezca lo mismo un AppID observado que uno supuesto.
   */
  appid_procedencia?: Procedencia;
  benchmark_id: string;
  numero_prueba: number | null;
  fecha: string;
}

export interface ConfigInstantanea {
  /** Comando literal confirmado por el motor de Rust. */
  comando_confirmado: ConProcedencia<string> | null;
  revision_motor: number | null;
  opciones_steam: ConProcedencia<string> | null;
  perfil_id: string | null;
  perfil_nombre: ConProcedencia<string> | null;
  cambios_sin_guardar: boolean | null;
  modo_proton: ConProcedencia<boolean> | null;
  /** Solo si es verificable. Configurar Proton no demuestra que se usara. */
  ejecucion_verificada: ConProcedencia<string> | null;
}

export interface EntornoInstantanea {
  gpu: ConProcedencia<string> | null;
  cpu: ConProcedencia<string> | null;
  ram_gb: ConProcedencia<number> | null;
  kernel: ConProcedencia<string> | null;
  driver: ConProcedencia<string> | null;
  mesa: ConProcedencia<string> | null;
  /** Versión de Proton instalada o seleccionada. NO la que usó el proceso. */
  proton_seleccionado: ConProcedencia<string> | null;
  build_id: ConProcedencia<string> | null;
  log_interval_ms: ConProcedencia<number> | null;
}

/**
 * Equipo tal como lo registró **MangoHud** durante la grabación.
 *
 * Es un bloque independiente del entorno detectado al analizar: pueden ser equipos
 * distintos y momentos distintos. Ninguno rellena los huecos del otro.
 */
export interface HardwareDeLaGrabacion {
  /** `os` del CSV. Con Proton suele ser «Steam Runtime 4», que NO es la distro. */
  os: ConProcedencia<string> | null;
  cpu: ConProcedencia<string> | null;
  gpu: ConProcedencia<string> | null;
  /** RAM total tal como la escribió MangoHud (KiB), sin convertir. */
  ram_kib: ConProcedencia<number> | null;
  /** La misma RAM convertida a GiB. */
  ram_gib: ConProcedencia<number> | null;
  kernel: ConProcedencia<string> | null;
  /** Suele venir vacío con RADV: `null`, nunca deducido del nombre de la GPU. */
  driver: ConProcedencia<string> | null;
  cpuscheduler: ConProcedencia<string> | null;
  intervalo_muestreo_ms: ConProcedencia<number> | null;
}

/** Resumen calculado por MangoHud. Fuente aparte; no sustituye a las métricas de Thoriapp. */
export interface ResumenOriginal {
  valores: Array<[string, number]>;
  capturado_en: string;
}

/**
 * Instantánea histórica de la configuración de una prueba.
 *
 * `null` en los benchmarks anteriores a la subfase 5.2. No se reconstruye: los datos
 * actuales no dicen cómo se hizo una prueba del mes pasado.
 */
export interface SnapshotConfig {
  version: number;
  identidad: IdentidadInstantanea;
  configuracion: ConfigInstantanea;
  entorno: EntornoInstantanea;
  /** `null` = el CSV no traía cabecera de sistema: condiciones desconocidas. */
  hardware_grabacion: HardwareDeLaGrabacion | null;
  /** `null` = no había `_summary.csv` o no se pudo leer. */
  resumen_mangohud: ResumenOriginal | null;
  capturado_en: string;
  /** Se tomó después de terminar la grabación: no afirma qué estuvo activo durante ella. */
  posterior_a_la_grabacion: boolean;
}

export interface Benchmark {
  id: string;
  profile_id: string;
  game_name: string;
  appid: number;
  timestamp: string;
  duration_seconds: number;
  /**
   * Duración medida con reloj fiable, en segundos. `null`/ausente = no hay eje temporal
   * en el que confiar. **Cuando falta, `duration_seconds` vale 0 y ese cero significa
   * «no consta», no «cero segundos»**: comprobar este campo antes de mostrar la duración.
   */
  duracion_medida?: number | null;
  /** Con qué metodología de cálculo se obtuvieron los números. Ausente = no consta. */
  metodologia_calculo?: 'v2' | null;
  /** De dónde salen las muestras que entraron en el cálculo. */
  contabilidad_muestras?: ContabilidadDeMuestras | null;
  /** Apoyo real de cada percentil (sobre cuántas muestras se calculó). */
  apoyo_metricas?: ApoyoDeMetricas | null;
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
  /**
   * Instantánea histórica de la configuración (subfase 5.2).
   *
   * Ausente/`null` en benchmarks anteriores: no se reconstruye con datos actuales.
   */
  snapshot?: SnapshotConfig | null;
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

/**
 * Incidencias del reloj de la grabación (metodología v2).
 *
 * Son recuentos de diagnóstico, **no exclusiones**: una misma fila puede acumular varias
 * y aun así entrar en el cálculo de las métricas de rendimiento.
 */
export interface IncidenciasTemporales {
  filas_repetidas: number;
  orden_invertido: number;
  negativas: number;
  no_finitas: number;
  /** Total de filas sin un valor de reloj utilizable. */
  reloj_incompleto: number;
  /** Desglose del total anterior: la cabecera no traía columna `elapsed`. */
  columna_elapsed_ausente?: number;
  /** Desglose: la fila tenía menos columnas que la cabecera. */
  celda_reloj_ausente?: number;
  /** Desglose: la celda existía pero estaba vacía. */
  celda_reloj_vacia?: number;
  /** Desglose: texto no numérico o número no finito (NaN, inf). */
  celda_reloj_ilegible?: number;
}

/** Por qué no se pudo aplicar el calentamiento solicitado. */
export type MotivoSinCalentamiento =
  | 'no_solicitado'
  | 'sin_eje_temporal'
  | 'pocas_muestras'
  | 'quedaria_demasiado_corto'
  | 'quedarian_pocas_muestras';

/**
 * De dónde salen las muestras que han entrado en el cálculo.
 *
 * `filas_originales` son las filas de datos del CSV (sin cabecera ni metadatos) y se
 * reparten enteras entre inválidas, excluidas por calentamiento y analizadas: ninguna
 * fila se cuenta dos veces.
 */
export interface ContabilidadDeMuestras {
  /** Filas de datos del CSV: no cuentan metadatos ni cabecera. */
  filas_originales: number;
  /** Filas con FPS y frametime utilizables. */
  filas_con_rendimiento_valido: number;
  /** Filas con FPS o frametime vacíos, no numéricos, negativos, cero o infinitos. */
  filas_con_rendimiento_invalido: number;
  /** De las válidas, las del calentamiento descartado. */
  excluidas_por_calentamiento: number;
  /** Las que llegan a las fórmulas. Es el `n` real de los percentiles. */
  muestras_analizadas: number;
  incidencias: IncidenciasTemporales;
  /** Si hay un eje temporal en el que se pueda confiar. */
  eje_temporal_fiable: boolean;
  /** La columna `elapsed` existía en la cabecera del CSV. */
  columna_elapsed_presente: boolean;
  /** Se pidió calentamiento y no se aplicó. Ausente si se aplicó o no se pidió. */
  calentamiento_no_aplicado?: MotivoSinCalentamiento | null;
}

/**
 * Cómo se calculó un percentil concreto.
 *
 * **No es un tamaño efectivo estadístico**: describe el cálculo, nada más. En una
 * grabación corta, el «0,1 % low» puede apoyarse en una cola nominal menor que una
 * muestra y en dos observaciones interpoladas.
 */
export interface ApoyoDePercentil {
  /** Muestras sobre las que se calculó (`n`). */
  muestras: number;
  /** Tamaño nominal de la cola (`n × (1−p)`), en unidades de muestra. */
  cola_nominal: number;
  /** Observaciones distintas que usó la interpolación del percentil. */
  observaciones_interpoladas: number;
  /** Muestras incluidas de verdad en la media de cola. Ausente si es un percentil puro. */
  muestras_en_la_media?: number | null;
}

/** Apoyo real de cada percentil del benchmark. */
export interface ApoyoDeMetricas {
  fps_1_percent_low?: ApoyoDePercentil | null;
  fps_0_1_percent_low?: ApoyoDePercentil | null;
  max_fps?: ApoyoDePercentil | null;
  min_fps?: ApoyoDePercentil | null;
  frame_time_1_percent_high?: ApoyoDePercentil | null;
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

// ==================== Sesiones de ejecución (subfase 5.3) ====================

/**
 * ¿Se pudo ver el proceso del juego?
 *
 * `'sin_visibilidad'` **no** significa que el juego no estuviera corriendo: significa que no
 * se pudo mirar (contenedor, otro usuario, `hidepid`). La interfaz nunca debe presentarlo
 * como lo segundo.
 */
export type EvidenciaProceso =
  | 'observada'
  | 'sin_visibilidad'
  | 'sin_permisos'
  | 'terminada'
  | 'no_observada';

/** De dónde salió el AppID de una sesión. */
export type OrigenAppId =
  | 'entorno_del_proceso'
  | 'ejecutable'
  | 'directorio_de_instalacion'
  | 'inferido_por_nombre'
  | 'confimado_por_usuario'
  | 'desconocido';

export interface IdentidadDeSesion {
  appid: number | null;
  origen: OrigenAppId;
  candidatos: number[];
  nombre_observado: string | null;
  evidencia: string[];
}

export interface ProcesoObservado {
  pid: number;
  inicio_del_proceso: string | null;
  primera_observacion: string | null;
  ejecutable: string | null;
  appid_del_entorno: { steam_app_id: string | null; steam_game_id: string | null } | null;
  bajo_proton: boolean | null;
  hijo_de_steam: boolean | null;
}

export interface ConfiguracionCapturada {
  comando_confirmado: ConProcedencia<string> | null;
  perfil: ConProcedencia<string> | null;
  revision_motor: number | null;
  opciones_steam: ConProcedencia<string> | null;
  capturado_en: string;
  /**
   * `true` si la foto se tomó **después** de que arrancara el juego. Entonces describe lo
   * que había puesto más tarde, no necesariamente lo que se usó al arrancar.
   */
  posterior_al_arranque: boolean;
}

export interface SesionDeEjecucion {
  id: string;
  tipo: 'observada' | 'agrupada_sin_proceso';
  identidad: IdentidadDeSesion;
  proceso: ProcesoObservado | null;
  evidencia_proceso: EvidenciaProceso;
  configuracion_inicial: ConfiguracionCapturada | null;
  grabaciones: string[];
  abierta_en: string;
  ultima_observacion: string | null;
  cerrada_en: string | null;
}

/** Qué está haciendo el observador de procesos. */
/** Un juego que el observador ve ahora mismo. */
export interface JuegoObservado {
  /** Clave del proceso observado (`pid@inicio`). */
  clave: string;
  /** AppID leído del propio proceso, si lo llevaba. */
  appid: number | null;
  /**
   * Nombre **comercial**, resuelto en Rust por AppID contra la biblioteca de Steam.
   * `null` = no se pudo resolver: se dice «identidad desconocida».
   */
  nombre: string | null;
}

export interface EstadoObservador {
  activo: boolean;
  estado: 'apagado' | 'sin_visibilidad' | 'observando' | 'buscando';
  /** Juegos observados ahora, con su nombre. Vacío si no se está observando. */
  juegos: JuegoObservado[];
  /** Solo `true` con evidencia real de que el juego no corre. En Docker es `false`. */
  puede_afirmar_que_no_corre: boolean;
  pasadas: number;
  coste_ms: number | null;
  error: string | null;
}
