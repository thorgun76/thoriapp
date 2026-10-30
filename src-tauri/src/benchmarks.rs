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

    // Buscar recursivamente archivos .csv y .log que sean de MangoHud
    fn search_dir(dir: &Path, log_files: &mut Vec<(std::path::PathBuf, std::time::SystemTime)>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    search_dir(&path, log_files);
                } else if path.is_file() {
                    let file_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    // Solo buscar archivos que parezcan ser de MangoHud
                    let is_mangohud_file = file_name.contains("mangohud") ||
                                          file_name.ends_with(".csv");

                    if is_mangohud_file {
                        if let Some(ext) = path.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if ext_str == "csv" || ext_str == "log" {
                                // Verificar que el archivo tenga el formato correcto de MangoHud
                                if let Ok(content) = fs::read_to_string(&path) {
                                    // MangoHud CSV tiene formato: fps,frametime,...
                                    // O formato con header: fps,frametime,duration,...
                                    let has_valid_format = content.lines().any(|line| {
                                        let parts: Vec<&str> = line.split(',').collect();
                                        parts.len() >= 2 &&
                                        parts[0].trim().parse::<f64>().is_ok() &&
                                        parts[1].trim().parse::<f64>().is_ok()
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

    // Ordenar por fecha de modificación (más reciente primero)
    log_files.sort_by(|a, b| b.1.cmp(&a.1));

    let latest = &log_files[0].0;
    println!("[Benchmark] Usando el log más reciente: {:?}", latest);

    Ok(latest.clone())
}

pub fn analyze_mangohud_log(log_path: &str) -> Result<Benchmark, String> {
    let path = Path::new(log_path);

    if !path.exists() {
        return Err(format!("La ruta no existe: {}", log_path));
    }

    // Si es un directorio, buscar archivos de log de MangoHud
    let file_path = if path.is_dir() {
        find_latest_mangohud_log(path)?
    } else {
        path.to_path_buf()
    };

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Error al leer el archivo {:?}: {}", file_path, e))?;

    let mut fps_values: Vec<f64> = Vec::new();
    let mut frame_times: Vec<f64> = Vec::new();

    // Detectar si el archivo tiene header
    let mut has_header = false;
    let mut fps_column = 0;
    let mut frametime_column = 1;

    if let Some(first_line) = content.lines().next() {
        // Si la primera línea contiene texto (no números), es un header
        if first_line.split(',').any(|part| part.trim().parse::<f64>().is_err()) {
            has_header = true;

            // Buscar las columnas de fps y frametime en el header
            let headers: Vec<&str> = first_line.split(',').collect();
            for (i, header) in headers.iter().enumerate() {
                let header_lower = header.trim().to_lowercase();
                if header_lower == "fps" {
                    fps_column = i;
                } else if header_lower == "frametime" || header_lower == "frame_time" {
                    frametime_column = i;
                }
            }

            println!("[Benchmark] Header detectado. Columnas: fps={}, frametime={}", fps_column, frametime_column);
        }
    }

    // Procesar las líneas de datos
    for line in content.lines().skip(if has_header { 1 } else { 0 }) {
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

    Ok(Benchmark {
        id: format!("bench_{}", now.replace([':', ' ', '-'], "")),
        profile_id: String::new(),
        game_name: String::new(),
        appid: 0,
        timestamp: now,
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
    benchmark.game_name = game_name;
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
    // Intentar leer la configuración de MangoHud
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    // Rutas posibles para el archivo de configuración
    let config_paths = vec![
        format!("{}/.config/MangoHud/MangoHud.conf", home),
        format!("{}/.config/goverlay/MangoHud.conf", home),
    ];

    for config_path in config_paths {
        if let Ok(content) = fs::read_to_string(&config_path) {
            // Buscar la línea output_folder
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("output_folder=") {
                    let folder = line.trim_start_matches("output_folder=").trim();
                    if !folder.is_empty() {
                        // MangoHud crea una subcarpeta "logs" automáticamente
                        let logs_path = if folder.ends_with("/logs") {
                            folder.to_string()
                        } else {
                            format!("{}/logs", folder)
                        };

                        // Verificar que la carpeta existe
                        if Path::new(&logs_path).exists() {
                            return Ok(logs_path);
                        }

                        // Si no existe la subcarpeta logs, devolver la carpeta base
                        if Path::new(folder).exists() {
                            return Ok(folder.to_string());
                        }
                    }
                }
            }
        }
    }

    // Rutas por defecto si no se encuentra configuración
    let default_paths = vec![
        format!("{}/.local/share/goverlay/logs", home),
        format!("{}/.local/share/MangoHud/logs", home),
        "/tmp/MangoHud".to_string(),
    ];

    for path in default_paths {
        if Path::new(&path).exists() {
            return Ok(path);
        }
    }

    // Fallback final
    Ok("/tmp/MangoHud".to_string())
}

#[tauri::command]
pub fn check_mangohud_installed() -> Result<bool, String> {
    use std::process::Command;

    // Verificar si el binario mangohud está en el PATH
    let output = Command::new("which")
        .arg("mangohud")
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            return Ok(true);
        }
    }

    // Verificar si existe el archivo de configuración
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

    // Verificar si existen carpetas de logs de MangoHud
    let log_paths = vec![
        format!("{}/.local/share/goverlay/logs", home),
        format!("{}/.local/share/MangoHud/logs", home),
        "/tmp/MangoHud".to_string(),
    ];

    for log_path in log_paths {
        if Path::new(&log_path).exists() {
            // Verificar si hay archivos CSV dentro
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
