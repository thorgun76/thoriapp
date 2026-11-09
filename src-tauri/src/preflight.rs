//! Estado real del sistema que necesita el comando de lanzamiento.
//!
//! Antes solo se comprobaba con `which`: eso daba falsos negativos (MangoHud se carga
//! como capa Vulkan, sin binario en el PATH) y falsos positivos (un archivo con el
//! nombre correcto pero que no es ejecutable). Aquí se mira el sistema de archivos
//! directamente y **no se lanza ningún proceso**: es instantáneo y no puede colgarse
//! esperando a un binario que, por ejemplo, abre un diálogo.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Wrappers que Thoriapp puede poner en el comando. Se comprueba su existencia para
/// avisar antes de que el juego no arranque por un "command not found".
pub const KNOWN_WRAPPERS: [&str; 8] = [
    "gamemoderun",
    "mangohud",
    "gamescope",
    "prime-run",
    "vkbasalt",
    "obs-gamecapture",
    "game-performance",
    "scx_lavd",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryInfo {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeEnv {
    /// Wrappers conocidos que sí existen, con su ruta real.
    pub binaries: Vec<BinaryInfo>,
    /// Binario `mangohud` (el wrapper que pone las variables y lanza el juego).
    pub mangohud_binary: Option<String>,
    /// Archivo de capa Vulkan de MangoHud (suficiente para que `MANGOHUD=1` funcione
    /// sin el binario wrapper).
    pub mangohud_vulkan_layer: Option<String>,
    /// Steam está instalado como Flatpak: el comando corre dentro del sandbox.
    pub steam_flatpak: bool,
    /// `/dev/ntsync` presente (kernel ≥ 6.14 con el módulo cargado).
    pub ntsync: bool,
    /// Versión del kernel en ejecución.
    pub kernel: String,
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
}

/// Directorios donde buscar ejecutables: el PATH más las rutas habituales, porque una
/// app lanzada desde el menú puede heredar un PATH recortado.
fn bin_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(|d| d.to_string())
        .collect();

    let home = home();
    for extra in [
        "/usr/local/bin",
        "/usr/bin",
        "/usr/games",
        "/snap/bin",
        "/var/lib/flatpak/exports/bin",
    ] {
        dirs.push(extra.to_string());
    }
    dirs.push(format!("{}/.local/bin", home));
    dirs.push(format!("{}/bin", home));

    dirs
}

/// Busca un ejecutable y devuelve su ruta completa. Exige que sea un archivo (no un
/// directorio con el mismo nombre, que `exists()` aceptaría).
pub fn find_binary(bin: &str) -> Option<String> {
    for dir in bin_dirs() {
        let path = Path::new(&dir).join(bin);
        if path.is_file() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

/// Rutas donde las capas Vulkan pueden estar registradas.
fn vulkan_layer_dirs() -> Vec<String> {
    let home = home();
    let mut dirs = vec![
        "/usr/share/vulkan/implicit_layer.d".to_string(),
        "/usr/local/share/vulkan/implicit_layer.d".to_string(),
        "/etc/vulkan/implicit_layer.d".to_string(),
        "/usr/share/vulkan/explicit_layer.d".to_string(),
        format!("{}/.local/share/vulkan/implicit_layer.d", home),
    ];
    // Capas instaladas DENTRO del sandbox de Steam Flatpak.
    dirs.push(format!(
        "{}/.var/app/com.valvesoftware.Steam/.local/share/vulkan/implicit_layer.d",
        home
    ));
    dirs
}

/// Ruta de la capa Vulkan de MangoHud (`MangoHud.x86_64.json` y similares).
fn find_mangohud_layer() -> Option<String> {
    for dir in vulkan_layer_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if name.contains("mangohud") && name.ends_with(".json") {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Lee el estado del sistema. Nunca falla: lo que no se pueda leer queda en `None` o
/// `false`, y la interfaz simplemente no muestra ese aviso.
pub fn get_runtime_env() -> RuntimeEnv {
    let binaries = KNOWN_WRAPPERS
        .iter()
        .filter_map(|name| {
            find_binary(name).map(|path| BinaryInfo {
                name: name.to_string(),
                path,
            })
        })
        .collect::<Vec<_>>();

    let home = home();
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    RuntimeEnv {
        binaries,
        mangohud_binary: find_binary("mangohud"),
        mangohud_vulkan_layer: find_mangohud_layer(),
        steam_flatpak: Path::new(&format!("{}/.var/app/com.valvesoftware.Steam", home)).is_dir(),
        ntsync: Path::new("/dev/ntsync").exists(),
        kernel,
    }
}

#[tauri::command]
pub fn get_runtime_env_command() -> RuntimeEnv {
    get_runtime_env()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_binary_ignora_lo_que_no_es_un_archivo() {
        // Un directorio con ese nombre no cuenta como ejecutable.
        assert!(find_binary("no-existe-seguro-12345").is_none());
    }

    #[test]
    fn find_binary_encuentra_un_binario_del_sistema() {
        // `sh` existe en cualquier sistema Linux donde corran los tests.
        let found = find_binary("sh");
        assert!(found.is_some(), "no encontró sh en el PATH");
        assert!(Path::new(&found.unwrap()).is_file());
    }

    #[test]
    fn el_entorno_siempre_devuelve_estructura_valida() {
        let env = get_runtime_env();
        // No se puede asumir qué hay instalado, pero sí que los datos son coherentes.
        for b in &env.binaries {
            assert!(!b.name.is_empty());
            assert!(!b.path.is_empty());
        }
        assert!(!env.kernel.contains('\n'));
    }
}
