use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Benchmark {
    pub id: String,
    pub profile_id: String,
    pub game_name: String,
    pub appid: u32,
    pub timestamp: String,
    pub duration_seconds: f64,
    pub avg_fps: f64,
    pub fps_1_percent_low: f64,
    pub fps_0_1_percent_low: f64,
    pub max_fps: f64,
    pub min_fps: f64,
    pub frame_time_avg: f64,
    pub frame_time_1_percent_high: f64,
    pub total_frames: u64,
    pub notes: Option<String>,
    pub launch_command: String,
    // Métricas de fluidez. Son None en los benchmarks guardados antes de esta versión
    // (por eso #[serde(default)]: los benchmarks viejos se siguen cargando sin problema).
    #[serde(default)]
    pub spikes_per_min: Option<f64>,
    #[serde(default)]
    pub pct_over_33ms: Option<f64>,
    #[serde(default)]
    pub jitter_ms: Option<f64>,
    #[serde(default)]
    pub worst_frame_ms: Option<f64>,
    /// Segundos iniciales que se descartaron (calentamiento). `None` en benchmarks
    /// guardados antes de existir esta métrica.
    #[serde(default)]
    pub warmup_seconds: Option<f64>,
    /// Margen de ruido de esta grabación: cuánto puede moverse el FPS promedio solo por
    /// variación natural (95 % de confianza), en %. Sirve para no llamar "mejora" a una
    /// diferencia que está dentro del ruido.
    #[serde(default)]
    pub noise_percent: Option<f64>,
    /// Los primeros segundos de la grabación (antes del calentamiento) frente al resto:
    /// ahí se ve el coste de compilar shaders la primera vez. `None` si no hay datos
    /// suficientes para compararlo.
    #[serde(default)]
    pub shader_stutter: Option<ShaderStutter>,
}

/// Coste del arranque: lo que se pierde en los primeros segundos de la grabación,
/// cuando los shaders todavía se están compilando.
///
/// Es la diferencia entre la primera ventana y el resto de la grabación, medida en el
/// **peor 1 %** de frames. Se usa el peor 1 % y no el FPS promedio porque la compilación
/// de shaders no baja el promedio: provoca tirones puntuales, que es justo lo que el
/// promedio esconde.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderStutter {
    /// Segundos comparados al principio (0 = no se pudo comparar).
    pub window_seconds: f64,
    /// Peor 1 % de FPS en esa ventana inicial.
    pub early_fps_1_percent_low: f64,
    /// Peor 1 % de FPS en el resto de la grabación.
    pub steady_fps_1_percent_low: f64,
    /// % de frames por encima de 33 ms (menos de 30 FPS) en la ventana inicial.
    pub early_pct_over_33ms: f64,
    /// Lo mismo en el resto de la grabación.
    pub steady_pct_over_33ms: f64,
    /// Cuánto peor es el arranque que el resto, en % sobre el peor 1 %. Se queda en 0
    /// si el arranque no es peor (un juego que precarga puede ir mejor al principio).
    pub stutter_penalty_percent: f64,
}

/// Segundos que se descartan al principio de cada grabación por defecto.
/// Incluye la carga del nivel y la compilación inicial de shaders, que no representan
/// el rendimiento en juego y ensucian sobre todo los percentiles bajos.
pub const DEFAULT_WARMUP_SECONDS: f64 = 10.0;

/// Si tras descartar el calentamiento quedan menos segundos que esto, no se descarta
/// nada: quedarían tan pocas muestras que el resultado sería peor que no filtrar.
const MIN_STEADY_SECONDS: f64 = 20.0;

/// Mínimo de muestras para que el análisis tenga sentido.
const MIN_SAMPLES: usize = 100;

/// Margen mínimo de ruido que se asume entre dos grabaciones del mismo juego (% FPS).
/// Por debajo de esto no se puede afirmar nada aunque la estadística diga otra cosa.
const MIN_NOISE: f64 = 3.0;

/// Segundos iniciales que se comparan contra el resto para medir el coste de compilar
/// shaders. Coincide con el calentamiento por defecto: es exactamente el trozo que se
/// descarta, así que se enseña lo que se está tirando en vez de esconderlo.
pub const SHADER_WINDOW_SECONDS: f64 = DEFAULT_WARMUP_SECONDS;

/// Muestras mínimas en cada lado para poder comparar arranque y resto.
const MIN_SHADER_SAMPLES: usize = 50;

/// Por debajo de este empeoramiento (%) el arranque no se considera un problema de
/// shaders. El peor 1 % es una medida ruidosa, así que el listón es alto.
///
/// Lo consume la interfaz, no este módulo: `stutter_penalty_percent` se calcula aquí y
/// lo compara `SIGNIFICANT_PENALTY` en `src/lib/shaders.ts`. La constante existe para que
/// el test `el_umbral_coincide_con_la_interfaz` compruebe que los dos lados no se
/// separan, y por eso lleva `#[allow(dead_code)]`: en compilación normal nadie la lee,
/// pero borrarla dejaría el umbral duplicado a ciegas en TypeScript.
#[allow(dead_code)]
pub const SIGNIFICANT_SHADER_PENALTY: f64 = 20.0;

/// Grabaciones que se conservan de cada juego. Al guardar la número 11, la más antigua
/// de ese juego se borra sola; las de los demás juegos no se tocan.
pub const MAX_BENCHMARKS_PER_GAME: usize = 10;

/// Métricas que miden los "tirones" (stutter) a partir de los frame times (en ms).
struct StutterStats {
    spikes_per_min: f64,
    pct_over_33ms: f64,
    jitter_ms: f64,
    worst_frame_ms: f64,
}

/// Percentil con interpolación lineal sobre un vector YA ORDENADO de menor a mayor.
/// `p` va de 0 a 100. Es más estable que promediar "el peor 1%" cuando hay pocas muestras
/// (MangoHud guarda ~10 filas por segundo, no un frame por fila).
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (p / 100.0).clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
}

/// Una fila válida del log de MangoHud.
struct Sample {
    fps: f64,
    frame_time: f64,
    elapsed_ns: Option<f64>,
}

/// MangoHud repite el último valor cuando el juego no entrega frames nuevos
/// (pantalla de carga, alt-tab, pausa). Esas filas "pegadas" no son frames reales y
/// distorsionan el promedio, así que se descartan las repetidas consecutivas.
/// Con 6 decimales es prácticamente imposible que dos muestras reales sean idénticas.
fn drop_stuck_samples(samples: Vec<Sample>) -> (Vec<Sample>, usize) {
    let total = samples.len();
    let mut cleaned: Vec<Sample> = Vec::with_capacity(total);

    for s in samples {
        let is_repeat = cleaned
            .last()
            .map(|prev| prev.fps == s.fps && prev.frame_time == s.frame_time)
            .unwrap_or(false);
        if !is_repeat {
            cleaned.push(s);
        }
    }

    let removed = total - cleaned.len();
    (cleaned, removed)
}

/// Descarta las muestras de los primeros `seconds` segundos de la grabación.
///
/// Devuelve las muestras que quedan y los segundos descartados de verdad. No descarta
/// nada (y devuelve 0) cuando no hay columna `elapsed` —sin reloj real no se sabe dónde
/// cae el corte—, cuando quedarían menos de [`MIN_STEADY_SECONDS`] segundos, o cuando
/// quedarían menos de [`MIN_SAMPLES`] muestras.
fn split_warmup(samples: Vec<Sample>, seconds: f64) -> (Vec<Sample>, f64) {
    if seconds <= 0.0 || samples.len() < MIN_SAMPLES {
        return (samples, 0.0);
    }

    let (Some(first), Some(last)) = (
        samples.first().and_then(|s| s.elapsed_ns),
        samples.last().and_then(|s| s.elapsed_ns),
    ) else {
        return (samples, 0.0);
    };

    let total = (last - first) / 1e9;
    // Si el corte deja menos de lo exigible, es mejor analizar todo que analizar un
    // trozo diminuto y presentarlo como si fuera representativo.
    if total - seconds < MIN_STEADY_SECONDS {
        return (samples, 0.0);
    }

    let cutoff = first + seconds * 1e9;
    let cut = samples
        .iter()
        .position(|s| s.elapsed_ns.map(|e| e >= cutoff).unwrap_or(false));

    let Some(cut) = cut else {
        return (samples, 0.0);
    };
    if cut == 0 || samples.len() - cut < MIN_SAMPLES {
        return (samples, 0.0);
    }

    let applied = (samples[cut].elapsed_ns.unwrap_or(cutoff) - first) / 1e9;
    let kept = samples.into_iter().skip(cut).collect();
    (kept, applied)
}

/// Parte las muestras en (primera ventana, resto).
///
/// La ventana se elige por tiempo real (`elapsed_ns`) y se corta por índice, así que
/// los dos tramos son disjuntos y un mismo frame nunca cuenta en los dos. Si no hay
/// reloj (`elapsed_ns` ausente, logs antiguos) o la ventana no deja frames en un lado,
/// devuelve dos listas vacías: sin datos fiables es mejor no decir nada.
fn sample_window(samples: &[Sample], seconds: f64) -> (Vec<f64>, Vec<f64>) {
    let empty = (Vec::new(), Vec::new());
    if seconds <= 0.0 || samples.len() < MIN_SHADER_SAMPLES * 2 {
        return empty;
    }
    let Some(first) = samples.first().and_then(|s| s.elapsed_ns) else {
        return empty;
    };
    let cutoff = first + seconds * 1e9;
    let Some(cut) = samples
        .iter()
        .position(|s| s.elapsed_ns.map(|e| e >= cutoff).unwrap_or(false))
    else {
        return empty;
    };
    if cut < MIN_SHADER_SAMPLES || samples.len() - cut < MIN_SHADER_SAMPLES {
        return empty;
    }
    let early: Vec<f64> = samples[..cut].iter().map(|s| s.frame_time).collect();
    let steady: Vec<f64> = samples[cut..].iter().map(|s| s.frame_time).collect();
    (early, steady)
}

/// Peor 1 % de FPS de un tramo de frames (o `None` si hay muy pocos).
fn worst_one_percent_fps(frame_times: &[f64]) -> Option<f64> {
    if frame_times.len() < MIN_SHADER_SAMPLES {
        return None;
    }
    let mut sorted = frame_times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p99 = percentile_sorted(&sorted, 99.0);
    if p99 <= 0.0 {
        return None;
    }
    Some(1000.0 / p99)
}

/// % de frames de un tramo que tardaron más de 33 ms (bajada por debajo de 30 FPS).
fn pct_over_33ms(frame_times: &[f64]) -> f64 {
    if frame_times.is_empty() {
        return 0.0;
    }
    let n = frame_times.iter().filter(|&&f| f > 33.3).count();
    (n as f64 / frame_times.len() as f64) * 100.0
}

/// Compara los primeros `window_seconds` de la grabación con el resto.
///
/// Por qué existe: el calentamiento (`DEFAULT_WARMUP_SECONDS`) se descarta para que el
/// análisis no salga contaminado por la compilación inicial de shaders. Pero descartarlo
/// en silencio oculta información útil: si el primer minuto de cada partida va a tirones,
/// el jugador lo sufre igual y no sabe por qué. Aquí se mide ese coste y se enseña.
///
/// Se trabaja con los frame times del tramo inicial ANTES de descartar el calentamiento,
/// así que las muestras tienen que llegar ya separadas: esta función recibe las dos
/// mitades hechas.
///
/// Devuelve `None` si algún lado tiene menos de [`MIN_SHADER_SAMPLES`] frames: con tan
/// poco no se puede afirmar nada sobre el arranque.
pub fn shader_stutter(early: &[f64], steady: &[f64], window_seconds: f64) -> Option<ShaderStutter> {
    if window_seconds <= 0.0 {
        return None;
    }
    let early_low = worst_one_percent_fps(early)?;
    let steady_low = worst_one_percent_fps(steady)?;

    // Penalización en % sobre el peor 1 %. Si el arranque va MEJOR que el resto no se
    // informa de una "mejora" (no tiene sentido: no has optimizado nada), se deja en 0.
    let penalty = if steady_low > 0.0 && early_low < steady_low {
        ((steady_low - early_low) / steady_low) * 100.0
    } else {
        0.0
    };

    Some(ShaderStutter {
        window_seconds,
        early_fps_1_percent_low: early_low,
        steady_fps_1_percent_low: steady_low,
        early_pct_over_33ms: pct_over_33ms(early),
        steady_pct_over_33ms: pct_over_33ms(steady),
        stutter_penalty_percent: penalty,
    })
}

/// Margen de ruido del FPS promedio de una grabación, en % (95 % de confianza).
///
/// La grabación se parte en bloques contiguos, se calcula la media de cada bloque y se
/// estima el error estándar **entre bloques**. Así la autocorrelación de los frame times
/// (un tirón afecta a varias filas seguidas) no se cuenta como si fueran muestras
/// independientes, que es el error de usar el desvío estándar por muestra.
///
/// `None` si hay muy pocas muestras para estimarlo con un mínimo de rigor.
fn noise_percent(fps: &[f64]) -> Option<f64> {
    let n = fps.len();
    if n < MIN_SAMPLES * 2 {
        return None;
    }

    // Bloques de ~60 filas (~6 s con MangoHud guardando cada 100 ms), entre 5 y 30.
    let blocks = (n / 60).clamp(5, 30);
    let size = n / blocks;
    if size < 20 {
        return None;
    }

    let means: Vec<f64> = (0..blocks)
        .map(|b| {
            let chunk = &fps[b * size..(b + 1) * size];
            chunk.iter().sum::<f64>() / chunk.len() as f64
        })
        .collect();

    let mean = means.iter().sum::<f64>() / means.len() as f64;
    if !mean.is_finite() || mean <= 0.0 {
        return None;
    }

    let var = means.iter().map(|m| (m - mean) * (m - mean)).sum::<f64>()
        / (means.len() as f64 - 1.0);
    let se = (var / means.len() as f64).sqrt();

    Some((1.96 * se / mean) * 100.0)
}

/// Margen de ruido combinado de dos grabaciones, en % sobre el FPS promedio.
/// Los errores de las dos medidas se suman en cuadratura (son independientes).
///
/// Si falta el dato de alguna de las dos, esa parte no aporta: el margen no puede
/// inventarse más incertidumbre de la que hay. Con una sola grabación medida el margen
/// es el de esa grabación, no `√2` veces mayor; con ninguna, el mínimo de la casa.
fn combined_noise(a: Option<f64>, b: Option<f64>) -> f64 {
    let a = a.filter(|v| v.is_finite() && *v > 0.0);
    let b = b.filter(|v| v.is_finite() && *v > 0.0);

    let combined = match (a, b) {
        (Some(a), Some(b)) => (a * a + b * b).sqrt(),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => MIN_NOISE,
    };

    combined.max(MIN_NOISE)
}

fn compute_stutter(frame_times: &[f64], duration_seconds: f64) -> StutterStats {
    let n = frame_times.len();

    let mut sorted = frame_times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[n / 2];

    // Un "pico" es un frame que tarda más del doble de lo normal (y al menos 25 ms,
    // para no contar ruido cuando el juego va a muchos FPS).
    let spike_threshold = (median * 2.0).max(25.0);
    let spikes = frame_times.iter().filter(|&&f| f > spike_threshold).count();

    // Frames que tardaron más de 33.3 ms = momentos por debajo de 30 FPS.
    let over_33 = frame_times.iter().filter(|&&f| f > 33.3).count();

    // Jitter: cuánto cambia el tiempo entre un frame y el siguiente.
    let jitter_ms = frame_times
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .sum::<f64>()
        / (n.saturating_sub(1).max(1)) as f64;

    // Peor frame (percentil 99.9): más estable que usar el máximo absoluto.
    let idx = (((n as f64) * 0.999) as usize).min(n - 1);

    StutterStats {
        spikes_per_min: if duration_seconds > 0.0 {
            spikes as f64 / (duration_seconds / 60.0)
        } else {
            0.0
        },
        pct_over_33ms: over_33 as f64 / n as f64 * 100.0,
        jitter_ms,
        worst_frame_ms: sorted[idx],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub baseline: Benchmark,
    pub test: Benchmark,
    pub fps_change_percent: f64,
    pub fps_1_percent_change_percent: f64,
    pub fps_0_1_percent_change_percent: f64,
    pub frame_time_change_percent: f64,
    pub conclusion: String,
    /// Margen de ruido combinado de las dos grabaciones (%). Una diferencia menor que
    /// esto no se puede distinguir de la variación normal del juego.
    #[serde(default)]
    pub noise_percent: Option<f64>,
    /// `true` cuando el cambio de FPS promedio entra dentro del margen de ruido.
    #[serde(default)]
    pub within_noise: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarksData {
    pub benchmarks: Vec<Benchmark>,
}

fn get_benchmarks_dir() -> std::path::PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("thoriapp")
        .join("benchmarks");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).ok();
    }

    config_dir
}

fn get_benchmarks_file() -> std::path::PathBuf {
    get_benchmarks_dir().join("benchmarks.json")
}

fn load_benchmarks_data() -> BenchmarksData {
    let file = get_benchmarks_file();

    if file.exists() {
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(data) = serde_json::from_str(&content) {
                return data;
            }
        }
    }

    BenchmarksData {
        benchmarks: Vec::new(),
    }
}

fn save_benchmarks_data(data: &BenchmarksData) -> Result<(), String> {
    let file = get_benchmarks_file();
    let content =
        serde_json::to_string_pretty(data).map_err(|e| format!("Error al serializar: {}", e))?;

    fs::write(&file, content).map_err(|e| format!("Error al guardar: {}", e))?;

    Ok(())
}

/// Lee como máximo `max_bytes` del inicio de un archivo (como texto, tolerando bytes raros).
fn read_head(path: &Path, max_bytes: usize) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; max_bytes];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// MangoHud crea además un `<nombre>_summary.csv` con estadísticas ya calculadas.
/// No sirve para el análisis frame a frame, así que se ignora.
fn is_summary_file(path: &Path) -> bool {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_lowercase().ends_with("_summary"))
        .unwrap_or(false)
}

fn find_latest_mangohud_log(dir: &Path) -> Result<std::path::PathBuf, String> {
    let mut log_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();

    fn search_dir(dir: &Path, log_files: &mut Vec<(std::path::PathBuf, std::time::SystemTime)>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    search_dir(&path, log_files);
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if (ext_str == "csv" || ext_str == "log") && !is_summary_file(&path) {
                            // Solo se lee el inicio del archivo: basta para ver el header
                            // y evita cargar en memoria logs enormes de otras aplicaciones.
                            if let Ok(content) = read_head(&path, 4096) {
                                let has_valid_format = content.lines().any(|line| {
                                    let lower = line.to_lowercase();
                                    lower.contains("fps,frametime")
                                        || (lower.starts_with("fps,")
                                            && lower.contains("frametime"))
                                });

                                if has_valid_format {
                                    if let Ok(metadata) = path.metadata() {
                                        if let Ok(modified) = metadata.modified() {
                                            println!(
                                                "[Benchmark] Archivo MangoHud encontrado: {:?}",
                                                path
                                            );
                                            log_files.push((path, modified));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    search_dir(dir, &mut log_files);

    if log_files.is_empty() {
        return Err(format!(
            "No se encontraron archivos de log de MangoHud válidos en {:?}.\n\n\
            Asegúrate de:\n\
            1. Tener MangoHud instalado y configurado\n\
            2. Haber grabado un log con Shift+F2 durante al menos 30 segundos\n\
            3. Que los logs se guarden en esta carpeta",
            dir
        ));
    }

    log_files.sort_by_key(|f| std::cmp::Reverse(f.1));

    let latest = &log_files[0].0;
    println!("[Benchmark] Usando el log más reciente: {:?}", latest);

    Ok(latest.clone())
}

fn extract_game_name_from_file(file_path: &Path) -> String {
    let file_name = match file_path.file_stem() {
        Some(name) => name.to_string_lossy().to_string(),
        None => return "Desconocido".to_string(),
    };

    println!("[Extract] Nombre original del archivo: {}", file_name);

    let mut name = file_name.clone();

    // PASO 1: Buscar patrón de fecha/hora y cortar ahí
    if let Ok(datetime_pattern) =
        Regex::new(r"[_-]\d{4}[-_]\d{2}[-_]\d{2}([_-]\d{2}[-_]\d{2}[-_]\d{2})?")
    {
        if let Some(mat) = datetime_pattern.find(&name) {
            name = name[..mat.start()].to_string();
            println!("[Extract] Después de cortar fecha/hora: '{}'", name);
        }
    }

    // PASO 2: Quitar sufijos técnicos comunes de juegos
    let technical_suffixes = [
        "-Win64-Shipping",
        "-Win32-Shipping",
        "-Shipping",
        "-Win64",
        "-Win32",
        "-x64",
        "-X64",
        "-Linux64",
        "-Linux",
        "-Mac",
        "-MacOS",
        "-Server",
        "-DedicatedServer",
        "-Client",
        "-Editor",
        "-Test",
        "-Debug",
        "-Release",
    ];

    let name_lower = name.to_lowercase();
    for suffix in &technical_suffixes {
        let suffix_lower = suffix.to_lowercase();
        if name_lower.ends_with(&suffix_lower) {
            name = name[..name.len() - suffix.len()].to_string();
            println!(
                "[Extract] Después de quitar sufijo '{}': '{}'",
                suffix, name
            );
            break;
        }
    }

    // PASO 3: Quitar prefijos comunes de MangoHud
    let prefixes_to_remove = ["mangohud", "mango", "hud", "log", "logs", "mg"];
    for prefix in &prefixes_to_remove {
        let name_lower = name.to_lowercase();
        if name_lower.starts_with(prefix) {
            let rest = name[prefix.len()..].to_string();
            let trimmed = rest.trim_start_matches(['_', '-', ' ']);
            if !trimmed.is_empty() {
                name = trimmed.to_string();
                println!(
                    "[Extract] Después de quitar prefijo '{}': '{}'",
                    prefix, name
                );
                break;
            }
        }
    }

    // PASO 4: Limpiar separadores finales y espacios extra
    name = name
        .trim_end_matches(['_', '-', ' '])
        .to_string();
    name = name
        .trim_start_matches(['_', '-', ' '])
        .to_string();

    // PASO 5: Si quedó vacío, usar el nombre original del archivo
    if name.is_empty() {
        println!("[Extract] Nombre vacío, usando original: {}", file_name);
        return file_name;
    }

    println!("[Extract] ✅ Nombre final del juego: {}", name);
    name
}

fn get_file_modified_time(file_path: &Path) -> String {
    match fs::metadata(file_path) {
        Ok(metadata) => match metadata.modified() {
            Ok(system_time) => {
                let datetime: chrono::DateTime<chrono::Local> = system_time.into();
                let formatted = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
                println!("[Time] ✅ Fecha de modificación del archivo: {}", formatted);
                formatted
            }
            Err(e) => {
                println!(
                    "[Time] ⚠️ Error obteniendo hora de modificación: {}, usando hora actual",
                    e
                );
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
            }
        },
        Err(e) => {
            println!("[Time] ⚠️ Error obteniendo meta {}, usando hora actual", e);
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
        }
    }
}

pub fn analyze_mangohud_log(log_path: &str, warmup_seconds: f64) -> Result<Benchmark, String> {
    let path = Path::new(log_path);

    if !path.exists() {
        return Err(format!("La ruta no existe: {}", log_path));
    }

    let file_path = if path.is_dir() {
        find_latest_mangohud_log(path)?
    } else {
        path.to_path_buf()
    };

    println!("[Benchmark] ============================================");
    println!("[Benchmark] Analizando archivo: {:?}", file_path);

    let game_name = extract_game_name_from_file(&file_path);
    let file_timestamp = get_file_modified_time(&file_path);

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Error al leer el archivo {:?}: {}", file_path, e))?;

    let mut raw_samples: Vec<Sample> = Vec::new();

    let mut fps_column: usize = 0;
    let mut frametime_column: usize = 1;
    let mut elapsed_column: Option<usize> = None;
    let mut data_start_line: usize = 0;

    for (line_idx, line) in content.lines().enumerate() {
        let lower = line.to_lowercase();

        if lower.contains("fps") && lower.contains("frametime") {
            let headers: Vec<&str> = line.split(',').collect();

            for (i, header) in headers.iter().enumerate() {
                let h = header.trim().to_lowercase();
                if h == "fps" {
                    fps_column = i;
                } else if h == "frametime" {
                    frametime_column = i;
                } else if h == "elapsed" {
                    elapsed_column = Some(i);
                }
            }

            data_start_line = line_idx + 1;
            println!(
                "[Benchmark] Header de datos encontrado en línea {}",
                line_idx + 1
            );
            println!(
                "[Benchmark] Columnas: fps={}, frametime={}, elapsed={:?}",
                fps_column, frametime_column, elapsed_column
            );
            break;
        }
    }

    if data_start_line == 0 {
        return Err("No se encontró el header de datos (fps,frametime) en el archivo CSV. ¿Es un log válido de MangoHud?".to_string());
    }

    for line in content.lines().skip(data_start_line) {
        let parts: Vec<&str> = line.split(',').collect();

        if parts.len() > fps_column && parts.len() > frametime_column {
            if let (Ok(fps), Ok(frame_time)) = (
                parts[fps_column].trim().parse::<f64>(),
                parts[frametime_column].trim().parse::<f64>(),
            ) {
                // is_finite descarta NaN/inf, que romperían los ordenamientos más abajo
                if fps.is_finite() && frame_time.is_finite() && fps > 0.0 && frame_time > 0.0 {
                    let elapsed_ns = elapsed_column
                        .and_then(|c| parts.get(c))
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .filter(|v| v.is_finite());

                    raw_samples.push(Sample {
                        fps,
                        frame_time,
                        elapsed_ns,
                    });
                }
            }
        }
    }

    // La duración se mide con el reloj real de la grabación (columna `elapsed`, en
    // nanosegundos) ANTES de limpiar muestras repetidas, para no acortarla.
    // MangoHud guarda ~1 fila cada 100 ms, así que sumar los frametimes (lo que se hacía
    // antes) daba ~7 s en una grabación de 60 s.
    let elapsed_only: Vec<f64> = raw_samples.iter().filter_map(|s| s.elapsed_ns).collect();
    let duration_seconds = if elapsed_only.len() >= 2 {
        let first = elapsed_only[0];
        let last = elapsed_only[elapsed_only.len() - 1];
        ((last - first) / 1e9).max(0.0)
    } else {
        // Respaldo para logs sin columna `elapsed`
        raw_samples.iter().map(|s| s.frame_time).sum::<f64>() / 1000.0
    };

    let (samples, stuck_removed) = drop_stuck_samples(raw_samples);

    println!(
        "[Benchmark] Muestras válidas: {} (descartadas {} repetidas/pegadas)",
        samples.len(),
        stuck_removed
    );
    println!("[Benchmark] Duración real de la grabación: {:.1} s", duration_seconds);

    // La ventana inicial se guarda ANTES de descartar el calentamiento: es justo el
    // trozo que se tira, y compararlo con el resto es lo que dice cuánto cuesta compilar
    // shaders la primera vez. La ventana busca su propio corte por tiempo sobre las
    // muestras completas, así que las dos mitades salen disjuntas por construcción y el
    // trozo que se compara como "arranque" es el mismo que luego se descarta.
    let (early_frames, steady_frames) = sample_window(&samples, SHADER_WINDOW_SECONDS);

    // El arranque (carga del nivel, compilación inicial de shaders) no representa el
    // rendimiento en juego y estropea sobre todo los percentiles bajos.
    let (samples, warmup_applied) = split_warmup(samples, warmup_seconds);
    if warmup_applied > 0.0 {
        println!(
            "[Benchmark] Calentamiento descartado: {:.1} s (quedan {:.1} s de estado estable)",
            warmup_applied,
            (duration_seconds - warmup_applied).max(0.0)
        );
    }

    if samples.len() < MIN_SAMPLES {
        return Err(format!(
            "El log tiene muy pocas muestras ({} útiles, mínimo {} requeridas).\n\n\
            Graba el log durante al menos 30 segundos (lo ideal es 60).",
            samples.len(),
            MIN_SAMPLES
        ));
    }

    let fps_values: Vec<f64> = samples.iter().map(|s| s.fps).collect();
    let frame_times: Vec<f64> = samples.iter().map(|s| s.frame_time).collect();

    // Las muestras están espaciadas de forma uniforme en el tiempo, así que el promedio
    // simple de FPS equivale a (frames totales / tiempo total).
    let total_frames = fps_values.len() as u64;
    let avg_fps = fps_values.iter().sum::<f64>() / fps_values.len() as f64;
    let frame_time_avg = frame_times.iter().sum::<f64>() / frame_times.len() as f64;

    // Máximo y mínimo "absolutos" son muy sensibles a una sola muestra rara (por ejemplo
    // una fila de 8 FPS por un tirón puntual). Se usan los percentiles 99,9 y 0,1 de FPS.
    let mut sorted_fps = fps_values.clone();
    sorted_fps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let max_fps = percentile_sorted(&sorted_fps, 99.9);
    let min_fps = percentile_sorted(&sorted_fps, 0.1);

    // Los "lows" se calculan sobre el frametime (como hacen CapFrameX y similares):
    // 1% low = FPS equivalentes al percentil 99 del frametime.
    // Con ~600 muestras, promediar "el peor 1%" serían solo ~6 filas y un valor extremo
    // lo arrastraría; el percentil con interpolación es mucho más estable.
    let mut sorted_ft = frame_times.clone();
    sorted_ft.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p99_ft = percentile_sorted(&sorted_ft, 99.0);
    let p999_ft = percentile_sorted(&sorted_ft, 99.9);
    let fps_1_percent_low = 1000.0 / p99_ft;
    let fps_0_1_percent_low = 1000.0 / p999_ft;

    // Promedio de los frametimes que superan el percentil 99 (el "1% más lento").
    let worst: Vec<f64> = sorted_ft.iter().cloned().filter(|&f| f >= p99_ft).collect();
    let frame_time_1_percent_high = if worst.is_empty() {
        p99_ft
    } else {
        worst.iter().sum::<f64>() / worst.len() as f64
    };

    let steady_duration = (duration_seconds - warmup_applied).max(0.0);
    let stutter = compute_stutter(&frame_times, steady_duration);
    let noise = noise_percent(&fps_values);
    let shader = shader_stutter(&early_frames, &steady_frames, SHADER_WINDOW_SECONDS);

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

    println!("[Benchmark] ✅ Análisis completado:");
    println!("[Benchmark]    Juego: {}", game_name);
    println!("[Benchmark]    Fecha del log: {}", file_timestamp);
    println!("[Benchmark]    FPS promedio: {:.1}", avg_fps);
    println!("[Benchmark]    1% Low: {:.1}", fps_1_percent_low);
    println!("[Benchmark]    0.1% Low: {:.1}", fps_0_1_percent_low);
    println!("[Benchmark]    Picos/min: {:.1}", stutter.spikes_per_min);
    if let Some(n) = noise {
        println!("[Benchmark]    Margen de ruido: ±{:.1} %", n);
    }
    if let Some(sh) = &shader {
        println!(
            "[Benchmark]    Arranque ({}s): peor 1% {:.0} FPS vs {:.0} FPS del resto ({:+.1} %)",
            sh.window_seconds,
            sh.early_fps_1_percent_low,
            sh.steady_fps_1_percent_low,
            -sh.stutter_penalty_percent
        );
    }
    println!("[Benchmark] ============================================");

    Ok(Benchmark {
        id: format!("bench_{}", now.replace([':', ' ', '-', '.'], "")),
        profile_id: String::new(),
        game_name,
        appid: 0,
        timestamp: file_timestamp,
        duration_seconds,
        avg_fps,
        fps_1_percent_low,
        fps_0_1_percent_low,
        max_fps,
        min_fps,
        frame_time_avg,
        frame_time_1_percent_high,
        total_frames,
        notes: None,
        launch_command: String::new(),
        spikes_per_min: Some(stutter.spikes_per_min),
        pct_over_33ms: Some(stutter.pct_over_33ms),
        jitter_ms: Some(stutter.jitter_ms),
        worst_frame_ms: Some(stutter.worst_frame_ms),
        warmup_seconds: Some(warmup_applied),
        noise_percent: noise,
        shader_stutter: shader,
    })
}

/// Nombre de juego normalizado para compararlo (minúsculas y sin extensión de ejecutable).
fn norm_game_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    for suffix in [".x86_64", ".x86", ".exe", ".bin"] {
        if let Some(stripped) = lower.strip_suffix(suffix) {
            return stripped.trim().to_string();
        }
    }
    lower
}

/// Clave de agrupación del historial: el appid si lo hay, el nombre normalizado si no.
fn game_key(b: &Benchmark) -> String {
    if b.appid != 0 {
        format!("appid:{}", b.appid)
    } else {
        format!("name:{}", norm_game_name(&b.game_name))
    }
}

/// Recorta el historial a las `MAX_BENCHMARKS_PER_GAME` grabaciones más recientes de
/// cada juego, descartando las más antiguas.
///
/// Es por juego, no global: guardar en un juego no borra las pruebas de otro, así que
/// puedes volver a un juego de hace semanas y seguir comparando. Dentro de un juego el
/// exceso se va por la cola según `timestamp` (formato ISO, que ordena bien como texto).
/// Si dos grabaciones comparten fecha, gana la que está más al final de la lista, que es
/// la última que se añadió. Conserva el orden original de lo que sobrevive.
fn trim_history(list: &mut Vec<Benchmark>) {
    let mut keep: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, b) in list.iter().enumerate() {
        keep.entry(game_key(b)).or_default().push(i);
    }

    let mut drop = vec![false; list.len()];
    for indices in keep.values() {
        if indices.len() <= MAX_BENCHMARKS_PER_GAME {
            continue;
        }
        // Índices a conservar: los últimos por fecha (y, a igualdad, los últimos en la
        // lista). `sort_by` es estable, así que el desempate es el orden de inserción.
        let mut ordered = indices.clone();
        ordered.sort_by(|&a, &b| list[a].timestamp.cmp(&list[b].timestamp));
        let excess = ordered.len() - MAX_BENCHMARKS_PER_GAME;
        for &i in &ordered[..excess] {
            drop[i] = true;
        }
    }

    let mut i = 0;
    list.retain(|_| {
        let keep_it = !drop[i];
        i += 1;
        keep_it
    });
}

#[tauri::command]
pub fn save_benchmark(
    mut benchmark: Benchmark,
    profile_id: String,
    game_name: String,
    appid: u32,
    launch_command: String,
    notes: Option<String>,
) -> Result<Benchmark, String> {
    let mut data = load_benchmarks_data();

    benchmark.profile_id = profile_id;

    if (benchmark.game_name.is_empty() || benchmark.game_name == "Desconocido")
        && !game_name.is_empty()
        && game_name != "Desconocido"
    {
        benchmark.game_name = game_name;
    }

    benchmark.appid = appid;
    benchmark.launch_command = launch_command;
    benchmark.notes = notes;

    // Historial por juego: se añade la grabación nueva y se conservan las
    // `MAX_BENCHMARKS_PER_GAME` más recientes de ESTE juego; las más viejas se van solas.
    // Los demás juegos no se tocan, y como el archivo entero se guarda, el recorte también
    // limpia lo que quedara de versiones anteriores.
    //
    // Aquí NO se borran las grabaciones anteriores del mismo juego: el recorte es cosa de
    // `trim_history`. Un `retain` por clave de juego en este punto dejaría una sola
    // grabación por juego y el historial de 10 no llegaría a existir.
    data.benchmarks.push(benchmark.clone());
    trim_history(&mut data.benchmarks);
    save_benchmarks_data(&data)?;

    Ok(benchmark)
}

#[tauri::command]
pub fn load_benchmarks() -> Result<Vec<Benchmark>, String> {
    let data = load_benchmarks_data();
    let mut list = data.benchmarks;
    // El recorte se aplica al leer y no solo al guardar: un archivo antiguo con más de
    // 10 grabaciones por juego se limpia la próxima vez que se cargue.
    trim_history(&mut list);
    Ok(list)
}

#[tauri::command]
pub fn load_benchmarks_for_game(appid: u32) -> Result<Vec<Benchmark>, String> {
    let data = load_benchmarks_data();
    let mut benchmarks: Vec<Benchmark> = data
        .benchmarks
        .into_iter()
        .filter(|b| b.appid == appid)
        .collect();
    trim_history(&mut benchmarks);
    Ok(benchmarks)
}

#[tauri::command]
pub fn delete_benchmark(benchmark_id: String) -> Result<(), String> {
    let mut data = load_benchmarks_data();
    data.benchmarks.retain(|b| b.id != benchmark_id);
    save_benchmarks_data(&data)?;
    Ok(())
}

// NUEVO: Comando para borrar todos los benchmarks
#[tauri::command]
pub fn delete_all_benchmarks() -> Result<(), String> {
    let mut data = load_benchmarks_data();
    let count = data.benchmarks.len();
    data.benchmarks.clear();
    save_benchmarks_data(&data)?;
    println!("[Benchmark] ✅ Se eliminaron {} benchmarks", count);
    Ok(())
}

#[tauri::command]
pub fn compare_benchmarks(
    baseline_id: String,
    test_id: String,
) -> Result<BenchmarkComparison, String> {
    let data = load_benchmarks_data();

    let baseline = data
        .benchmarks
        .iter()
        .find(|b| b.id == baseline_id)
        .ok_or("Benchmark base no encontrado")?
        .clone();

    let test = data
        .benchmarks
        .iter()
        .find(|b| b.id == test_id)
        .ok_or("Benchmark de prueba no encontrado")?
        .clone();

    // Cambio porcentual de `b` respecto a `a`. Si `a` es 0 (dato inválido) devuelve 0
    // en vez de infinito o NaN.
    fn pct_change(a: f64, b: f64) -> f64 {
        if a.abs() < f64::EPSILON {
            0.0
        } else {
            (b - a) / a * 100.0
        }
    }

    let fps_change_percent = pct_change(baseline.avg_fps, test.avg_fps);
    let fps_1_percent_change_percent =
        pct_change(baseline.fps_1_percent_low, test.fps_1_percent_low);
    let fps_0_1_percent_change_percent =
        pct_change(baseline.fps_0_1_percent_low, test.fps_0_1_percent_low);
    let frame_time_change_percent = pct_change(baseline.frame_time_avg, test.frame_time_avg);

    // Entre dos grabaciones del mismo juego en las mismas condiciones, el propio ruido
    // se estima a partir de las propias grabaciones (variación entre bloques). El mínimo
    // es MIN_NOISE: por debajo de eso nunca se afirma una mejora.
    let noise = combined_noise(baseline.noise_percent, test.noise_percent);
    let within_noise = fps_change_percent.abs() <= noise;

    let mut conclusion = if within_noise {
        format!(
            "🟡 Sin diferencia clara ({:+.1}% FPS promedio, dentro del margen de ruido ±{:.1}%). Repite la prueba antes de decidir.",
            fps_change_percent, noise
        )
    } else if fps_change_percent > noise && fps_1_percent_change_percent < -noise {
        format!(
            "⚠️ Más FPS promedio (+{:.1}%) pero los peores momentos empeoran ({:.1}%): puede haber más tirones",
            fps_change_percent, fps_1_percent_change_percent
        )
    } else if fps_change_percent < -noise && fps_1_percent_change_percent > noise {
        format!(
            "⚠️ Menos FPS promedio ({:.1}%) pero los peores momentos mejoran (+{:.1}%): el juego puede sentirse más parejo",
            fps_change_percent, fps_1_percent_change_percent
        )
    } else if fps_change_percent > noise {
        format!(
            "✅ Mejora: +{:.1}% FPS promedio (por encima del ruido, ±{:.1}%)",
            fps_change_percent, noise
        )
    } else {
        format!(
            "❌ Rendimiento degradado: {:.1}% FPS promedio (por encima del ruido, ±{:.1}%)",
            fps_change_percent, noise
        )
    };

    // Avisos que ayudan a no sacar conclusiones erróneas
    if baseline.game_name != test.game_name {
        conclusion.push_str(&format!(
            "\nℹ️ Son juegos distintos ({} vs {}): la comparación no es válida.",
            baseline.game_name, test.game_name
        ));
    }
    if baseline.duration_seconds < 20.0 || test.duration_seconds < 20.0 {
        conclusion.push_str(
            "\nℹ️ Alguna grabación dura menos de 20 s: el resultado puede no ser fiable.",
        );
    }

    Ok(BenchmarkComparison {
        baseline,
        test,
        fps_change_percent,
        fps_1_percent_change_percent,
        fps_0_1_percent_change_percent,
        frame_time_change_percent,
        conclusion,
        noise_percent: Some(noise),
        within_noise,
    })
}

#[tauri::command]
pub fn analyze_mangohud_log_command(
    log_path: String,
    warmup_seconds: Option<f64>,
) -> Result<Benchmark, String> {
    analyze_mangohud_log(
        &log_path,
        warmup_seconds.unwrap_or(DEFAULT_WARMUP_SECONDS),
    )
}

/// Lee el valor de una clave de MangoHud.conf (`clave=valor` o `clave = valor`).
/// Ignora comentarios (#) y toma la ÚLTIMA aparición, igual que MangoHud.
fn read_conf_value(content: &str, key: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                found = Some(v.trim().to_string());
            }
        }
    }
    found
}

/// Expande `~` al directorio personal.
fn expand_home(path: &str, home: &str) -> String {
    if path == "~" {
        home.to_string()
    } else if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{}", home, rest)
    } else {
        path.to_string()
    }
}

#[tauri::command]
pub fn detect_mangohud_log_path() -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    // 1) Lo más fiable: la carpeta que el propio usuario configuró en MangoHud.
    for config_path in mangohud_conf_paths() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Some(folder) = read_conf_value(&content, "output_folder") {
                let folder = expand_home(&folder, &home);
                if !folder.is_empty() && Path::new(&folder).is_dir() {
                    println!("[Detect] Ruta desde config: {}", folder);
                    return Ok(folder);
                }
            }
        }
    }

    // 2) Carpetas habituales que ya contengan logs de MangoHud
    let possible_paths = vec![
        format!("{}/.local/share/goverlay", home),
        format!("{}/.local/share/goverlay/logs", home),
        format!("{}/.local/share/MangoHud", home),
        format!("{}/.local/share/MangoHud/logs", home),
        format!("{}/MangoHud", home),
        format!("{}/.config/MangoHud", home),
        format!("{}/.config/goverlay", home),
        "/tmp/MangoHud".to_string(),
        format!(
            "{}/.var/app/com.valvesoftware.Steam/.local/share/goverlay",
            home
        ),
        format!(
            "{}/.var/app/com.valvesoftware.Steam/.local/share/MangoHud",
            home
        ),
    ];

    for path in &possible_paths {
        let p = Path::new(path);
        if p.is_dir() && dir_has_mangohud_log(p) {
            println!("[Detect] Encontrados logs de MangoHud en: {}", path);
            return Ok(path.clone());
        }
    }

    // 3) Último recurso: carpeta por defecto (aunque aún esté vacía)
    let goverlay_path = format!("{}/.local/share/goverlay", home);
    if Path::new(&goverlay_path).exists() {
        return Ok(goverlay_path);
    }

    Ok("/tmp/MangoHud".to_string())
}

/// ¿Hay en esta carpeta (sin entrar en subcarpetas) algún CSV con formato de log de MangoHud?
/// Se comprueba el header para no confundirlo con un CSV cualquiera de otra aplicación.
fn dir_has_mangohud_log(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let is_csv = p
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase() == "csv")
                .unwrap_or(false);
            if is_csv && !is_summary_file(&p) {
                if let Ok(head) = read_head(&p, 4096) {
                    let lower = head.to_lowercase();
                    if lower.contains("fps,frametime") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[tauri::command]
pub fn check_mangohud_installed() -> Result<bool, String> {
    // 1) Lo que de verdad decide si MangoHud funciona: el binario (wrapper) o la capa
    // Vulkan. Antes solo se miraba `which mangohud`, y quien lo tiene instalado como
    // capa (lo normal: el binario existe, pero el que actúa es la capa) o dentro de
    // Steam Flatpak recibía un "no está instalado" falso.
    let runtime = crate::preflight::get_runtime_env();
    if runtime.mangohud_binary.is_some() || runtime.mangohud_vulkan_layer.is_some() {
        return Ok(true);
    }

    // 2) Configuración escrita por MangoHud o Goverlay: indicio de que está instalado.
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_paths = vec![
        format!("{}/.config/MangoHud/MangoHud.conf", home),
        format!("{}/.config/goverlay/MangoHud.conf", home),
    ];

    for config_path in config_paths {
        if Path::new(&config_path).exists() {
            return Ok(true);
        }
    }

    // 3) Logs ya grabados: si hay un CSV de MangoHud, está instalado.
    let log_paths = vec![
        format!("{}/.local/share/goverlay/logs", home),
        format!("{}/.local/share/goverlay", home),
        format!("{}/.local/share/MangoHud/logs", home),
        "/tmp/MangoHud".to_string(),
    ];

    for log_path in log_paths {
        if Path::new(&log_path).exists() {
            if let Ok(entries) = fs::read_dir(&log_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

// ───────────────────────── Duración de la grabación de MangoHud ─────────────────────────

fn mangohud_conf_paths() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    vec![
        std::path::PathBuf::from(format!("{}/.config/MangoHud/MangoHud.conf", home)),
        std::path::PathBuf::from(format!("{}/.config/goverlay/MangoHud.conf", home)),
    ]
}

/// Devuelve la duración (en segundos) que MangoHud graba con Shift+F2.
/// Devuelve 0 si no hay límite configurado o no se encuentra el archivo.
#[tauri::command]
pub fn get_mangohud_log_duration() -> Result<u32, String> {
    for path in mangohud_conf_paths() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(value) = read_conf_value(&content, "log_duration") {
                if let Ok(n) = value.parse::<u32>() {
                    return Ok(n);
                }
            }
        }
    }
    Ok(0)
}

/// Cambia `log_duration` en MangoHud.conf (y en el de GOverlay si existe).
/// Solo toca esa línea; el resto de la configuración queda igual.
#[tauri::command]
pub fn set_mangohud_log_duration(seconds: u32) -> Result<u32, String> {
    if !(5..=600).contains(&seconds) {
        return Err("La duración debe estar entre 5 segundos y 10 minutos.".to_string());
    }

    let mut updated = 0;

    for path in mangohud_conf_paths() {
        if !path.exists() {
            continue;
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("No se pudo leer {:?}: {}", path, e))?;

        // Copia de seguridad (solo la primera vez)
        let backup = path.with_extension("conf.thoriapp.bak");
        if !backup.exists() {
            fs::write(&backup, &content).ok();
        }

        let new_line = format!("log_duration={}", seconds);
        let mut found = false;
        let mut lines: Vec<String> = content
            .lines()
            .map(|line| {
                let is_duration_line = !line.trim().starts_with('#')
                    && line
                        .split_once('=')
                        .map(|(k, _)| k.trim() == "log_duration")
                        .unwrap_or(false);
                if is_duration_line {
                    found = true;
                    new_line.clone()
                } else {
                    line.to_string()
                }
            })
            .collect();

        if !found {
            lines.push(new_line);
        }

        let mut output = lines.join("\n");
        output.push('\n');

        fs::write(&path, output)
            .map_err(|e| format!("No se pudo guardar {:?}: {}", path, e))?;

        println!("[MangoHud] log_duration={} guardado en {:?}", seconds, path);
        updated += 1;
    }

    if updated == 0 {
        return Err(
            "No encontré tu MangoHud.conf. Abre GOverlay una vez y guarda tu configuración."
                .to_string(),
        );
    }

    Ok(seconds)
}

// ───────────────────────── Pruebas guardadas ("Mis pruebas") ─────────────────────────
//
// Cuando el usuario pulsa "Guardar en Mis pruebas" se guarda una COPIA completa de la
// comparación (Antes + Después + resultado). Así no se pierde aunque después se grabe
// otra prueba y el Antes/Después de la pantalla Benchmarks se reemplace.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedComparison {
    pub id: String,
    /// Cuándo se pulsó "Guardar" (la fecha de la prueba es `comparison.test.timestamp`).
    pub saved_at: String,
    pub game_name: String,
    pub before_fps: f64,
    pub after_fps: f64,
    pub fps_change_percent: f64,
    pub comparison: BenchmarkComparison,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedComparisonsData {
    comparisons: Vec<SavedComparison>,
}

fn get_saved_comparisons_file() -> std::path::PathBuf {
    get_benchmarks_dir().join("saved_comparisons.json")
}

fn load_saved_data() -> SavedComparisonsData {
    let file = get_saved_comparisons_file();

    if file.exists() {
        if let Ok(content) = fs::read_to_string(&file) {
            match serde_json::from_str::<SavedComparisonsData>(&content) {
                Ok(data) => return data,
                Err(e) => {
                    // El archivo existe pero no se pudo leer: se guarda una copia antes de
                    // que un guardado posterior lo sobrescriba con una lista vacía.
                    println!("[Pruebas] ⚠️ saved_comparisons.json ilegible: {}", e);
                    let backup = file.with_extension("json.corrupt.bak");
                    fs::write(&backup, &content).ok();
                }
            }
        }
    }

    SavedComparisonsData {
        comparisons: Vec::new(),
    }
}

fn save_saved_data(data: &SavedComparisonsData) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(data).map_err(|e| format!("Error al serializar: {}", e))?;
    fs::write(get_saved_comparisons_file(), content)
        .map_err(|e| format!("Error al guardar: {}", e))
}

/// Guarda en "Mis pruebas" la comparación entre dos benchmarks (copia completa).
#[tauri::command]
pub fn save_comparison(baseline_id: String, test_id: String) -> Result<SavedComparison, String> {
    let comparison = compare_benchmarks(baseline_id, test_id)?;
    let mut data = load_saved_data();

    let already_saved = data.comparisons.iter().any(|c| {
        c.comparison.baseline.id == comparison.baseline.id
            && c.comparison.test.id == comparison.test.id
    });
    if already_saved {
        return Err("Esta comparación ya está guardada en Mis pruebas.".to_string());
    }

    let now = chrono::Local::now();
    let game_name = if comparison.test.game_name.is_empty() {
        comparison.baseline.game_name.clone()
    } else {
        comparison.test.game_name.clone()
    };
    let before_fps = comparison.baseline.avg_fps;
    let after_fps = comparison.test.avg_fps;
    let fps_change_percent = comparison.fps_change_percent;

    let saved = SavedComparison {
        id: format!("cmp_{}", now.format("%Y%m%d%H%M%S%3f")),
        saved_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        game_name,
        before_fps,
        after_fps,
        fps_change_percent,
        comparison,
    };

    data.comparisons.push(saved.clone());
    save_saved_data(&data)?;

    println!("[Pruebas] ✅ Comparación guardada: {}", saved.game_name);
    Ok(saved)
}

#[tauri::command]
pub fn load_comparisons() -> Result<Vec<SavedComparison>, String> {
    Ok(load_saved_data().comparisons)
}

#[tauri::command]
pub fn delete_comparison(comparison_id: String) -> Result<(), String> {
    let mut data = load_saved_data();
    data.comparisons.retain(|c| c.id != comparison_id);
    save_saved_data(&data)
}

#[tauri::command]
pub fn delete_all_comparisons() -> Result<(), String> {
    let data = SavedComparisonsData {
        comparisons: Vec::new(),
    };
    save_saved_data(&data)
}

// ==================== TESTS ====================
// Solo cubren la lógica pura: no tocan disco ni el runtime de Tauri.

#[cfg(test)]
mod tests {
    use super::*;

    fn bench(appid: u32, name: &str) -> Benchmark {
        Benchmark {
            id: format!("{}-{}", appid, name),
            profile_id: String::new(),
            game_name: name.to_string(),
            appid,
            timestamp: String::new(),
            duration_seconds: 60.0,
            avg_fps: 60.0,
            fps_1_percent_low: 30.0,
            fps_0_1_percent_low: 20.0,
            max_fps: 120.0,
            min_fps: 10.0,
            frame_time_avg: 16.6,
            frame_time_1_percent_high: 33.0,
            total_frames: 3600,
            notes: None,
            launch_command: "%command%".to_string(),
            spikes_per_min: None,
            pct_over_33ms: None,
            jitter_ms: None,
            worst_frame_ms: None,
            warmup_seconds: None,
            noise_percent: None,
            shader_stutter: None,
        }
    }

    fn sample(fps: f64, ft: f64) -> Sample {
        Sample { fps, frame_time: ft, elapsed_ns: None }
    }

    #[test]
    fn percentile_interpola_entre_valores_vecinos() {
        let v = [0.0, 10.0, 20.0, 30.0];
        assert_eq!(percentile_sorted(&v, 0.0), 0.0);
        assert_eq!(percentile_sorted(&v, 100.0), 30.0);
        // rank = 0.5 * 3 = 1.5 → entre 10 y 20
        assert_eq!(percentile_sorted(&v, 50.0), 15.0);
    }

    #[test]
    fn percentile_con_un_solo_valor_lo_devuelve() {
        assert_eq!(percentile_sorted(&[42.0], 99.0), 42.0);
    }

    #[test]
    fn percentile_con_lista_vacia_da_cero_sin_panico() {
        assert_eq!(percentile_sorted(&[], 50.0), 0.0);
    }

    #[test]
    fn percentile_satura_fuera_de_rango() {
        let v = [1.0, 2.0, 3.0];
        assert_eq!(percentile_sorted(&v, -10.0), 1.0);
        assert_eq!(percentile_sorted(&v, 500.0), 3.0);
    }

    #[test]
    fn descarta_muestras_repetidas_consecutivas() {
        let (out, removed) = drop_stuck_samples(vec![
            sample(60.0, 16.6),
            sample(60.0, 16.6), // repetida
            sample(59.0, 16.9),
            sample(59.0, 16.9), // repetida
            sample(60.0, 16.6), // no es consecutiva: se conserva
        ]);
        assert_eq!(removed, 2);
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].fps, 60.0);
    }

    #[test]
    fn sin_repetidas_no_descarta_nada() {
        let (out, removed) = drop_stuck_samples(vec![sample(60.0, 16.6), sample(70.0, 14.2)]);
        assert_eq!(removed, 0);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn stutter_detecta_picos_y_frames_lentos() {
        // 8 frames normales de 16 ms y 2 picos de 100 ms.
        let mut ft = vec![16.0; 8];
        ft.push(100.0);
        ft.push(100.0);
        let s = compute_stutter(&ft, 60.0);

        // mediana 16 → umbral max(32, 25) = 32 → los dos de 100 son picos
        assert_eq!(s.spikes_per_min, 2.0); // 2 picos en 1 minuto
        assert!((s.pct_over_33ms - 20.0).abs() < 1e-9); // 2 de 10
        assert!(s.jitter_ms > 0.0);
        assert!((s.worst_frame_ms - 100.0).abs() < 1e-9);
    }

    #[test]
    fn stutter_con_frame_times_uniformes_no_hay_jitter() {
        let ft = vec![16.0; 60];
        let s = compute_stutter(&ft, 60.0);
        assert_eq!(s.spikes_per_min, 0.0);
        assert_eq!(s.pct_over_33ms, 0.0);
        assert_eq!(s.jitter_ms, 0.0);
    }

    #[test]
    fn stutter_con_duracion_cero_no_divide_por_cero() {
        let s = compute_stutter(&[16.0, 200.0], 0.0);
        assert_eq!(s.spikes_per_min, 0.0);
    }

    #[test]
    fn same_game_cae_al_nombre_si_falta_appid_via_game_key() {
        // Un juego de Lutris no tiene appid (0): se compara el nombre, sin
        // distinguir mayúsculas ni espacios sobrantes. Esto antes lo resolvía una
        // función `same_game` que ya no existe: `game_key` la sustituyó y es la
        // única que se usa de verdad, así que la cobertura vive aquí.
        assert_eq!(
            game_key(&bench(0, "Hollow Knight")),
            game_key(&bench(0, "hollow knight "))
        );
        assert_ne!(
            game_key(&bench(0, "Hollow Knight")),
            game_key(&bench(0, "Celeste"))
        );
        // Y con appid manda el appid, aunque el nombre no coincida.
        assert_eq!(
            game_key(&bench(440, "TF2")),
            game_key(&bench(440, "Otro nombre"))
        );
        assert_ne!(game_key(&bench(440, "TF2")), game_key(&bench(730, "CS2")));
    }

    /// Benchmark con fecha conocida, para probar el recorte por antigüedad.
    ///
    /// El `id` incluye la fecha a propósito: dos grabaciones del mismo juego nunca
    /// comparten id en la app real (lleva la hora con milisegundos), y un test que las
    /// hiciera chocar estaría probando algo que no puede pasar.
    fn bench_at(appid: u32, name: &str, day: u32) -> Benchmark {
        let mut b = bench(appid, name);
        b.timestamp = format!("2026-01-{:02} 10:00:00", day);
        b.id = format!("{}-{}", b.id, day);
        b
    }

    #[test]
    fn trim_history_conserva_las_diez_mas_recientes() {
        // Se añaden en orden, y se recorta: deben quedar las 10 últimas (días 6..15).
        let mut list: Vec<Benchmark> = (1..=15).map(|d| bench_at(440, "TF2", d)).collect();
        trim_history(&mut list);

        assert_eq!(list.len(), MAX_BENCHMARKS_PER_GAME);
        assert_eq!(list.first().unwrap().timestamp, "2026-01-06 10:00:00");
        assert_eq!(list.last().unwrap().timestamp, "2026-01-15 10:00:00");
        // El orden original se respeta: no se reordena el archivo.
        assert!(list.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
    }

    #[test]
    fn trim_history_recorta_por_fecha_y_no_por_orden_de_lista() {
        // Añadida la última pero con fecha vieja: es la que debe caer.
        let mut list: Vec<Benchmark> = (1..=MAX_BENCHMARKS_PER_GAME as u32)
            .map(|d| bench_at(440, "TF2", d + 10))
            .collect();
        list.push(bench_at(440, "TF2", 1));

        trim_history(&mut list);
        assert_eq!(list.len(), MAX_BENCHMARKS_PER_GAME);
        assert!(!list.iter().any(|b| b.timestamp == "2026-01-01 10:00:00"));
    }

    #[test]
    fn trim_history_no_toca_los_demas_juegos() {
        // Un juego no se come el historial de otro, aunque comparta nombre.
        let mut list: Vec<Benchmark> = (1..=MAX_BENCHMARKS_PER_GAME as u32 + 5)
            .map(|d| bench_at(440, "TF2", d))
            .collect();
        list.extend((1..=3).map(|d| bench_at(730, "CS2", d)));

        trim_history(&mut list);
        assert_eq!(list.iter().filter(|b| b.appid == 440).count(), MAX_BENCHMARKS_PER_GAME);
        assert_eq!(list.iter().filter(|b| b.appid == 730).count(), 3);
    }

    #[test]
    fn trim_history_separa_juegos_sin_appid_por_nombre() {
        let mut list: Vec<Benchmark> = (1..=MAX_BENCHMARKS_PER_GAME as u32 + 2)
            .map(|d| bench_at(0, "Hollow Knight", d))
            .collect();
        list.push(bench_at(0, "Celeste", 99));

        trim_history(&mut list);
        assert_eq!(list.len(), MAX_BENCHMARKS_PER_GAME + 1);
        assert!(list.iter().any(|b| b.game_name == "Celeste"));
    }

    #[test]
    fn guardar_un_juego_no_borra_las_pruebas_de_otro() {
        // La política anterior borraba todo al cambiar de juego; ahora el historial es
        // por juego, así que se puede volver a un juego antiguo y seguir comparando.
        let mut data = vec![
            bench_at(440, "TF2", 1),
            bench_at(440, "TF2", 2),
            bench_at(730, "CS2", 3),
        ];

        // Llega una grabación nueva de TF2: se añade al historial sin tocar lo anterior
        // ni la de CS2. La política anterior borraba todo al cambiar de juego.
        data.push(bench_at(440, "TF2", 9));
        trim_history(&mut data);

        assert_eq!(data.iter().filter(|b| b.appid == 440).count(), 3);
        assert_eq!(data.iter().filter(|b| b.appid == 730).count(), 1);
        // Las tres de TF2 siguen ahí, en orden de guardado, y CS2 intacto.
        let tf2: Vec<_> = data.iter().filter(|b| b.appid == 440).collect();
        assert_eq!(tf2[0].timestamp, "2026-01-01 10:00:00");
        assert_eq!(tf2[1].timestamp, "2026-01-02 10:00:00");
        assert_eq!(tf2[2].timestamp, "2026-01-09 10:00:00");
    }

    #[test]
    fn trim_history_no_hace_nada_si_no_sobra_nada() {
        let mut list: Vec<Benchmark> = vec![bench_at(440, "TF2", 1), bench_at(440, "TF2", 2)];
        trim_history(&mut list);
        assert_eq!(list.len(), 2);

        let mut empty: Vec<Benchmark> = vec![];
        trim_history(&mut empty);
        assert!(empty.is_empty());
    }

    /// Frame times con reloj real: `n` filas cada 100 ms, con el frametime indicado.
    fn timed_frames(counts: &[(usize, f64)]) -> Vec<Sample> {
        let mut out = Vec::new();
        let mut i = 0usize;
        for (n, ft) in counts {
            for _ in 0..*n {
                out.push(Sample {
                    fps: 1000.0 / ft,
                    frame_time: *ft,
                    elapsed_ns: Some(i as f64 * 1e8),
                });
                i += 1;
            }
        }
        out
    }

    #[test]
    fn sample_window_parte_en_dos_tramos_disjuntos() {
        // 600 muestras = 60 s. Ventana de 10 s = 100 muestras.
        let samples = timed_frames(&[(600, 16.0)]);
        let (early, steady) = sample_window(&samples, 10.0);
        assert_eq!(early.len(), 100);
        assert_eq!(steady.len(), 500);
        assert_eq!(early.len() + steady.len(), samples.len());
    }

    #[test]
    fn sample_window_sin_reloj_no_inventa_nada() {
        // Log antiguo sin columna `elapsed`: imposible saber dónde está el corte.
        let samples: Vec<Sample> = (0..600)
            .map(|_| Sample { fps: 60.0, frame_time: 16.0, elapsed_ns: None })
            .collect();
        let (early, steady) = sample_window(&samples, 10.0);
        assert!(early.is_empty());
        assert!(steady.is_empty());
    }

    #[test]
    fn sample_window_exige_muestras_en_los_dos_lados() {
        // Grabación de 5 s con ventana de 10: no queda resto con el que comparar.
        let samples = timed_frames(&[(50, 16.0)]);
        let (early, steady) = sample_window(&samples, 10.0);
        assert!(early.is_empty());
        assert!(steady.is_empty());
    }

    #[test]
    fn arranque_con_tirones_se_detecta_por_el_peor_uno_por_ciento() {
        // El arranque tiene los mismos FPS medios que el resto, pero con tirones
        // puntuales: es exactamente el caso que el promedio esconde.
        let mut early = vec![16.0; 2000];
        for i in (0..2000).step_by(10) {
            early[i] = 80.0; // 12,5 FPS: tirón de compilación de shaders
        }
        let steady = vec![16.0; 2000];

        let sh = shader_stutter(&early, &steady, 10.0).expect("debería medirse");
        assert!(
            sh.stutter_penalty_percent >= SIGNIFICANT_SHADER_PENALTY,
            "penalización: {}",
            sh.stutter_penalty_percent
        );
        assert!(sh.early_pct_over_33ms > sh.steady_pct_over_33ms);
    }

    #[test]
    fn arranque_igual_al_resto_no_es_un_problema() {
        let steady = vec![16.0; 2000];
        let sh = shader_stutter(&steady, &steady, 10.0).unwrap();
        assert_eq!(sh.stutter_penalty_percent, 0.0);
    }

    #[test]
    fn arranque_mejor_que_el_resto_no_se_presenta_como_mejora() {
        // Si el principio va mejor (precarga), no se informa de penalización negativa.
        let early = vec![10.0; 2000];
        let steady = vec![16.0; 2000];
        let sh = shader_stutter(&early, &steady, 10.0).unwrap();
        assert_eq!(sh.stutter_penalty_percent, 0.0);
    }

    #[test]
    fn sin_muestras_suficientes_no_se_opina_sobre_shaders() {
        assert!(shader_stutter(&[16.0; 10], &[16.0; 2000], 10.0).is_none());
        assert!(shader_stutter(&[16.0; 2000], &[16.0; 10], 10.0).is_none());
        assert!(shader_stutter(&[16.0; 2000], &[16.0; 2000], 0.0).is_none());
    }

    #[test]
    fn benchmark_viejo_sin_datos_de_shaders_se_sigue_cargando() {
        // Compatibilidad: los benchmarks guardados antes de esta mejora no tienen el
        // campo `shader_stutter` y deben deserializar sin error.
        let json = r#"{"id":"x","profile_id":"","game_name":"TF2","appid":440,"timestamp":"",
            "duration_seconds":60.0,"avg_fps":60.0,"fps_1_percent_low":30.0,
            "fps_0_1_percent_low":20.0,"max_fps":120.0,"min_fps":10.0,
            "frame_time_avg":16.6,"frame_time_1_percent_high":33.0,"total_frames":3600,
            "notes":null,"launch_command":"%command%"}"#;
        let b: Benchmark = serde_json::from_str(json).unwrap();
        assert!(b.shader_stutter.is_none());
        assert!(b.noise_percent.is_none());
    }

    #[test]
    fn el_umbral_coincide_con_el_de_la_interfaz() {
        // `src/lib/shaders.ts` tiene su propia copia (SIGNIFICANT_PENALTY) porque la
        // decisión se toma en el frontend, donde está testeada. Si alguien cambia una y
        // no la otra, el test lo detecta.
        let ts = include_str!("../../src/lib/shaders.ts");
        // Se busca la declaración exacta (`= 20;`), no solo el número, para que un
        // cambio a 200 u 2 no pase por casualidad.
        let expected = format!("SIGNIFICANT_PENALTY = {};", SIGNIFICANT_SHADER_PENALTY);
        assert!(
            ts.contains(&expected),
            "src/lib/shaders.ts debe declarar `{}`",
            expected
        );
    }

    #[test]
    fn guardar_diez_veces_conserva_las_diez_y_no_solo_una() {
        // Recorre la misma secuencia que `save_benchmark` (push + trim_history) sin tocar
        // disco. Existía un `retain` por juego que borraba todas las grabaciones
        // anteriores del juego antes de recortar, así que el historial de 10 nunca llegaba
        // a llenarse. Este test lo habría detectado.
        let mut data: Vec<Benchmark> = Vec::new();
        for day in 1..=10u32 {
            data.push(bench_at(440, "TF2", day));
            trim_history(&mut data);
            assert_eq!(
                data.iter().filter(|b| b.appid == 440).count() as u32,
                day,
                "tras {} grabaciones deberían quedar {}",
                day,
                day
            );
        }

        // La número 11 tira la más antigua (día 1) y conserva la 2..11.
        data.push(bench_at(440, "TF2", 11));
        trim_history(&mut data);
        assert_eq!(data.len(), 10);
        assert_eq!(data[0].timestamp, "2026-01-02 10:00:00");
        assert_eq!(data[9].timestamp, "2026-01-11 10:00:00");
    }

    #[test]
    fn game_key_usa_appid_y_cae_al_nombre() {
        assert_eq!(game_key(&bench(440, "TF2")), "appid:440");
        assert_eq!(game_key(&bench(0, "Hollow Knight")), "name:hollow knight");
    }

    #[test]
    fn norm_game_name_ignora_mayusculas_y_espacios() {
        assert_eq!(norm_game_name("  Hollow Knight  "), norm_game_name("hollow knight"));
    }

    #[test]
    fn norm_game_name_quita_la_extension_del_ejecutable() {
        assert_eq!(norm_game_name("Celeste.exe"), "celeste");
        assert_eq!(norm_game_name("Game.x86_64"), "game");
        assert_eq!(norm_game_name("Game.bin"), "game");
    }

    /// Serie de muestras con reloj real: `n` filas separadas 100 ms (como MangoHud).
    fn samples_with_clock(n: usize, fps: f64) -> Vec<Sample> {
        (0..n)
            .map(|i| Sample {
                fps,
                frame_time: 1000.0 / fps,
                elapsed_ns: Some(i as f64 * 1e8),
            })
            .collect()
    }

    #[test]
    fn warmup_descarta_los_primeros_segundos() {
        // 600 filas = 60 s → descartar los primeros 10 s quita 100 filas.
        let (kept, applied) = split_warmup(samples_with_clock(600, 60.0), 10.0);
        assert_eq!(kept.len(), 500);
        assert!((applied - 10.0).abs() < 0.11);
    }

    #[test]
    fn warmup_no_se_aplica_si_queda_poco() {
        // 25 s de grabación: descartar 10 dejaría 15 s (< MIN_STEADY_SECONDS).
        let (kept, applied) = split_warmup(samples_with_clock(250, 60.0), 10.0);
        assert_eq!(kept.len(), 250);
        assert_eq!(applied, 0.0);
    }

    #[test]
    fn warmup_sin_columna_elapsed_no_hace_nada() {
        let samples: Vec<Sample> = (0..600).map(|_| sample(60.0, 16.6)).collect();
        let (kept, applied) = split_warmup(samples, 10.0);
        assert_eq!(kept.len(), 600);
        assert_eq!(applied, 0.0);
    }

    #[test]
    fn warmup_cero_no_toca_nada() {
        let (kept, applied) = split_warmup(samples_with_clock(600, 60.0), 0.0);
        assert_eq!(kept.len(), 600);
        assert_eq!(applied, 0.0);
    }

    #[test]
    fn ruido_es_cero_si_la_serie_es_constante() {
        let fps: Vec<f64> = (0..600).map(|_| 60.0).collect();
        let noise = noise_percent(&fps).expect("debería poder estimarse");
        assert!(noise < 1e-9);
    }

    #[test]
    fn ruido_crece_cuando_los_bloques_varian() {
        // 600 filas en 10 bloques: cada bloque con una media distinta (±10 FPS).
        let fps: Vec<f64> = (0..600)
            .map(|i| if (i / 60) % 2 == 0 { 50.0 } else { 70.0 })
            .collect();
        let noise = noise_percent(&fps).expect("debería poder estimarse");
        // Medias 50 y 70 con 5 bloques de cada: media 60, error estándar ≈ 3,33
        // → ±10,9 % con 95 % de confianza.
        assert!(noise > 6.0 && noise < 15.0, "ruido inesperado: {}", noise);
    }

    #[test]
    fn ruido_no_se_estima_con_muy_pocas_muestras() {
        let fps: Vec<f64> = (0..150).map(|_| 60.0).collect();
        assert!(noise_percent(&fps).is_none());
    }

    #[test]
    fn ruido_combinado_suma_en_cuadratura() {
        // 3-4-5: √(9+16) = 5
        assert!((combined_noise(Some(3.0), Some(4.0)) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn ruido_combinado_respeta_el_minimo_si_falta_el_dato() {
        // Un dato ausente o absurdo no se inventa: no aporta margen.
        assert!((combined_noise(Some(0.0), None) - MIN_NOISE).abs() < 1e-9);
        assert!((combined_noise(Some(0.1), Some(0.1)) - MIN_NOISE).abs() < 1e-9);
        assert!((combined_noise(None, None) - MIN_NOISE).abs() < 1e-9);
        // Con una sola grabación medida, el margen es el suyo (no √2 veces mayor).
        assert!((combined_noise(Some(8.0), None) - 8.0).abs() < 1e-9);
    }
}
