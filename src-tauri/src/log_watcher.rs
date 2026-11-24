//! Vigilancia de la carpeta de logs de MangoHud para recoger grabaciones solas.
//!
//! El usuario graba como siempre: pulsa Shift+F2 dentro del juego y MangoHud escribe un
//! CSV en `output_folder`. Este módulo se limita a **detectar que la grabación ha
//! terminado** para poder analizarla sin que haya que ir a buscar el fichero a mano.
//!
//! # Por qué no se usa `log_duration` ni se arranca la grabación
//!
//! Arrancar la grabación al abrir el juego mediría la carga del nivel, los logos y la
//! compilación inicial de shaders, que no es lo que se quiere comparar. El usuario
//! decide cuándo empezar (dentro de la escena) y cuándo parar, y MangoHud trata
//! `log_duration` como **máximo**, no como duración fija: pulsar Shift+F2 otra vez corta
//! antes. Por eso aquí no se programa nada, solo se escucha.
//!
//! # Cómo se sabe que una grabación terminó
//!
//! MangoHud escribe, al cerrar la grabación, un segundo fichero `<nombre>_summary.csv`
//! **1 ms después** del CSV de datos. Ese fichero es la señal fiable de cierre:
//!
//! ```text
//! Palworld-Win64-Shipping_2026-11-02_14-43-52.csv          ← datos
//! Palworld-Win64-Shipping_2026-11-02_14-43-52_summary.csv  ← señal de cierre
//! ```
//!
//! Un CSV **sin** `_summary.csv` es una grabación que no llegó a cerrarse (el juego se
//! cerró de golpe, por ejemplo). Se ignora a propósito: analizarla daría métricas de una
//! grabación truncada.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Sufijo del fichero que MangoHud escribe al cerrar una grabación.
const SUMMARY_SUFFIX: &str = "_summary.csv";
/// Extensión de los CSV de datos.
const DATA_SUFFIX: &str = ".csv";

/// Una grabación de MangoHud ya cerrada, lista para analizar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinishedLog {
    /// Ruta del CSV de datos (no del `_summary.csv`).
    pub path: String,
    /// Nombre del ejecutable tal cual lo usa MangoHud, sin extensión ni fecha.
    /// Ej.: `Palworld-Win64-Shipping`, `7DaysToDie.x86_64`.
    pub program: String,
    /// Marca de tiempo del nombre del fichero, en formato `YYYY-MM-DD_HH-MM-SS`.
    pub timestamp: String,
}

/// ¿Es `name` un CSV de datos válido? Descarta los `_summary.csv`, que son la señal y no
/// los datos.
fn is_data_file(name: &str) -> bool {
    name.ends_with(DATA_SUFFIX) && !name.ends_with(SUMMARY_SUFFIX)
}

/// Nombre del fichero de resumen que acompaña a `data_file`.
fn summary_name_of(data_file: &str) -> String {
    let stem = data_file.strip_suffix(DATA_SUFFIX).unwrap_or(data_file);
    format!("{}{}", stem, SUMMARY_SUFFIX)
}

/// Separa `Palworld-Win64-Shipping_2026-11-02_14-43-52` en programa y fecha.
///
/// El nombre del programa puede llevar puntos (`7DaysToDie.x86_64`) y guiones
/// (`Palworld-Win64-Shipping`), así que no se puede partir por el primer `_` ni por el
/// primero de cualquier cosa. La fecha tiene una forma fija y conocida
/// (`YYYY-MM-DD_HH-MM-SS`, 19 caracteres), así que se busca **desde el final**.
fn split_stem(stem: &str) -> Option<(String, String)> {
    if stem.len() < 20 {
        return None;
    }
    // El caracter que separa programa y fecha es el que está justo antes de los 19
    // caracteres de la fecha.
    let split_at = stem.len() - 19;
    let (program, rest) = stem.split_at(split_at);
    let program = program.strip_suffix('_')?;
    let timestamp = &rest[..19];

    // Se comprueba la forma de la fecha para no aceptar ficheros que solo se parezcan:
    // un `algo_2026-11-02_14-43-52` sí, un `notas_importantes_aqui.csv` no.
    let bytes = timestamp.as_bytes();
    let shape_ok = bytes.len() == 19
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'_'
        && bytes[13] == b'-'
        && bytes[16] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .iter()
            .all(|&i| bytes[i].is_ascii_digit());
    if !shape_ok || program.is_empty() {
        return None;
    }
    Some((program.to_string(), timestamp.to_string()))
}

/// Lista las grabaciones **cerradas** de una carpeta.
///
/// El orden es de la más antigua a la más reciente, por nombre (que lleva la fecha), para
/// que el llamador pueda quedarse con las nuevas comparando con lo que ya conocía.
pub fn scan_folder(folder: &Path) -> Result<Vec<FinishedLog>, String> {
    let entries = fs::read_dir(folder)
        .map_err(|e| format!("No se pudo leer la carpeta de logs {}: {}", folder.display(), e))?;

    // Se recogen los nombres primero: hacen falta los dos ficheros de cada grabación
    // (datos y resumen) para decidir si está cerrada.
    let mut names: HashSet<String> = HashSet::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        // Los directorios y enlaces raros se ignoran: solo interesan ficheros sueltos.
        match entry.file_type() {
            Ok(t) if t.is_file() => {}
            _ => continue,
        }
        if let Ok(name) = entry.file_name().into_string() {
            names.insert(name);
        }
    }

    let mut found: Vec<FinishedLog> = names
        .iter()
        .filter(|n| is_data_file(n))
        .filter(|n| names.contains(&summary_name_of(n)))
        .filter_map(|n| {
            let stem = n.strip_suffix(DATA_SUFFIX)?;
            let (program, timestamp) = split_stem(stem)?;
            Some(FinishedLog {
                path: folder.join(n).to_string_lossy().to_string(),
                program,
                timestamp,
            })
        })
        .collect();

    // Orden estable por nombre de fichero (equivale al orden temporal, porque la fecha
    // va en el nombre con dígitos de ancho fijo).
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn touch(dir: &Path, name: &str, content: &str) {
        let mut f = fs::File::create(dir.join(name)).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "thoriapp-watch-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn solo_se_considera_cerrada_la_grabacion_con_summary() {
        let dir = temp_dir("cerrada");
        touch(&dir, "Palworld_2026-11-02_14-43-52.csv", "fps,frametime\n");
        touch(&dir, "Palworld_2026-11-02_14-43-52_summary.csv", "avg\n");
        // Esta se quedó sin cerrar (el juego se cerró de golpe).
        touch(&dir, "Palworld_2026-11-02_15-00-00.csv", "fps,frametime\n");

        let found = scan_folder(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("2026-11-02_14-43-52.csv"));
        assert_eq!(found[0].program, "Palworld");
        assert_eq!(found[0].timestamp, "2026-11-02_14-43-52");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_summary_no_se_confunde_con_una_grabacion() {
        let dir = temp_dir("summary");
        touch(&dir, "Juego_2026-11-02_14-43-52_summary.csv", "avg\n");
        let found = scan_folder(&dir).unwrap();
        assert!(found.is_empty(), "un summary sin sus datos no es una grabación");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_nombre_del_programa_puede_llevar_puntos_y_guiones() {
        // Casos reales sacados de una instalación de Goverlay.
        let casos = [
            ("7DaysToDie_2026-10-30_16-56-40", "7DaysToDie"),
            ("7DaysToDie.x86_64_2026-11-02_11-49-37", "7DaysToDie.x86_64"),
            ("Palworld-Win64-Shipping_2026-11-02_14-43-52", "Palworld-Win64-Shipping"),
            ("AION2_2026-11-03_10-46-42", "AION2"),
        ];
        for (stem, esperado) in casos {
            let (program, _) = split_stem(stem).unwrap_or_else(|| panic!("falló con {}", stem));
            assert_eq!(program, esperado);
        }
    }

    #[test]
    fn se_extrae_la_fecha_del_nombre() {
        let (_, ts) = split_stem("AION2_2026-11-03_10-46-42").unwrap();
        assert_eq!(ts, "2026-11-03_10-46-42");
    }

    #[test]
    fn un_nombre_sin_fecha_no_cuenta() {
        assert!(split_stem("notas_importantes_aqui").is_none());
        assert!(split_stem("resumen").is_none());
        // Se parece a una fecha pero no lo es.
        assert!(split_stem("juego_2026-11-03_XX-46-42").is_none());
    }

    #[test]
    fn el_orden_es_del_mas_antiguo_al_mas_reciente() {
        let dir = temp_dir("orden");
        for ts in ["2026-11-02_16-00-00", "2026-11-02_14-00-00", "2026-11-02_15-00-00"] {
            touch(&dir, &format!("J_{}.csv", ts), "x\n");
            touch(&dir, &format!("J_{}_summary.csv", ts), "x\n");
        }
        let all = scan_folder(&dir).unwrap();
        let ts: Vec<&str> = all.iter().map(|l| l.timestamp.as_str()).collect();
        assert_eq!(ts, vec!["2026-11-02_14-00-00", "2026-11-02_15-00-00", "2026-11-02_16-00-00"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_carpeta_que_no_existe_da_error_explicado() {
        let err = scan_folder(Path::new("/no/existe/esta/carpeta")).unwrap_err();
        assert!(err.contains("No se pudo leer"));
    }

    #[test]
    fn los_subdirectorios_se_ignoran() {
        let dir = temp_dir("subdir");
        fs::create_dir_all(dir.join("Juego_2026-11-02_14-00-00.csv")).unwrap();
        fs::create_dir_all(dir.join("Juego_2026-11-02_14-00-00_summary.csv")).unwrap();
        assert!(scan_folder(&dir).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn se_filtra_por_ruta_completa_no_por_nombre_suelto() {
        // Dos carpetas distintas pueden tener un log con el mismo nombre: una no debe
        // "tapar" a la otra solo porque el nombre coincida.
        let dir_a = temp_dir("ruta-a");
        let dir_b = temp_dir("ruta-b");
        for d in [&dir_a, &dir_b] {
            touch(d, "J_2026-11-02_10-00-00.csv", "x\n");
            touch(d, "J_2026-11-02_10-00-00_summary.csv", "x\n");
        }
        let a = scan_folder(&dir_a).unwrap();
        let b = scan_folder(&dir_b).unwrap();
        assert_eq!(a[0].timestamp, b[0].timestamp);
        assert_ne!(a[0].path, b[0].path, "la ruta completa distingue las dos");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }
}

// ==================== COMANDOS DE TAURI ====================
// La lógica de arriba es pura (recibe rutas y devuelve datos); estas funciones solo
// atan esa lógica al disco y a la configuración de la app.

use crate::log_watcher_state::{state_file, WatchState};

/// Carpeta de configuración de la app, donde se guarda el estado del vigilante.
fn config_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("thoriapp")
}

fn state_path() -> std::path::PathBuf {
    state_file(&config_dir())
}

/// Grabaciones cerradas que hay en la carpeta y que Thoriapp **todavía no conoce**.
///
/// Devuelve solo las nuevas para que el frontend no vuelva a analizar (ni a duplicar en
/// el historial) lo que ya procesó. No marca nada como visto: eso lo hace
/// `mark_logs_processed` cuando el análisis ha salido bien. Así, si el análisis falla, la
/// grabación se reintenta en el siguiente sondeo en vez de perderse.
#[tauri::command]
pub fn scan_new_mangohud_logs(folder: String) -> Result<Vec<FinishedLog>, String> {
    let all = scan_folder(Path::new(&folder))?;
    let state = WatchState::load(&state_path());
    let nuevas: Vec<FinishedLog> = all
        .into_iter()
        .filter(|l| !state.contains(&l.path))
        .collect();
    Ok(nuevas)
}

/// Todas las grabaciones cerradas de la carpeta, sin filtrar por lo ya procesado.
///
/// Se usa al elegir carpeta y para el botón de "revisar ahora": el usuario quiere ver lo
/// que hay, no solo lo nuevo.
#[tauri::command]
pub fn list_mangohud_logs(folder: String) -> Result<Vec<FinishedLog>, String> {
    scan_folder(Path::new(&folder))
}

/// Marca rutas como ya procesadas. Se llama **después** de guardar el benchmark.
#[tauri::command]
pub fn mark_logs_processed(paths: Vec<String>) -> Result<(), String> {
    let file = state_path();
    let mut state = WatchState::load(&file);
    for p in paths {
        state.mark(&p);
    }
    state.save(&file)
}

/// Cuántas grabaciones hay apuntadas como procesadas (para diagnóstico y tests).
#[tauri::command]
pub fn processed_logs_count() -> Result<usize, String> {
    Ok(WatchState::load(&state_path()).len())
}

/// Olvida todo lo procesado: la próxima revisión vuelve a ver la carpeta entera.
///
/// Es la salida para cuando algo se procesó mal o el usuario quiere rehacer el historial
/// sin tocar los ficheros de MangoHud.
#[tauri::command]
pub fn forget_processed_logs() -> Result<(), String> {
    let file = state_path();
    if file.exists() {
        std::fs::remove_file(&file)
            .map_err(|e| format!("No se pudo borrar el registro de logs procesados: {}", e))?;
    }
    Ok(())
}
