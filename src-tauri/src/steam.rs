use serde::Serialize;
use std::{env, fs, path::PathBuf};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LibGame {
    pub appid: u32,
    pub name: String,
    /// Carpeta del juego dentro de `steamapps/common`, según el `appmanifest`.
    ///
    /// Es lo único que el `.acf` garantiza sobre el contenido del juego: el nombre de la
    /// carpeta. **No** se deduce ni se lista qué ejecutables hay dentro, porque el
    /// manifiesto no los trae y suponerlos inventaría identidad.
    #[serde(default)]
    pub install_dir: Option<String>,
}

fn steam_roots() -> Vec<PathBuf> {
    let home = PathBuf::from(env::var("HOME").unwrap_or_default());
    vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]
}

fn vdf_value(line: &str, key: &str) -> Option<String> {
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 4 && parts[1].eq_ignore_ascii_case(key) {
        Some(parts[3].to_string())
    } else {
        None
    }
}

/// Carpetas `steamapps` de todas las bibliotecas de Steam instaladas.
pub fn bibliotecas() -> Vec<PathBuf> {
    let mut libs: Vec<PathBuf> = vec![];
    for root in steam_roots() {
        if !root.join("steamapps").exists() {
            continue;
        }
        libs.push(root.join("steamapps"));
        if let Ok(txt) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for l in txt.lines() {
                if let Some(p) = vdf_value(l, "path") {
                    libs.push(PathBuf::from(p).join("steamapps"));
                }
            }
        }
    }
    libs.sort();
    libs.dedup();
    libs
}

/// Catálogo reducido para resolver identidad de sesiones.
///
/// `install_dir` es la pista fuerte: la ruta del ejecutable
/// (`.../steamapps/common/<dir>/...`) se cruza con este campo. Cuando el manifiesto no lo
/// trae, queda `None` y se conserva la incertidumbre en vez de inventarlo.
pub fn catalogo_para_identidad() -> Vec<crate::sessions::JuegoDeBiblioteca> {
    scan_library()
        .into_iter()
        .map(|g| crate::sessions::JuegoDeBiblioteca {
            appid: g.appid,
            nombre: g.name,
            installdir: g.install_dir,
        })
        .collect()
}

pub fn scan_library() -> Vec<LibGame> {
    let mut out: Vec<LibGame> = vec![];
    for lib in bibliotecas() {
        let Ok(rd) = fs::read_dir(&lib) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !(n.starts_with("appmanifest_") && n.ends_with(".acf")) {
                continue;
            }
            let Ok(txt) = fs::read_to_string(e.path()) else { continue };
            let mut appid = None;
            let mut name = None;
            let mut install_dir = None;
            for l in txt.lines() {
                if appid.is_none() {
                    appid = vdf_value(l, "appid").and_then(|v| v.parse::<u32>().ok());
                }
                if name.is_none() {
                    name = vdf_value(l, "name");
                }
                if install_dir.is_none() {
                    install_dir = vdf_value(l, "installdir");
                }
            }
            if let (Some(appid), Some(name)) = (appid, name) {
                let skip = name.starts_with("Proton")
                    || name.starts_with("Steam Linux Runtime")
                    || name.starts_with("Steamworks Common");
                if !skip && !out.iter().any(|g| g.appid == appid) {
                    out.push(LibGame {
                        appid,
                        name,
                        install_dir,
                    });
                }
            }
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

fn get_json(url: &str) -> Result<serde_json::Value, String> {
    // Con tope de tamaño: la respuesta de Steam no la controla la app.
    crate::net::fetch_json(url, std::time::Duration::from_secs(10))
}

pub fn search_store(term: &str) -> Result<Vec<LibGame>, String> {
    let t: String = term.chars().map(|c| if c.is_alphanumeric() { c.to_string() } else { format!("%{:02X}", c as u32 % 256) }).collect();
    let v = get_json(&format!("https://store.steampowered.com/api/storesearch/?term={t}&cc=us&l=english"))?;
    Ok(v["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|i| i["type"] == "app")
                .filter_map(|i| Some(LibGame { appid: i["id"].as_u64()? as u32, name: i["name"].as_str()?.to_string(), install_dir: None }))
                .collect()
        })
        .unwrap_or_default())
}

#[derive(Serialize, Default)]
pub struct GameInfo {
    pub name: Option<String>,
    pub native: Option<bool>,
    pub tier: Option<String>,
    pub score: Option<f64>,
    pub total: Option<u64>,
    pub confidence: Option<String>,
    pub errors: Vec<String>,
}

/// Requisitos de sistema publicados por Steam para un juego.
///
/// **Steam ignora `pc_requirements` si es el único filtro** y responde
/// `{"data":[]}` (39 bytes) aunque el juego sí tenga requisitos publicados. Hay
/// que pedir `filters=basic,pc_requirements` para que devuelva la ficha. Sin
/// `basic` esta función decía "Steam no publica requisitos" en juegos que sí los
/// tienen, que es peor que no enseñar nada.
///
/// El campo llega como objeto `{minimum, recommended}` con el HTML **ya
/// encabezado** (`<strong>Minimum:</strong><ul>...`), así que no se le añade
/// ningún marcador: hacerlo duplicaba el encabezado.
pub fn pc_requirements(appid: u32) -> Vec<crate::requirements::Requirement> {
    // `cc=es&l=spanish` para que Steam devuelva los textos en español: sin esto
    // la tarjeta mezclaba castellano con "55 GB available space". Si un juego no
    // tiene traducción, Steam cae al inglés por su cuenta.
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={appid}&cc=es&l=spanish&filters=basic,pc_requirements"
    );
    let Ok(v) = get_json(&url) else { return Vec::new() };
    let data = &v[appid.to_string()]["data"];
    // `data` puede ser `[]` (array) en vez de un objeto: se trata como "sin datos".
    if data.is_array() {
        return Vec::new();
    }
    let req = &data["pc_requirements"];

    let mut raw = Vec::new();
    for tier in ["minimum", "recommended"] {
        let Some(html) = req[tier].as_str() else { continue };
        // El HTML de Steam trae su propio encabezado; se quita y se re-inyecta el
        // del tier que se está leyendo para que `parse_pc_requirements` sepa a qué
        // bloque pertenece cada línea.
        let body = strip_tier_heading(html);
        let wrapped = format!("<strong>{tier}:</strong>{body}");
        raw.extend(
            crate::requirements::parse_pc_requirements(&wrapped)
                .into_iter()
                .map(|r| crate::requirements::Requirement {
                    tier: tier.to_string(),
                    ..r
                }),
        );
    }
    raw
}

/// Quita el encabezado que Steam mete dentro de cada bloque
/// (`<strong>Minimum:</strong>`, `<strong>Mínimo:</strong>`…).
fn strip_tier_heading(html: &str) -> String {
    let lower = html.to_lowercase();
    for marker in ["minimum:", "mínimo:", "minimo:", "recommended:", "recomendado:"] {
        if let Some(pos) = lower.find(marker) {
            // El encabezado va dentro de una etiqueta; se corta tras el `>` que
            // cierre esa etiqueta, no en medio del HTML.
            let after = pos + marker.len();
            if let Some(close) = html[after..].find('>') {
                return html[after + close + 1..].to_string();
            }
        }
    }
    html.to_string()
}

pub fn game_info(appid: u32) -> GameInfo {
    let mut info = GameInfo::default();
    match get_json(&format!("https://store.steampowered.com/api/appdetails?appids={appid}&filters=basic,platforms")) {
        Ok(v) => {
            let d = &v[appid.to_string()]["data"];
            info.name = d["name"].as_str().map(String::from);
            info.native = d["platforms"]["linux"].as_bool();
        }
        Err(e) => info.errors.push(format!("Steam: {e}")),
    }
    match get_json(&format!("https://www.protondb.com/api/v1/reports/summaries/{appid}.json")) {
        Ok(v) => {
            info.tier = v["tier"].as_str().map(String::from);
            info.score = v["score"].as_f64();
            info.total = v["total"].as_u64();
            info.confidence = v["confidence"].as_str().map(String::from);
        }
        Err(e) => info.errors.push(format!("ProtonDB: {e}")),
    }
    info
}

#[cfg(test)]
mod tests {
    use super::strip_tier_heading;

    /// HTML real de Steam (`pc_requirements.minimum`): trae su propio encabezado
    /// dentro del campo, con `<br>` y `<ul class="bb_ul">`.
    const REAL_MINIMUM: &str = "<strong>Minimum:</strong><br><ul class=\"bb_ul\"><li>Requires a 64-bit processor and operating system<br></li><li><strong>OS:</strong> Windows 10<br></li><li><strong>Processor:</strong> INTEL CORE I5-8400 or AMD RYZEN 3 3300X<br></li><li><strong>Memory:</strong> 12 GB RAM<br></li><li><strong>Graphics:</strong> NVIDIA GEFORCE GTX 1060 3 GB or AMD RADEON RX 580 4 GB<br></li><li><strong>DirectX:</strong> Version 12<br></li><li><strong>Storage:</strong> 60 GB available space<br></li></ul>";

    #[test]
    fn quita_el_encabezado_que_ya_trae_steam() {
        let body = strip_tier_heading(REAL_MINIMUM);
        // El encabezado duplicado hacia que el parser viera el tier mal.
        assert!(!body.to_lowercase().starts_with("<strong>minimum"));
        assert!(body.contains("Windows 10"));
    }

    #[test]
    fn el_html_sin_encabezado_se_deja_igual() {
        let sin = "<ul><li><strong>OS:</strong> Windows 10</li></ul>";
        assert_eq!(strip_tier_heading(sin), sin);
    }

    #[test]
    fn el_html_real_produce_los_campos_esperados() {
        let body = strip_tier_heading(REAL_MINIMUM);
        let wrapped = format!("<strong>minimum:</strong>{body}");
        let parsed = crate::requirements::parse_pc_requirements(&wrapped);

        let kinds: Vec<&str> = parsed.iter().map(|r| r.kind.as_str()).collect();
        assert!(kinds.contains(&"os"));
        assert!(kinds.contains(&"cpu"));
        assert!(kinds.contains(&"ram"));
        assert!(kinds.contains(&"gpu"));
        assert!(kinds.contains(&"directx"));
        assert!(kinds.contains(&"storage"));

        let ram = parsed.iter().find(|r| r.kind == "ram").unwrap();
        assert_eq!(ram.text, "12 GB RAM");
        assert_eq!(ram.tier, "minimum");
    }

    #[test]
    fn la_linea_sin_dos_puntos_no_rompe() {
        // "Requires a 64-bit processor..." no tiene `:` y no debe clasificarse.
        let body = strip_tier_heading(REAL_MINIMUM);
        let wrapped = format!("<strong>minimum:</strong>{body}");
        let parsed = crate::requirements::parse_pc_requirements(&wrapped);
        assert!(!parsed.iter().any(|r| r.text.contains("64-bit")));
    }
}

/// `buildid` del `appmanifest_<appid>.acf`, que identifica la **versión instalada** del
/// juego.
///
/// Sirve para saber si dos pruebas se hicieron con la misma versión: tras un parche, una
/// mejora de rendimiento puede venir del juego y no del ajuste probado.
///
/// `Ok(None)` = el juego no está instalado en ninguna biblioteca (dato legítimo: puede
/// probarse con un juego ya desinstalado). `Err` = hay un `appmanifest` pero no se pudo
/// leer, que es distinto de «no existe».
pub fn build_id(appid: u32) -> Result<Option<String>, String> {
    if appid == 0 {
        // 0 no es un AppID real: es el que llevan los CSV cuando MangoHud no lo escribe.
        return Ok(None);
    }
    let nombre = format!("appmanifest_{}.acf", appid);
    let mut encontrado = false;
    for lib in bibliotecas() {
        let ruta = lib.join(&nombre);
        if !ruta.exists() {
            continue;
        }
        encontrado = true;
        let txt = match fs::read_to_string(&ruta) {
            Ok(t) => t,
            Err(e) => return Err(format!("no se pudo leer {}: {}", ruta.display(), e)),
        };
        for l in txt.lines() {
            if let Some(v) = vdf_value(l, "buildid") {
                return Ok(Some(v));
            }
        }
    }
    if encontrado {
        // El archivo existe pero no trae `buildid`: Steam no lo ha rellenado (instalación
        // recién empezada). No se inventa una versión.
        Ok(None)
    } else {
        Ok(None)
    }
}
