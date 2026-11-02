use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use regex::Regex;

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
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Error al serializar: {}", e))?;

    fs::write(&file, content)
        .map_err(|e| format!("Error al guardar: {}", e))?;

    Ok(())
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
                        if ext_str == "csv" || ext_str == "log" {
                            if let Ok(content) = fs::read_to_string(&path) {
                                let has_valid_format = content.lines().any(|line| {
                                    let lower = line.to_lowercase();
                                    lower.contains("fps,frametime") ||
                                    (lower.starts_with("fps,") && lower.contains("frametime"))
                                });

                                if has_valid_format {
                                    if let Ok(metadata) = path.metadata() {
                                        if let Ok(modified) = metadata.modified() {
                                            println!("[Benchmark] Archivo MangoHud encontrado: {:?}", path);
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

    log_files.sort_by(|a, b| b.1.cmp(&a.1));

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
    if let Ok(datetime_pattern) = Regex::new(r"[_-]\d{4}[-_]\d{2}[-_]\d{2}([_-]\d{2}[-_]\d{2}[-_]\d{2})?") {
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
            println!("[Extract] Después de quitar sufijo '{}': '{}'", suffix, name);
            break;
        }
    }

    // PASO 3: Quitar prefijos comunes de MangoHud
    let prefixes_to_remove = ["mangohud", "mango", "hud", "log", "logs", "mg"];
    for prefix in &prefixes_to_remove {
        let name_lower = name.to_lowercase();
        if name_lower.starts_with(prefix) {
            let rest = name[prefix.len()..].to_string();
            let trimmed = rest.trim_start_matches(|c: char| c == '_' || c == '-' || c == ' ');
            if !trimmed.is_empty() {
                name = trimmed.to_string();
                println!("[Extract] Después de quitar prefijo '{}': '{}'", prefix, name);
                break;
            }
        }
    }

    // PASO 4: Limpiar separadores finales y espacios extra
    name = name.trim_end_matches(|c: char| c == '_' || c == '-' || c == ' ').to_string();
    name = name.trim_start_matches(|c: char| c == '_' || c == '-' || c == ' ').to_string();

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
        Ok(metadata) => {
            match metadata.modified() {
                Ok(system_time) => {
                    let datetime: chrono::DateTime<chrono::Local> = system_time.into();
                    let formatted = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
                    println!("[Time] ✅ Fecha de modificación del archivo: {}", formatted);
                    formatted
                }
                Err(e) => {
                    println!("[Time] ⚠️ Error obteniendo hora de modificación: {}, usando hora actual", e);
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
                }
            }
        }
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

    let mut fps_values: Vec<f64> = Vec::new();
    let mut frame_times: Vec<f64> = Vec::new();

    let mut fps_column: usize = 0;
    let mut frametime_column: usize = 1;
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
                }
            }

            data_start_line = line_idx + 1;
            println!("[Benchmark] Header de datos encontrado en línea {}", line_idx + 1);
            println!("[Benchmark] Columnas: fps={}, frametime={}", fps_column, frametime_column);
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
                parts[frametime_column].trim().parse::<f64>()
            ) {
                if fps > 0.0 && frame_time > 0.0 {
                    fps_values.push(fps);
                    frame_times.push(frame_time);
                }
            }
        }
    }

    println!("[Benchmark] Frames válidos encontrados: {}", fps_values.len());

    if fps_values.len() < 100 {
        return Err(format!(
            "El log tiene muy pocos frames ({} encontrados, mínimo 100 requeridos).\n\n\
            Asegúrate de grabar el log durante al menos 30 segundos.",
            fps_values.len()
        ));
    }

    let total_frames = fps_values.len() as u64;
    let avg_fps = fps_values.iter().sum::<f64>() / fps_values.len() as f64;
    let max_fps = fps_values.iter().cloned().fold(0.0, f64::max);
    let min_fps = fps_values.iter().cloned().fold(f64::MAX, f64::min);

    let frame_time_avg = frame_times.iter().sum::<f64>() / frame_times.len() as f64;
    let duration_seconds = frame_times.iter().sum::<f64>() / 1000.0;

    let mut sorted_fps = fps_values.clone();
    sorted_fps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let one_percent_index = (sorted_fps.len() as f64 * 0.01).ceil() as usize;
    let fps_1_percent_low = sorted_fps[..one_percent_index].iter().sum::<f64>() / one_percent_index as f64;

    let zero_one_percent_index = (sorted_fps.len() as f64 * 0.001).ceil() as usize;
    let fps_0_1_percent_low = sorted_fps[..zero_one_percent_index].iter().sum::<f64>() / zero_one_percent_index as f64;

    let mut sorted_frame_times = frame_times.clone();
    sorted_frame_times.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let one_percent_ft_index = (sorted_frame_times.len() as f64 * 0.01).ceil() as usize;
    let frame_time_1_percent_high = sorted_frame_times[..one_percent_ft_index].iter().sum::<f64>() / one_percent_ft_index as f64;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    println!("[Benchmark] ✅ Análisis completado:");
    println!("[Benchmark]    Juego: {}", game_name);
    println!("[Benchmark]    Fecha del log: {}", file_timestamp);
    println!("[Benchmark]    FPS promedio: {:.1}", avg_fps);
    println!("[Benchmark]    1% Low: {:.1}", fps_1_percent_low);
    println!("[Benchmark]    0.1% Low: {:.1}", fps_0_1_percent_low);
    println!("[Benchmark] ============================================");

    Ok(Benchmark {
        id: format!("bench_{}", now.replace([':', ' ', '-'], "")),
        profile_id: String::new(),
        game_name: game_name,
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
    })
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

    if benchmark.game_name.is_empty() || benchmark.game_name == "Desconocido" {
        if !game_name.is_empty() && game_name != "Desconocido" {
            benchmark.game_name = game_name;
        }
    }

    benchmark.appid = appid;
    benchmark.launch_command = launch_command;
    benchmark.notes = notes;

    data.benchmarks.push(benchmark.clone());
    save_benchmarks_data(&data)?;

    Ok(benchmark)
}

#[tauri::command]
pub fn load_benchmarks() -> Result<Vec<Benchmark>, String> {
    let data = load_benchmarks_data();
    Ok(data.benchmarks)
}

#[tauri::command]
pub fn load_benchmarks_for_game(appid: u32) -> Result<Vec<Benchmark>, String> {
    let data = load_benchmarks_data();
    let benchmarks = data.benchmarks.into_iter()
        .filter(|b| b.appid == appid)
        .collect();
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
pub fn compare_benchmarks(baseline_id: String, test_id: String) -> Result<BenchmarkComparison, String> {
    let data = load_benchmarks_data();

    let baseline = data.benchmarks.iter()
        .find(|b| b.id == baseline_id)
        .ok_or("Benchmark base no encontrado")?
        .clone();

    let test = data.benchmarks.iter()
        .find(|b| b.id == test_id)
        .ok_or("Benchmark de prueba no encontrado")?
        .clone();

    let fps_change_percent = ((test.avg_fps - baseline.avg_fps) / baseline.avg_fps) * 100.0;
    let fps_1_percent_change_percent = ((test.fps_1_percent_low - baseline.fps_1_percent_low) / baseline.fps_1_percent_low) * 100.0;
    let fps_0_1_percent_change_percent = ((test.fps_0_1_percent_low - baseline.fps_0_1_percent_low) / baseline.fps_0_1_percent_low) * 100.0;
    let frame_time_change_percent = ((test.frame_time_avg - baseline.frame_time_avg) / baseline.frame_time_avg) * 100.0;

    let conclusion = if fps_change_percent > 5.0 {
        format!("✅ Mejora significativa: +{:.1}% FPS promedio", fps_change_percent)
    } else if fps_change_percent > 0.0 {
        format!("🟡 Mejora marginal: +{:.1}% FPS promedio", fps_change_percent)
    } else if fps_change_percent > -5.0 {
        format!("🟡 Cambio insignificante: {:.1}% FPS promedio", fps_change_percent)
    } else {
        format!("❌ Rendimiento degradado: {:.1}% FPS promedio", fps_change_percent)
    };

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

#[tauri::command]
pub fn detect_mangohud_log_path() -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    let possible_paths = vec![
        format!("{}/.local/share/goverlay", home),
        format!("{}/.local/share/goverlay/logs", home),
        format!("{}/.local/share/MangoHud", home),
        format!("{}/.local/share/MangoHud/logs", home),
        format!("{}/.config/MangoHud", home),
        format!("{}/.config/goverlay", home),
        "/tmp/MangoHud".to_string(),
        format!("{}/.var/app/com.valvesoftware.Steam/.local/share/goverlay", home),
        format!("{}/.var/app/com.valvesoftware.Steam/.local/share/MangoHud", home),
    ];

    for path in &possible_paths {
        let p = Path::new(path);
        if p.exists() && p.is_dir() {
            if let Ok(entries) = fs::read_dir(p) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if let Some(ext) = file_path.extension() {
                        if ext.to_string_lossy().to_lowercase() == "csv" {
                            println!("[Detect] Encontrados CSV en: {}", path);
                            return Ok(path.clone());
                        }
                    }
                }
            }
        }
    }

    let config_paths = vec![
        format!("{}/.config/MangoHud/MangoHud.conf", home),
        format!("{}/.config/goverlay/MangoHud.conf", home),
    ];

    for config_path in config_paths {
        if let Ok(content) = fs::read_to_string(&config_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("output_folder=") {
                    let folder = line.trim_start_matches("output_folder=").trim();
                    if !folder.is_empty() && Path::new(folder).exists() {
                        println!("[Detect] Ruta desde config: {}", folder);
                        return Ok(folder.to_string());
                    }
                }
            }
        }
    }

    let search_dirs = vec![
        format!("{}/.local/share", home),
        format!("{}/.config", home),
    ];

    for search_dir in search_dirs {
        let p = Path::new(&search_dir);
        if p.exists() {
            if let Some(found) = find_csv_in_dir_limited(p, 2) {
                println!("[Detect] Encontrado recursivamente: {}", found);
                return Ok(found);
            }
        }
    }

    let goverly_path = format!("{}/.local/share/goverlay", home);
    if Path::new(&goverly_path).exists() {
        return Ok(goverly_path);
    }

    Ok("/tmp/MangoHud".to_string())
}

fn find_csv_in_dir_limited(dir: &Path, max_depth: usize) -> Option<String> {
    if max_depth == 0 || !dir.is_dir() {
        return None;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.to_string_lossy().to_lowercase() == "csv" {
                        return path.parent().map(|p| p.to_string_lossy().to_string());
                    }
                }
            }

            if path.is_dir() {
                if let Some(found) = find_csv_in_dir_limited(&path, max_depth - 1) {
                    return Some(found);
                }
            }
        }
    }

    None
}

#[tauri::command]
pub fn check_mangohud_installed() -> Result<bool, String> {
    use std::process::Command;

    let output = Command::new("which")
        .arg("mangohud")
        .output();

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

pub fn extract_game_name_from_csv(csv_path: &str) -> Option<String> {
    let path = Path::new(csv_path);
    Some(extract_game_name_from_file(path))
}
