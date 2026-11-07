// ==================== CARÁTULAS LOCALES DE LOS LAUNCHERS ====================
//
// Cada launcher guarda en disco las carátulas que ve el usuario, incluidas las
// que haya puesto a mano. Reutilizarlas da la mejor experiencia: si alguien
// cambió la portada de un juego en Lutris, aquí se ve la misma.
//
// Rutas (Linux):
//   Lutris   ~/.local/share/lutris/coverart/<slug>.jpg   (versiones antiguas: ~/.cache/lutris)
//   Heroic   ~/.config/heroic/<store>_store/library_cache/ + art_cover del JSON
//   Heronite ~/.local/share/heronite/local_library/art/
//
// La imagen se devuelve como data URI base64. En Tauri 2 una ruta `file://` no
// se puede poner en un <img>: haría falta habilitar el protocolo `asset` y la
// CSP. Así el frontend recibe un `src` que funciona sin permisos extra.

use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::{Path, PathBuf};

/// Tamaño máximo de una carátula. Evita cargar en memoria un archivo enorme si
/// la ruta apunta a algo inesperado.
const MAX_COVER_BYTES: u64 = 8 * 1024 * 1024;

/// Extensiones admitidas, en orden de preferencia.
const EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "avif"];

fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Convierte un archivo de imagen en `data:image/<mime>;base64,...`.
/// Devuelve `None` si no existe, es demasiado grande o no es una imagen válida.
fn to_data_uri(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_COVER_BYTES {
        return None;
    }

    let bytes = std::fs::read(path).ok()?;
    to_data_uri_from_bytes(&bytes)
}

/// Convierte bytes de imagen en data URI, validando la firma del archivo.
/// Devuelve `None` si no son una imagen reconocible.
fn to_data_uri_from_bytes(bytes: &[u8]) -> Option<String> {
    // Se comprueba la firma real en vez de fiarse de la extensión: así un .jpg
    // que en realidad es un HTML de error no se envía como imagen.
    let mime = match bytes.get(..4)? {
        [0xFF, 0xD8, 0xFF, _] => "image/jpeg",
        [0x89, b'P', b'N', b'G'] => "image/png",
        [b'R', b'I', b'F', b'F'] if bytes.get(8..12) == Some(b"WEBP") => "image/webp",
        [0, 0, 0, _] if bytes.get(4..8) == Some(b"ftyp") => "image/avif",
        _ => return None,
    };

    Some(format!("data:{};base64,{}", mime, STANDARD.encode(bytes)))
}

/// Busca el juego en un objeto indexado por `app_name` o en una lista con el
/// campo `app_name` (según la versión y la tienda de Heroic).
fn lookup_heroic_entry<'a>(value: &'a serde_json::Value, app_name: &str) -> Option<&'a serde_json::Value> {
    if let Some(found) = value.get(app_name) {
        return Some(found);
    }
    value.as_array().and_then(|a| {
        a.iter().find(|g| {
            g.get("app_name").and_then(|n| n.as_str()) == Some(app_name)
                || g.get("appName").and_then(|n| n.as_str()) == Some(app_name)
        })
    })
}

/// Carpeta donde se cachean las carátulas descargadas de Heroic.
fn remote_cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".cache"))
        .join("thoriapp/covers")
}

/// Descarga una carátula remota (Epic/GOG sirven el arte por URL) y la cachea.
///
/// Se cachea en disco porque la respuesta se manda como data URI: sin caché,
/// 40 juegos supondrían volver a descargar ~2 MB en cada arranque. Si la
/// descarga falla, se devuelve `None` y la tarjeta cae al CDN de Steam.
fn remote_image(url: &str) -> Option<String> {
    // Nombre de archivo estable a partir de la URL (hash simple para evitar
    // caracteres problemáticos y URLs larguísimas).
    let mut hash: u64 = 1469598103934665603;
    for byte in url.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    let cached = remote_cache_dir().join(format!("{:016x}.img", hash));

    if let Some(uri) = to_data_uri(&cached) {
        return Some(uri);
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(8))
        .build();
    let resp = agent.get(url).call().ok()?;
    let bytes = {
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut resp.into_reader(), &mut buf).ok()?;
        buf
    };
    if bytes.is_empty() || bytes.len() as u64 > MAX_COVER_BYTES {
        return None;
    }

    // Se guarda solo si de verdad es una imagen.
    to_data_uri_from_bytes(&bytes)?;
    let _ = std::fs::create_dir_all(remote_cache_dir());
    let _ = std::fs::write(&cached, &bytes);
    println!("[Heroic] Carátula descargada: {} -> {}", url, cached.display());
    to_data_uri(&cached)
}

/// Busca un archivo de imagen con cualquiera de las extensiones admitidas.
fn find_image_with_extensions(base: &Path) -> Option<PathBuf> {
    // Si ya viene con extensión y existe, se usa tal cual.
    if base.is_file() {
        return Some(base.to_path_buf());
    }
    EXTENSIONS
        .iter()
        .map(|ext| base.with_extension(ext))
        .find(|p| p.is_file())
}

// ==================== LUTRIS ====================

/// Directorios donde Lutris guarda las carátulas, del más nuevo al más antiguo.
/// Lutris migró de `~/.cache` a `~/.local/share` en la 0.5.18 y mantiene ambas.
fn lutris_cover_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    vec![
        home.join(".local/share/lutris/coverart"),
        home.join(".cache/lutris/coverart"),
        // Flatpak
        home.join(".var/app/net.lutris.Lutris/data/lutris/coverart"),
        home.join(".var/app/net.lutris.Lutris/cache/lutris/coverart"),
    ]
}

/// Carátula de un juego de Lutris.
///
/// `keys` admite varias candidatas separadas por `|` (slug, `slug-timestamp`
/// sin el timestamp, id numérico...). Se prueba cada una y se devuelve la
/// primera que exista, porque Lutris no usa un único esquema de nombres.
pub fn lutris_cover(keys: &str) -> Option<String> {
    let dirs = lutris_cover_dirs();
    for key in keys.split('|').filter(|k| !k.trim().is_empty()) {
        // Evita leer archivos arbitrarios si el nombre viniera manipulado.
        if key.contains('/') || key.contains("..") {
            continue;
        }
        for dir in &dirs {
            // Lutris guarda `.jpg` para el arte oficial y `.png` para el que
            // añade el usuario a mano; `find_image_with_extensions` cubre ambos.
            if let Some(path) = find_image_with_extensions(&dir.join(key)) {
                if let Some(uri) = to_data_uri(&path) {
                    println!("[Lutris] Carátula: {} -> {}", key, path.display());
                    return Some(uri);
                }
            }
        }
    }

    // Sin carátula: se deja rastro para poder diagnosticar («este juego sale
    // con el icono de mando, ¿qué busca?»). Solo si la carpeta existe.
    let existentes: Vec<String> = dirs
        .iter()
        .filter(|d| d.is_dir())
        .map(|d| d.display().to_string())
        .collect();
    if !existentes.is_empty() {
        println!(
            "[Lutris] ✗ Sin carátula. Buscado '{}' en: {}",
            keys,
            existentes.join(", ")
        );
    }
    None
}

// ==================== HEROIC ====================

fn heroic_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    vec![
        home.join(".config/heroic"),
        home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
    ]
}

/// Carátula de un juego de Heroic (Epic, GOG, Amazon o añadido a mano).
///
/// `app_name` es el identificador interno. Primero se buscan los campos
/// `art_cover` / `art_square` de la biblioteca correspondiente (funciona
/// también si el usuario puso una imagen propia), y si no, un archivo dentro de
/// `*_store/library_cache/`.
pub fn heroic_cover(app_name: &str) -> Option<String> {
    if app_name.trim().is_empty() || app_name.contains("..") {
        return None;
    }

    for dir in heroic_dirs() {
        // 1. Arte declarado en el JSON de la biblioteca
        for store in ["legendary", "gog", "nile", "sideload"] {
            let json = match store {
                "legendary" => dir.join("store_cache/legendary_library.json"),
                "gog" => dir.join("store_cache/gog_library.json"),
                "nile" => dir.join("store_cache/nile_library.json"),
                "sideload" => dir.join("sideload_apps/library.json"),
                _ => continue,
            };
            if !json.is_file() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&json) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            // El JSON de Heroic va envuelto: `{__timestamp, library: {...}}` en
            // Epic y `{games: {...}}` en sideloaded. Se mira el objeto interior
            // antes que la raíz.
            let entry = ["library", "games"]
                .iter()
                .filter_map(|k| value.get(*k))
                .find_map(|inner| lookup_heroic_entry(inner, app_name))
                .or_else(|| lookup_heroic_entry(&value, app_name));
            let Some(entry) = entry else { continue };

            for field in ["art_cover", "art_square"] {
                let Some(art) = entry.get(field).and_then(|v| v.as_str()) else {
                    continue;
                };
                if art.trim().is_empty() {
                    continue;
                }
                // El campo puede ser una ruta local o una URL (lo habitual en
                // Epic/GOG). Se intentan las dos.
                if art.starts_with("http://") || art.starts_with("https://") {
                    if let Some(uri) = remote_image(art) {
                        return Some(uri);
                    }
                    continue;
                }
                let candidate = Path::new(art);
                let full = if candidate.is_absolute() {
                    candidate.to_path_buf()
                } else {
                    dir.join(candidate)
                };
                if let Some(uri) = to_data_uri(&full) {
                    println!("[Heroic] Carátula local: {}", full.display());
                    return Some(uri);
                }
            }
        }

        // 2. Archivo en la caché de imágenes de la tienda
        for store in ["legendary", "gog", "nile", "sideload"] {
            let cache = dir.join(format!("{store}_store/library_cache"));
            if let Some(path) = find_image_with_extensions(&cache.join(app_name)) {
                if let Some(uri) = to_data_uri(&path) {
                    return Some(uri);
                }
            }
            // Variante con el arte dentro de una carpeta con el nombre del juego
            let nested = cache.join(app_name);
            if nested.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&nested) {
                    for entry in entries.flatten() {
                        if let Some(uri) = to_data_uri(&entry.path()) {
                            return Some(uri);
                        }
                    }
                }
            }
        }
    }
    None
}

// ==================== HERONITE ====================
//
// Heronite (https://heronite.app) guarda el arte **como URL** en sus JSONs, no
// como un archivo con el nombre del juego:
//
//   local_library/library.json   {"games": {"<appName>": {"title": ..., ...}}}
//   local_library/art.json       {"games": {"<runner>:<appName>": {"coverUrl": ...}}}
//   local_library/metadata.json  {"games": {"<runner>:<appName>": {"coverUrl": ...}}}
//   store_cache/legendary_library.json  {"library": [{"art_square": ...}]}
//
// y `images-cache/` es la caché HTTP de esas URLs: el nombre de archivo es el
// **SHA-256 del string exacto de la URL** (`crates/images/src/fetch.rs`), así
// que no se puede adivinar a partir del nombre del juego. Por eso se resuelve
// al revés: se lee la URL del JSON y se calcula su hash para encontrar el
// archivo ya descargado. Si no está en caché se descarga (y se cachea), igual
// que hace Heronite.
//
// Orden de la cadena, espejo de `card_cover` de Heronite: arte propio ->
// metadatos -> `art_square` -> `art_cover` -> carátula de Steam -> *fallback*
// a las demás launchers.

/// Carpetas de datos de Heronite. `HERONITE_DATA_DIR` permite una segunda
/// Collection (documentado por Heronite) y se prueba antes que la estándar.
fn heronite_data_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    let mut dirs = Vec::new();
    if let Some(custom) = std::env::var_os("HERONITE_DATA_DIR") {
        dirs.push(PathBuf::from(custom));
    }
    dirs.push(home.join(".local/share/heronite"));
    dirs
}

/// `true` si el nombre se puede usar para componer una ruta sin escapar de la
/// carpeta de arte (nada de `../` ni rutas absolutas).
fn is_safe_cover_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && !name.contains("..") && !name.contains('/') && !name.contains('\\')
}

/// SHA-256 en hexadecimal: el nombre que Heronite (heredado de Heroic) usa en
/// `images-cache` para una URL.
fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

/// Ruta del archivo de caché de `url` dentro de `data_dir`.
fn heronite_cache_path(data_dir: &Path, url: &str) -> PathBuf {
    data_dir.join("images-cache").join(sha256_hex(url))
}

/// `true` para los valores de arte que apuntan a un archivo del propio disco
/// (`localart://`, `file:` o una ruta absoluta), no a una URL.
fn is_local_art(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("localart:")
        || value.starts_with("file:")
        || value.starts_with('/')
}

/// Interpreta un valor de arte de Heronite como URL descargable.
///
/// Los valores `localart://...` / `file:...` apuntan a archivos del propio
/// usuario (relativos a la carpeta de datos) y se tratan aparte; `fallback` es
/// el marcador de Heronite, no una imagen.
fn art_url(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() || value == "fallback" {
        return None;
    }
    if value.starts_with("localart:") || value.starts_with("file:") || value.starts_with('/') {
        return None;
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        Some(value)
    } else {
        None
    }
}

/// Archivo local al que apunta un valor `localart://` de Heronite.
///
/// `localart://art/<x>` vive en `local_library/art/<x>`; el resto se resuelve
/// contra la carpeta de datos (el esquema no está documentado y solo se usa
/// para el arte elegido a mano).
fn localart_path(data_dir: &Path, value: &str) -> Option<PathBuf> {
    let rest = value
        .strip_prefix("localart://")
        .or_else(|| value.strip_prefix("localart:"))
        .or_else(|| value.strip_prefix("file://"))
        .or_else(|| value.strip_prefix("file:"))
        .or_else(|| Some(value).filter(|v| v.starts_with('/')))?;

    let rest = rest.trim_start_matches('/');
    // Sin `..`: el valor viene de un JSON que puede haberse editado.
    if rest.split('/').any(|part| part == "..") {
        return None;
    }

    let direct = data_dir.join(rest);
    if direct.is_file() {
        return Some(direct);
    }
    // `localart://art/<x>` -> `local_library/art/<x>`
    if let Some(name) = rest.strip_prefix("art/") {
        let p = data_dir.join("local_library/art").join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    direct.is_file().then_some(direct)
}

/// Lee un JSON de Heronite y devuelve la entrada del juego en `games`.
///
/// Heronite indexa por `"<runner>:<appName>"` (art.json, metadata.json) o por
/// `appName` (library.json). Se prueban las dos, más el `appName` suelto, para
/// no depender de qué runner puso el usuario.
fn heronite_json_entry<'a>(
    value: &'a serde_json::Value,
    app_name: &str,
    runner: Option<&str>,
) -> Option<&'a serde_json::Value> {
    let games = value.get("games")?;
    if let Some(key) = runner.filter(|r| !r.is_empty()) {
        if let Some(found) = games.get(format!("{key}:{app_name}")) {
            return Some(found);
        }
    }
    games.get(app_name)
}

/// Valores de arte de una entrada, en el orden de `card_cover` de Heronite.
///
/// `coverUrl` (art.json y metadata.json) es lo que el usuario eligió; luego
/// `art_square` (carátula vertical de Heroic) y `art_cover` (horizontal). Se
/// conservan los `localart://` y se descartan los vacíos y `fallback`.
fn heronite_art_urls(entry: &serde_json::Value) -> Vec<String> {
    ["coverUrl", "cover_url", "art_square", "artSquare", "art_cover", "artCover"]
        .iter()
        .filter_map(|field| entry.get(*field).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "fallback")
        .filter(|v| is_local_art(v) || art_url(v).is_some())
        .map(String::from)
        .collect()
}

/// Lee los JSONs de Heronite que pueden tener arte para `app_name` y devuelve
/// los valores en orden de preferencia (puede haber `localart://`).
fn heronite_art_values_in(dirs: &[PathBuf], app_name: &str, runner: Option<&str>) -> Vec<String> {
    const FILES: [&str; 3] = [
        "local_library/art.json",
        "local_library/metadata.json",
        // Copia de los datos de Heroic que Heronite importa al arrancar: aquí
        // viven `art_square` / `art_cover` de los juegos de Epic y GOG.
        "store_cache/legendary_library.json",
    ];

    let mut out: Vec<String> = Vec::new();
    for dir in dirs {
        for file in FILES {
            let path = dir.join(file);
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };

            // El JSON de Epic es una lista de juegos con `app_name`.
            if let Some(list) = json.get("library").and_then(|l| l.as_array()) {
                for game in list {
                    if game.get("app_name").and_then(|n| n.as_str()) == Some(app_name) {
                        out.extend(heronite_art_urls(game));
                    }
                }
                continue;
            }

            if let Some(entry) = heronite_json_entry(&json, app_name, runner) {
                out.extend(heronite_art_urls(entry));
            }
        }
    }

    // Sin duplicados, conservando el orden de preferencia.
    let mut seen = std::collections::HashSet::new();
    out.retain(|v| seen.insert(v.clone()));
    out
}

/// Convierte un valor de arte (`localart://...` o una URL) en data URI.
///
/// Para las URLs se reutiliza la caché HTTP de Heronite cuando ya está
/// descargada; si no, se descarga con la caché propia de Thoriapp (para no
/// escribir en la carpeta de Heronite) y se devuelve igualmente.
fn heronite_art_to_uri(dir: &Path, value: &str) -> Option<String> {
    // Primero los archivos del usuario (`localart://`), que `art_url` descarta
    // porque no son URLs descargables.
    if is_local_art(value) {
        return localart_path(dir, value).and_then(|path| to_data_uri(&path));
    }
    let url = art_url(value)?;

    // Cache ya presente: es el caso normal cuando Heronite se ha abierto.
    for candidate in [heronite_cache_path(dir, url), heronite_cache_path(dir, &format!("{url}?h=400&resize=1&w=300"))] {
        if let Some(uri) = to_data_uri(&candidate) {
            return Some(uri);
        }
    }

    println!("[Heronite] Descargando carátula: {}", url);
    remote_image(url)
}

/// Carátula de un juego de Heronite a partir de sus datos en disco.
///
/// `key` lleva las candidatas separadas por `|` (appName, clave del
/// library.json, título) y `runner` el runner de Heronite (`legendary`, `gog`,
/// `sideload`...). Separada de `heronite_cover` para poder testearla con
/// carpetas temporales.
fn heronite_cover_in(dirs: &[PathBuf], key: &str, runner: Option<&str>) -> Option<String> {
    let candidates: Vec<&str> = key.split('|').map(|k| k.trim()).filter(|k| !k.is_empty()).collect();
    if candidates.is_empty() {
        return None;
    }

    for dir in dirs {
        for cand in &candidates {
            if !is_safe_cover_name(cand) {
                continue;
            }
            // `art_square` de Epic requiere un `;` / `!` final en la URL según
            // la tienda; se prueban tal cual y con los parámetros de tamaño que
            // usa Heronite para las tarjetas.
            for value in heronite_art_values_in(dirs, cand, runner) {
                if let Some(uri) = heronite_art_to_uri(dir, &value) {
                    println!("[Heronite] Carátula: {} -> {}", cand, value);
                    return Some(uri);
                }
            }
        }
    }
    None
}

/// Carátula de un juego de Heronite sin runner conocido.
#[cfg(test)]
#[allow(dead_code)]
pub fn heronite_cover(key: &str) -> Option<String> {
    heronite_cover_with_runner(key, None)
}

/// Igual que [`heronite_cover`], indicando el runner del juego (lo manda el
/// escáner de la biblioteca).
pub fn heronite_cover_with_runner(key: &str, runner: Option<&str>) -> Option<String> {
    if key.trim().is_empty() || key.contains("..") {
        return None;
    }

    let dirs = heronite_data_dirs();
    if let Some(uri) = heronite_cover_in(&dirs, key, runner) {
        return Some(uri);
    }

    // Sin resultado: se deja rastro para poder ajustar sin adivinar.
    for dir in &dirs {
        for sub in ["local_library/art", "images-cache"] {
            let base = dir.join(sub);
            if base.is_dir() {
                let disponibles = list_images(&base);
                println!(
                    "[Heronite] ✗ Sin carátula para '{}'. {} archivos en {}{}",
                    key,
                    disponibles.len(),
                    base.display(),
                    if disponibles.is_empty() {
                        String::new()
                    } else {
                        format!(", ej: {:?}", disponibles.iter().take(4).collect::<Vec<_>>())
                    }
                );
            }
        }
        let art_json = dir.join("local_library/art.json");
        println!(
            "[Heronite]   {}: {}",
            art_json.display(),
            if art_json.is_file() { "existe" } else { "no existe (arte elegido a mano)" }
        );
        let meta_json = dir.join("local_library/metadata.json");
        println!(
            "[Heronite]   {}: {}",
            meta_json.display(),
            if meta_json.is_file() { "existe" } else { "no existe (metadatos descargados)" }
        );
    }
    None
}

/// Entradas de `library.json`: objeto indexado por clave (versiones nuevas) o
/// array (versiones antiguas, con la clave en `appName`).
fn heronite_entries(games: &serde_json::Value) -> Vec<(String, &serde_json::Value)> {
    match games {
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        serde_json::Value::Array(list) => list
            .iter()
            .map(|entry| {
                let key = entry
                    .get("app_name")
                    .or_else(|| entry.get("appName"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                (key, entry)
            })
            .collect(),
        _ => Vec::new(),
    }
}

// ==================== DIAGNÓSTICO ====================

/// Vuelca qué carpetas y archivos de carátulas existen realmente.
///
/// Sin esto, saber por qué un juego sale con el icono de mando obliga a adivinar
/// nombres de archivo. Se ejecuta bajo demanda (botón de la UI), no en cada
/// arranque, y no lee imágenes: solo lista rutas.
pub fn diagnose() -> String {
    let mut out = String::from("=== Carátulas: diagnóstico ===\n");

    out.push_str("\n-- Lutris --\n");
    for dir in lutris_cover_dirs() {
        out.push_str(&format!("{}: {}\n", dir.display(), if dir.is_dir() { "existe" } else { "no existe" }));
        if dir.is_dir() {
            for f in list_images(&dir) {
                out.push_str(&format!("   {}\n", f));
            }
        }
    }

    out.push_str("\n-- Heroic --\n");
    for dir in heroic_dirs() {
        out.push_str(&format!("{}: {}\n", dir.display(), if dir.is_dir() { "existe" } else { "no existe" }));
        if !dir.is_dir() {
            continue;
        }
        for store in ["legendary", "gog", "nile", "sideload"] {
            let cache = dir.join(format!("{store}_store/library_cache"));
            if cache.is_dir() {
                out.push_str(&format!("  {}\n", cache.display()));
                for f in list_images(&cache) {
                    out.push_str(&format!("     {}\n", f));
                }
            }
        }
        for json in [
            "store_cache/legendary_library.json",
            "store_cache/gog_library.json",
            "store_cache/nile_library.json",
            "sideload_apps/library.json",
        ] {
            let p = dir.join(json);
            if p.is_file() {
                out.push_str(&format!("  JSON {}\n", p.display()));
                if let Ok(text) = std::fs::read_to_string(&p) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        out.push_str(&format!("     claves: {}\n", json_summary(&v)));
                    }
                }
            }
        }
    }

    out.push_str("\n-- Heronite --\n");
    // Heronite guarda el arte como URL en sus JSONs y `images-cache` es la
    // caché HTTP de esas URLs (nombre = SHA-256 de la URL). Para que este
    // diagnóstico sirva de algo se muestra cada juego con la URL de arte que
    // tiene y si su archivo ya está en la caché; si no, Thoriapp la descarga.
    out.push_str("  Nota: images-cache/<sha256 de la URL>; el arte vive en local_library/art.json,\n");
    out.push_str("        local_library/metadata.json y store_cache/legendary_library.json.\n");
    out.push_str("  Los juegos se leen de sideload_apps/library.json y store_cache/*_library.json;\n");
    out.push_str("  local_library/library.json es solo metadata (no dice qué está instalado).\n");
    for dir in heronite_data_dirs() {
        out.push_str(&format!("  Datos: {}\n", dir.display()));

        for file in [
            "local_library/art.json",
            "local_library/metadata.json",
            "local_library/library.json",
        ] {
            let p = dir.join(file);
            out.push_str(&format!(
                "  {}: {}\n",
                p.display(),
                if p.is_file() { "existe" } else { "no existe" }
            ));
        }

        let art = dir.join("local_library/art");
        if art.is_dir() {
            let files = list_images(&art);
            out.push_str(&format!(
                "  {}: {}{} archivos\n",
                art.display(),
                if files.is_empty() { "(vacía) " } else { "" },
                files.len()
            ));
            for f in files.iter().take(10) {
                out.push_str(&format!("     {}\n", f));
            }
        }

        let imgcache = dir.join("images-cache");
        let cache_files = if imgcache.is_dir() { list_images(&imgcache).len() } else { 0 };
        out.push_str(&format!(
            "  {}: {} archivos (caché HTTP, nombre = sha256 de la URL)\n",
            imgcache.display(),
            cache_files
        ));

        // Juegos reales de la Colección: los ficheros de cada tienda y los
        // locales. `local_library/library.json` es solo metadata (no tiene
        // `is_installed`), así que no sirve para saber qué está instalado.
        let mut entries: Vec<(String, serde_json::Value)> = Vec::new();
        // (las entradas se clonan: el JSON temporal se va al acabar la iteración)
        for rel in [
            "sideload_apps/library.json",
            "store_cache/legendary_library.json",
            "store_cache/gog_library.json",
            "store_cache/nile_library.json",
        ] {
            let p = dir.join(rel);
            if !p.is_file() {
                out.push_str(&format!("  {rel}: no existe\n"));
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let Some(pool) = json.get("games").or_else(|| json.get("library")) else { continue };
            let found: Vec<(String, serde_json::Value)> = heronite_entries(pool)
                .into_iter()
                .map(|(k, v)| (k, v.clone()))
                .collect();
            out.push_str(&format!("  {rel}: {} juegos\n", found.len()));
            entries.extend(found);
        }

        let dirs = std::slice::from_ref(&dir);
        for (key, entry) in entries.iter().take(40) {
            let titulo = entry.get("title").and_then(|t| t.as_str()).unwrap_or("<sin title>");
            // `app_name` en las fuentes de Heronite, `appName` en local_library.
            let app_name = entry
                .get("app_name")
                .or_else(|| entry.get("appName"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            let runner = entry.get("runner").and_then(|r| r.as_str());

            // Estado de instalación tal cual lo guarda Heronite. Se muestra para
            // poder distinguir un juego desinstalado (que Thoriapp oculta) de
            // uno que solo tiene el campo ausente.
            // Ruta que guarda Heronite y si sigue en disco: es lo que decide si
            // el juego está realmente instalado.
            let ruta = entry
                .get("install")
                .and_then(|i| i.get("install_path").or_else(|| i.get("executable")))
                .and_then(|v| v.as_str())
                .or_else(|| entry.get("folder_name").and_then(|v| v.as_str()))
                .unwrap_or("");
            let ruta_estado = if ruta.is_empty() {
                String::new()
            } else {
                let existe = ruta.strip_prefix("~/").map(|r| {
                    std::env::var_os("HOME")
                        .map(|h| std::path::Path::new(&h).join(r).exists())
                        .unwrap_or(false)
                });
                let existe = existe.unwrap_or_else(|| std::path::Path::new(ruta).exists());
                format!(" [ruta {}{}]", ruta, if existe { "" } else { " NO EXISTE" })
            };

            let estado_instalacion = match entry.get("is_installed") {
                Some(serde_json::Value::Bool(true)) => "instalado",
                Some(serde_json::Value::Bool(false)) => "DESINSTALADO (se oculta)",
                Some(_) => "is_installed no booleano",
                None => "is_installed ausente (se muestra)",
            };

            let valores = heronite_art_values_in(dirs, app_name, runner);
            let estado = if valores.is_empty() {
                "sin arte en los JSON (usar la carátula de Steam)".to_string()
            } else {
                valores
                    .iter()
                    .map(|v| {
                        if let Some(url) = art_url(v) {
                            let en_cache = heronite_cache_path(&dir, url).is_file()
                                || heronite_cache_path(&dir, &format!("{url}?h=400&resize=1&w=300")).is_file();
                            format!(
                                "{} [{}]",
                                v,
                                if en_cache { "en caché" } else { "se descargará" }
                            )
                        } else {
                            format!("{} [archivo local]", v)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            out.push_str(&format!(
                "     {} -> {} (appName {}, runner {}, {}{})\n        {}\n",
                key,
                titulo,
                if app_name.is_empty() { "-" } else { app_name },
                runner.unwrap_or("-"),
                estado_instalacion,
                ruta_estado,
                estado
            ));
        }
    }

    out
}

/// Nombres de archivo de un directorio (solo el nombre, sin rutas largas).
fn list_images(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Primeras claves de un JSON: sirve para ver si es un objeto indexado por
/// app_name o una lista de juegos.
fn json_summary(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Object(map) => {
            let keys: Vec<&str> = map.keys().take(3).map(|k| k.as_str()).collect();
            format!("objeto ({} entradas, ej: {:?})", map.len(), keys)
        }
        serde_json::Value::Array(arr) => {
            let names: Vec<&str> = arr
                .iter()
                .take(3)
                .filter_map(|g| g.get("app_name").and_then(|n| n.as_str()))
                .collect();
            format!("lista ({} entradas, ej: {:?})", arr.len(), names)
        }
        _ => "otro".to_string(),
    }
}

// ==================== PUNTO DE ENTRADA ====================

/// Devuelve la carátula local del juego, o `None` si el launcher no tiene una.
/// El frontend cae entonces a la del CDN de Steam o al icono de mando.
#[cfg(test)]
#[allow(dead_code)]
pub fn local_cover(launcher: &str, appid: Option<u32>, key: &str) -> Option<String> {
    local_cover_with_runner(launcher, appid, key, None)
}

/// Igual que [`local_cover`], con el runner del launcher (`gog`, `sideload`...).
/// Heronite indexa su arte por `"<runner>:<appName>"`, así que sin el runner
/// solo se encontraría el arte guardado bajo la clave sin prefijo.
pub fn local_cover_with_runner(
    launcher: &str,
    appid: Option<u32>,
    key: &str,
    runner: Option<&str>,
) -> Option<String> {
    match launcher {
        "lutris" => lutris_cover(key),
        "heroic" => heroic_cover(key),
        "heronite" => heronite_cover_with_runner(key, runner),
        // Steam las tiene en el CDN; si es un juego de Steam dentro de otro
        // launcher, se aprovecha el appid como último recurso.
        _ => appid.and_then(|id| lutris_cover(&format!("steam-{id}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rechaza_slugs_con_ruta() {
        // Evita leer archivos arbitrarios si el nombre viene manipulado.
        assert!(lutris_cover("../etc/passwd").is_none());
        assert!(lutris_cover("a/b").is_none());
        assert!(heroic_cover("../../x").is_none());
    }

    #[test]
    fn un_archivo_inexistente_no_da_carátula() {
        assert!(lutris_cover("slug-que-no-existe-en-este-equipo").is_none());
    }

    #[test]
    fn detecta_el_mime_por_la_firma_no_por_la_extension() {
        let dir = std::env::temp_dir().join("thoriapp_cover_test");
        std::fs::create_dir_all(&dir).unwrap();

        // Firma JPEG real pero con extensión .txt
        let jpeg = dir.join("falso.txt");
        std::fs::write(&jpeg, [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]).unwrap();
        assert!(to_data_uri(&jpeg).unwrap_or_default().starts_with("data:image/jpeg;base64,"));

        // HTML disfrazado de .jpg: debe rechazarse
        let falso = dir.join("roto.jpg");
        std::fs::write(&falso, b"<!DOCTYPE html><html>404</html>").unwrap();
        assert!(to_data_uri(&falso).is_none());

        // PNG
        let png = dir.join("ok.png");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
        assert!(to_data_uri(&png).unwrap_or_default().starts_with("data:image/png;base64,"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_archivo_vacio_no_es_una_caratula() {
        let dir = std::env::temp_dir().join("thoriapp_cover_empty");
        std::fs::create_dir_all(&dir).unwrap();
        let vacio = dir.join("vacio.jpg");
        std::fs::write(&vacio, b"").unwrap();
        assert!(to_data_uri(&vacio).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn heroic_encuentra_el_juego_en_un_objeto_anidado() {
        // Forma real de Epic: {"__timestamp": ..., "library": {"<app_name>": {...}}}
        let json = serde_json::json!({
            "__timestamp": 1,
            "library": { "abc123": { "app_name": "abc123", "title": "Juego" } }
        });
        let inner = json.get("library").unwrap();
        assert!(lookup_heroic_entry(inner, "abc123").is_some());
    }

    #[test]
    fn heroic_encuentra_el_juego_en_una_lista() {
        // Forma de GOG/Amazon: lista de objetos con `app_name`.
        let json = serde_json::json!([{ "app_name": "xyz", "title": "Otro" }]);
        assert!(lookup_heroic_entry(&json, "xyz").is_some());
        assert!(lookup_heroic_entry(&json, "no-existe").is_none());
    }

    #[test]
    fn heroic_acepta_app_name_camel_case() {
        let json = serde_json::json!([{ "appName": "camel", "title": "Camel" }]);
        assert!(lookup_heroic_entry(&json, "camel").is_some());
    }

    #[test]
    fn heroic_no_encuentra_lo_que_no_esta() {
        let json = serde_json::json!({ "library": { "uno": {} } });
        let inner = json.get("library").unwrap();
        assert!(lookup_heroic_entry(inner, "dos").is_none());
    }

    /// PNG mínimo con firma válida, para que `to_data_uri` lo acepte.
    fn png_de_prueba() -> [u8; 8] {
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    }

    /// Carpeta temporal exclusiva por test (los tests de Rust corren en
    /// paralelo y compartir carpeta los haría pisarse).
    fn dir_temporal(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("thoriapp_test_{}_{}", nombre, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Escribe un data URI PNG de prueba con bytes distinguibles, para poder
    /// comprobar *qué* archivo se eligió (todos los PNG comparten prefijo).
    fn escribir_png(path: &Path, marca: &str) {
        let mut bytes = png_de_prueba().to_vec();
        bytes.extend_from_slice(marca.as_bytes());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn sha256_coincide_con_el_esquema_de_heroic() {
        // Heronite (y Heroic) nombran la caché `<sha256 hex de la URL exacta>`.
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(sha256_hex("").len(), 64);
    }

    #[test]
    fn la_cache_de_heronite_se_localiza_por_el_hash_de_la_url() {
        // Regresión del bug reportado: los archivos de `images-cache` se llaman
        // como el hash de la URL, no como el juego, así que buscar por título
        // nunca encontraba nada.
        let base = dir_temporal("heronite_hash_url");
        let url = "https://cdn2.steamgriddb.com/grid/abc123.png";
        escribir_png(&heronite_cache_path(&base, url), "grid");

        let uri = heronite_art_to_uri(&base, url).expect("debe leer la caché ya descargada");
        assert!(uri.starts_with("data:image/png;base64,"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn tambien_prueba_la_url_con_los_parametros_de_tarjeta() {
        // De Epic, Heronite no cachea la URL limpia sino `?h=400&resize=1&w=300`.
        let base = dir_temporal("heronite_hash_params");
        let url = "https://cdn1.epicgames.com/item/cover.jpg";
        escribir_png(&heronite_cache_path(&base, &format!("{url}?h=400&resize=1&w=300")), "epic");

        assert!(heronite_art_to_uri(&base, url).is_some());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn el_arte_se_lee_de_art_json_por_runner_y_appname() {
        // art.json indexa por `<runner>:<appName>` (no por el título).
        let base = dir_temporal("heronite_art_json");
        let url = "https://cdn2.steamgriddb.com/grid/soulmask.png";
        escribir_png(&heronite_cache_path(&base, url), "soulmask");
        std::fs::create_dir_all(base.join("local_library")).unwrap();
        std::fs::write(
            base.join("local_library/art.json"),
            serde_json::json!({
                "games": { "sideload:steam_2646460": { "coverUrl": url, "heroUrl": "https://x/hero.jpg" } }
            })
            .to_string(),
        )
        .unwrap();

        let uri = heronite_cover_in(std::slice::from_ref(&base), "steam_2646460", Some("sideload"));
        assert!(uri.unwrap_or_default().starts_with("data:image/png;base64,"));

        // Sin el runner la clave `sideload:` no se encuentra.
        assert!(heronite_cover_in(std::slice::from_ref(&base), "steam_2646460", Some("gog")).is_none());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn el_arte_se_lee_de_metadata_json() {
        // metadata.json (arte descargado de IGDB/SteamGridDB) tiene prioridad
        // sobre art_square y usa el mismo formato `<runner>:<appName>`.
        let base = dir_temporal("heronite_metadata_json");
        let url = "https://images.igdb.com/igdb/image/cover/abc.jpg";
        escribir_png(&heronite_cache_path(&base, url), "meta");
        std::fs::create_dir_all(base.join("local_library")).unwrap();
        std::fs::write(
            base.join("local_library/metadata.json"),
            serde_json::json!({
                "games": { "sideload:Fortnite": { "appName": "Fortnite", "runner": "sideload", "coverUrl": url } }
            })
            .to_string(),
        )
        .unwrap();

        assert!(heronite_cover_in(std::slice::from_ref(&base), "Fortnite", Some("sideload")).is_some());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn art_square_de_heroic_importado_de_epic() {
        // Heronite copia la biblioteca de Heroic: `library` es una lista con
        // `app_name` y `art_square`.
        let base = dir_temporal("heronite_epic_library");
        let url = "https://cdn1.epicgames.com/abc/cover.png";
        escribir_png(&heronite_cache_path(&base, url), "epic2");
        std::fs::create_dir_all(base.join("store_cache")).unwrap();
        std::fs::write(
            base.join("store_cache/legendary_library.json"),
            serde_json::json!({
                "library": [{ "app_name": "abc123", "title": "Juego", "art_square": url }]
            })
            .to_string(),
        )
        .unwrap();

        assert!(heronite_cover_in(std::slice::from_ref(&base), "abc123", Some("legendary")).is_some());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn el_arte_local_del_usuario_se_resuelve_a_su_archivo() {
        // `localart://art/<x>` -> `local_library/art/<x>`.
        let base = dir_temporal("heronite_localart");
        escribir_png(&base.join("local_library/art/mi_portada.png"), "local");
        std::fs::create_dir_all(base.join("local_library")).unwrap();
        std::fs::write(
            base.join("local_library/art.json"),
            serde_json::json!({
                "games": { "sideload:X": { "coverUrl": "localart://art/mi_portada.png" } }
            })
            .to_string(),
        )
        .unwrap();

        assert!(heronite_cover_in(std::slice::from_ref(&base), "X", Some("sideload")).is_some());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn prioridad_arte_propio_antes_que_art_square() {
        // El usuario eligió `coverUrl`; debe ganar al `art_square` de la tienda.
        let base = dir_temporal("heronite_prioridad_arte");
        let propio = "https://cdn2.steamgriddb.com/grid/propio.png";
        let tienda = "https://cdn1.epicgames.com/tienda.png";
        escribir_png(&heronite_cache_path(&base, propio), "propio");
        escribir_png(&heronite_cache_path(&base, tienda), "tienda");
        std::fs::create_dir_all(base.join("local_library")).unwrap();
        std::fs::create_dir_all(base.join("store_cache")).unwrap();
        std::fs::write(
            base.join("local_library/art.json"),
            serde_json::json!({
                "games": { "legendary:juego": { "coverUrl": propio } }
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            base.join("store_cache/legendary_library.json"),
            serde_json::json!({
                "library": [{ "app_name": "juego", "art_square": tienda }]
            })
            .to_string(),
        )
        .unwrap();

        let valores = heronite_art_values_in(std::slice::from_ref(&base), "juego", Some("legendary"));
        assert_eq!(valores.first().map(String::as_str), Some(propio));
        assert!(valores.contains(&tienda.to_string()));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn los_urls_de_arte_ignoran_marcadores_y_rutas_locales() {
        // `fallback` es el marcador de Heronite y `localart:`/`file:` no son
        // URLs descargables.
        assert_eq!(art_url("fallback"), None);
        assert_eq!(art_url("   "), None);
        assert_eq!(art_url("localart://art/x.png"), None);
        assert_eq!(art_url("file:///tmp/x.png"), None);
        assert_eq!(art_url("https://cdn2.steamgriddb.com/grid/a.png"), Some("https://cdn2.steamgriddb.com/grid/a.png"));
        // Rutas relativas sin esquema no se intentan descargar.
        assert_eq!(art_url("art/x.png"), None);
    }

    #[test]
    fn localart_no_permite_salir_de_la_carpeta_de_datos() {
        let base = dir_temporal("heronite_localart_traversal");
        std::fs::create_dir_all(&base).unwrap();
        assert!(localart_path(&base, "localart://../../etc/passwd").is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sin_json_de_arte_no_hay_caratula() {
        // Con solo `images-cache` no se puede saber a qué juego pertenece cada
        // archivo (su nombre es un hash), así que no hay carátula.
        let base = dir_temporal("heronite_sin_json");
        escribir_png(&base.join("images-cache/deadbeef"), "huerfano");
        assert!(heronite_cover_in(std::slice::from_ref(&base), "cualquiera", None).is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rechaza_nombres_que_salen_de_la_carpeta() {
        // La clave no puede escapar del directorio.
        assert!(!is_safe_cover_name("../fuera"));
        assert!(!is_safe_cover_name("a/b"));
        assert!(!is_safe_cover_name(".."));
        assert!(!is_safe_cover_name("   "));
        assert!(is_safe_cover_name("Juego Normal"));
    }

    #[test]
    fn heroic_no_confunde_una_entrada_con_otra() {
        // Dos juegos en la misma caché: cada uno lee su propia URL.
        let base = dir_temporal("heronite_dos_juegos");
        let a = "https://cdn2.steamgriddb.com/grid/a.png";
        let b = "https://cdn2.steamgriddb.com/grid/b.png";
        escribir_png(&heronite_cache_path(&base, a), "aaa");
        escribir_png(&heronite_cache_path(&base, b), "bbb");
        std::fs::create_dir_all(base.join("local_library")).unwrap();
        std::fs::write(
            base.join("local_library/art.json"),
            serde_json::json!({
                "games": {
                    "sideload:A": { "coverUrl": a },
                    "sideload:B": { "coverUrl": b }
                }
            })
            .to_string(),
        )
        .unwrap();

        let ua = heronite_cover_in(std::slice::from_ref(&base), "A", Some("sideload")).unwrap();
        let ub = heronite_cover_in(std::slice::from_ref(&base), "B", Some("sideload")).unwrap();
        assert_ne!(ua, ub);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn busca_por_extension_cuando_no_se_indica() {
        let dir = std::env::temp_dir().join("thoriapp_cover_ext");
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("juego.png");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();

        assert!(find_image_with_extensions(&dir.join("juego")).is_some());
        assert!(find_image_with_extensions(&dir.join("otro")).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
