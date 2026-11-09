//! Dónde guarda el sistema los shaders ya compilados de un juego.
//!
//! Esto responde a otra pregunta que la del análisis del log: si un juego tartamudea
//! solo las primeras veces, la causa suele ser que todavía no tiene caché de shaders.
//! Aquí se mira si esa caché existe y cuánto ocupa, **sin lanzar ningún proceso** y sin
//! abrir el juego: solo se recorren directorios.
//!
//! Se mira lo que de verdad se usa en Linux:
//!
//! - `compatdata/<appid>/pfx` + `steamapps/shadercache/<appid>`: lo que prepara Steam
//!   para los juegos de Proton (Steam hace un pre-caché al instalar y al actualizar).
//! - `mesa_shader_cache` en la caché del usuario: la caché propia de Mesa, que afecta
//!   también a los juegos nativos con OpenGL/Vulkan.
//! - `*.dxvk-cache`: conversión anticipada de shaders de D3D9/D3D10/D3D11 a SPIR-V.
//! - `GPL` (Graphics Pipeline Library, en `~/.cache/nvidia`): las tuberías gráficas que
//!   reutiliza el driver, tanto en NVIDIA como en Mesa.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Estado de las cachés de shaders de un juego.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShaderCache {
    /// Bytes totales de las cachés encontradas para este juego.
    pub bytes: u64,
    /// Directorios o archivos de caché encontrados, con la ruta real.
    pub locations: Vec<ShaderCacheEntry>,
    /// `true` si Steam tiene preparada la caché previa del juego (`shadercache/<appid>`).
    /// Es la señal de que la primera compilación ya está hecha en el disco.
    pub steam_precached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderCacheEntry {
    /// `steam`, `mesa`, `dxvk` o `gpl`.
    pub kind: String,
    pub path: String,
    pub bytes: u64,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// Raíces de Steam: nativo y Flatpak. Se repiten aquí en vez de importar las de `steam`
/// porque allí son privadas y además allí se añaden las de `libraryfolders.vdf`.
fn steam_roots() -> Vec<PathBuf> {
    let home = home();
    vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]
}

/// Tamaño de un archivo o de un árbol de directorios, en bytes.
/// No sigue enlaces simbólicos para no contar dos veces lo mismo ni salirse de la caché.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| dir_size(&e.path()))
        .sum()
}

/// Bibliotecas de Steam donde puede estar instalado el juego: las de siempre más las de
/// `libraryfolders.vdf`, porque la caché de shaders vive junto a `steamapps`.
fn steam_libraries() -> Vec<PathBuf> {
    let mut libs: Vec<PathBuf> = Vec::new();
    for root in steam_roots() {
        if root.join("steamapps").is_dir() {
            libs.push(root.join("steamapps"));
        }
        if let Ok(txt) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in txt.lines() {
                let parts: Vec<&str> = line.split('"').collect();
                if parts.len() >= 4 && parts[1].eq_ignore_ascii_case("path") {
                    libs.push(PathBuf::from(parts[3]).join("steamapps"));
                }
            }
        }
    }
    libs.sort();
    libs.dedup();
    libs
}

/// Cachés de shaders del juego en el disco duro.
///
/// Solo se cuentan las de **este** juego: `mesa_shader_cache` y `GPL` son globales y no
/// se pueden atribuir a un appid, así que quedan fuera a propósito. Sumarlas daría un
/// número grande que no dice nada del juego.
pub fn shader_cache(appid: u32) -> ShaderCache {
    let mut cache = ShaderCache::default();
    if appid == 0 {
        return cache;
    }

    for lib in steam_libraries() {
        let steam_cache = lib.join("shadercache").join(appid.to_string());
        if steam_cache.is_dir() {
            let bytes = dir_size(&steam_cache);
            cache.steam_precached = true;
            cache.bytes += bytes;
            cache.locations.push(ShaderCacheEntry {
                kind: "steam".to_string(),
                path: steam_cache.to_string_lossy().to_string(),
                bytes,
            });
            // No se sigue buscando en las demás bibliotecas: el juego está en esta y su
            // caché ya está contada. La de DXVK de abajo se suma aparte porque es otro
            // archivo, no el mismo contado dos veces.
            continue;
        }

        // Sin caché de Steam, al menos saber si existe el prefix (el juego se lanzó).
        let pfx = lib.join("compatdata").join(appid.to_string()).join("pfx");
        if pfx.is_dir() {
            cache.locations.push(ShaderCacheEntry {
                kind: "prefix".to_string(),
                path: pfx.to_string_lossy().to_string(),
                bytes: 0,
            });
        }
    }

    // Caché de DXVK del propio juego (vive en su carpeta de instalación o en el prefix).
    // Se busca un nivel de profundidad a propósito: recorrer "common" entero tarda
    // segundos y aquí no se puede hacer esperar a la interfaz.
    for lib in steam_libraries() {
        let dxvk = lib.join("common").join(appid.to_string());
        if dxvk.is_dir() {
            let mut total = 0u64;
            let mut found = false;
            if let Ok(entries) = fs::read_dir(&dxvk) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.ends_with(".dxvk-cache") {
                        found = true;
                        total += dir_size(&entry.path());
                    }
                }
            }
            if found {
                cache.bytes += total;
                cache.locations.push(ShaderCacheEntry {
                    kind: "dxvk".to_string(),
                    path: dxvk.to_string_lossy().to_string(),
                    bytes: total,
                });
            }
        }
    }

    cache
}

#[tauri::command]
pub fn get_shader_cache(appid: u32) -> ShaderCache {
    shader_cache(appid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appid_cero_no_tiene_cache() {
        // Un juego sin appid (Lutris, binario suelto) no puede tener caché de Steam.
        let cache = shader_cache(0);
        assert_eq!(cache.bytes, 0);
        assert!(cache.locations.is_empty());
        assert!(!cache.steam_precached);
    }

    #[test]
    fn dir_size_de_una_ruta_inexistente_es_cero() {
        assert_eq!(dir_size(Path::new("/no/existe/esto/seguro-12345")), 0);
    }

    #[test]
    fn dir_size_cuenta_el_contenido_real() {
        let base = std::env::temp_dir().join("thoriapp-test-shaders");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("a.txt"), b"12345").unwrap();
        fs::write(base.join("sub/b.txt"), b"123").unwrap();

        // Los dos archivos, sin contar directorios ni rutas que no existen.
        assert_eq!(dir_size(&base), 8);
        assert_eq!(dir_size(&base.join("sub")), 3);
        // Se limpia para no dejar basura en /tmp.
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn un_appid_sin_cache_no_inventa_datos() {
        let cache = shader_cache(999_999_999);
        assert_eq!(cache.bytes, 0);
        assert!(!cache.steam_precached);
    }
}
