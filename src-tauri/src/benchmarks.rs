use regex::Regex;
use serde::{Deserialize, Serialize};
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
}

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

pub fn analyze_mangohud_log(log_path: &str) -> Result<Benchmark, String> {
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

    if samples.len() < 100 {
        return Err(format!(
            "El log tiene muy pocas muestras ({} útiles, mínimo 100 requeridas).\n\n\
            Graba el log durante al menos 30 segundos (lo ideal es 60).",
            samples.len()
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

    let stutter = compute_stutter(&frame_times, duration_seconds);

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

    println!("[Benchmark] ✅ Análisis completado:");
    println!("[Benchmark]    Juego: {}", game_name);
    println!("[Benchmark]    Fecha del log: {}", file_timestamp);
    println!("[Benchmark]    FPS promedio: {:.1}", avg_fps);
    println!("[Benchmark]    1% Low: {:.1}", fps_1_percent_low);
    println!("[Benchmark]    0.1% Low: {:.1}", fps_0_1_percent_low);
    println!("[Benchmark]    Picos/min: {:.1}", stutter.spikes_per_min);
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

/// ¿Las dos grabaciones son del mismo juego? Se usa el appid de Steam cuando ambos lo
/// tienen; si no, se compara el nombre.
fn same_game(a: &Benchmark, b: &Benchmark) -> bool {
    if a.appid != 0 && b.appid != 0 {
        a.appid == b.appid
    } else {
        norm_game_name(&a.game_name) == norm_game_name(&b.game_name)
    }
}

/// Deja solo el Antes (primero) y el Después (último). Sirve para archivos guardados
/// con versiones antiguas, que podían tener muchas grabaciones. No modifica el archivo.
fn pair_only(mut list: Vec<Benchmark>) -> Vec<Benchmark> {
    if list.len() > 2 {
        if let Some(last) = list.pop() {
            list.truncate(1);
            list.push(last);
        }
    }
    list
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

    // Política "un Antes y un Después": se guardan como máximo 2 grabaciones.
    //   [0] = Antes   (la primera, se mantiene como punto de partida)
    //   [1] = Después (la más reciente; una grabación nueva reemplaza a la anterior)
    // Si la grabación nueva es de OTRO juego, empieza una prueba nueva desde cero,
    // porque comparar juegos distintos no tiene sentido.
    if let Some(first) = data.benchmarks.first() {
        if !same_game(first, &benchmark) {
            data.benchmarks.clear();
        }
    }
    data.benchmarks.truncate(1); // también limpia archivos antiguos con más de 2
    data.benchmarks.push(benchmark.clone());
    save_benchmarks_data(&data)?;

    Ok(benchmark)
}

#[tauri::command]
pub fn load_benchmarks() -> Result<Vec<Benchmark>, String> {
    let data = load_benchmarks_data();
    Ok(pair_only(data.benchmarks))
}

#[tauri::command]
pub fn load_benchmarks_for_game(appid: u32) -> Result<Vec<Benchmark>, String> {
    let data = load_benchmarks_data();
    let benchmarks: Vec<Benchmark> = data
        .benchmarks
        .into_iter()
        .filter(|b| b.appid == appid)
        .collect();
    Ok(pair_only(benchmarks))
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

    // Entre dos grabaciones del mismo juego en las mismas condiciones, el propio ruido ronda
    // el 2-3 %. Por debajo de ese margen no se puede afirmar que haya mejora ni empeoramiento.
    const NOISE: f64 = 3.0;

    let mut conclusion = if fps_change_percent > NOISE && fps_1_percent_change_percent < -NOISE {
        format!(
            "⚠️ Más FPS promedio (+{:.1}%) pero los peores momentos empeoran ({:.1}%): puede haber más tirones",
            fps_change_percent, fps_1_percent_change_percent
        )
    } else if fps_change_percent < -NOISE && fps_1_percent_change_percent > NOISE {
        format!(
            "⚠️ Menos FPS promedio ({:.1}%) pero los peores momentos mejoran (+{:.1}%): el juego puede sentirse más parejo",
            fps_change_percent, fps_1_percent_change_percent
        )
    } else if fps_change_percent > NOISE {
        format!(
            "✅ Mejora: +{:.1}% FPS promedio",
            fps_change_percent
        )
    } else if fps_change_percent < -NOISE {
        format!(
            "❌ Rendimiento degradado: {:.1}% FPS promedio",
            fps_change_percent
        )
    } else {
        format!(
            "🟡 Sin diferencia clara ({:+.1}% FPS promedio, dentro del margen normal de ±{:.0}%)",
            fps_change_percent, NOISE
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
    })
}

#[tauri::command]
pub fn analyze_mangohud_log_command(log_path: String) -> Result<Benchmark, String> {
    analyze_mangohud_log(&log_path)
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
    use std::process::Command;

    let output = Command::new("which").arg("mangohud").output();

    if let Ok(out) = output {
        if out.status.success() {
            return Ok(true);
        }
    }

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
    fn same_game_usa_appid_si_ambos_lo_tienen() {
        assert!(same_game(&bench(440, "TF2"), &bench(440, "Otro nombre")));
        assert!(!same_game(&bench(440, "TF2"), &bench(730, "CS2")));
    }

    #[test]
    fn same_game_cae_al_nombre_si_falta_appid() {
        // Un juego de Lutris no tiene appid (0): se compara el nombre, sin
        // distinguir mayúsculas ni espacios sobrantes.
        assert!(same_game(&bench(0, "Hollow Knight"), &bench(0, "hollow knight ")));
        assert!(!same_game(&bench(0, "Hollow Knight"), &bench(0, "Celeste")));
    }

    #[test]
    fn pair_only_deja_antes_y_ultimo() {
        let list = vec![bench(1, "a"), bench(1, "b"), bench(1, "c"), bench(1, "d")];
        let out = pair_only(list);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].game_name, "a"); // el Antes original
        assert_eq!(out[1].game_name, "d"); // el Después más reciente
    }

    #[test]
    fn pair_only_no_toca_listas_de_dos_o_menos() {
        assert_eq!(pair_only(vec![bench(1, "a")]).len(), 1);
        assert_eq!(pair_only(vec![bench(1, "a"), bench(1, "b")]).len(), 2);
        assert_eq!(pair_only(vec![]).len(), 0);
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
}
