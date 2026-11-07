use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Serialize, Clone, Debug)]
pub struct ExternalGame {
    pub name: String,
    pub appid: Option<u32>,
    pub launcher: String,
    pub install_path: Option<String>,
    pub runner: Option<String>,
    /// Clave con la que el launcher guarda la carátula en disco (slug de Lutris,
    /// app_name de Heroic, clave del library.json de Heronite). Necesaria para
    /// localizar su portada real, incluidas las que el usuario cambió a mano.
    pub cover_key: Option<String>,
}

// ==================== HELPERS ====================

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// true si el string es una ruta no vacía que existe en disco
fn path_exists(p: &Option<String>) -> bool {
    p.as_deref()
        .map(|s| !s.is_empty() && Path::new(s).exists())
        .unwrap_or(false)
}

/// Lee un archivo JSON de forma segura
fn read_json(path: &Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Extrae un string de un valor JSON
fn json_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

// ==================== LUTRIS ====================

/// Encuentra el directorio de datos de Lutris (nativo o Flatpak) que contiene pga.db
fn find_lutris_dir() -> Option<PathBuf> {
    let home = home_dir();
    [
        home.join(".local/share/lutris"),
        home.join(".var/app/net.lutris.Lutris/data/lutris"),
    ]
    .into_iter()
    .find(|p| p.join("pga.db").exists())
}

/// Lee la ruta del exe desde el .yml de configuración del juego (si existe)
fn read_lutris_exe(yml: &Path) -> Option<String> {
    let content = fs::read_to_string(yml).ok()?;
    content.lines().find_map(|line| {
        line.trim()
            .strip_prefix("exe:")
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
    })
}

/// Escanea juegos de Lutris.
/// La fuente de verdad es pga.db (installed = 1), NO los .yml de games/,
/// porque Lutris deja .yml huérfanos al desinstalar.
/// Nombre con el que Lutris guarda la carátula de un juego.
///
/// Se juega con varias formas porque en disco conviven:
///   - juegos de servicio: `configpath` es el slug limpio (`soulmask`)
///   - juegos añadidos a mano: `configpath` es `soulmask-1700000000`
///   - la imagen real acaba siendo `soulmask.jpg` (slug) o `<id>.jpg`
///
/// Se devuelven todas separadas por `|` y el lado Rust prueba cada una.
fn build_lutris_cover_key(name: &str, configpath: Option<&str>, db_id: Option<i64>) -> String {
    let mut keys: Vec<String> = Vec::new();

    if let Some(cp) = configpath.filter(|c| !c.is_empty()) {
        keys.push(cp.to_string());
        // `slug-timestamp` -> `slug`. Se exige un timestamp real (más de 8
        // dígitos) para no destrozar títulos que acaban en número: `half-life-2`
        // es el slug completo, no `half-life` + timestamp.
        if let Some((slug, stamp)) = cp.rsplit_once('-') {
            let es_timestamp = stamp.len() >= 9 && stamp.chars().all(|c| c.is_ascii_digit());
            if !slug.is_empty() && es_timestamp {
                keys.push(slug.to_string());
            }
        }
    }

    // Slug derivado del nombre, con las mismas reglas que `lutris.util.strings.slugify`
    let derived = slugify(name);
    if !derived.is_empty() {
        keys.push(derived);
    }

    if let Some(id) = db_id {
        keys.push(id.to_string());
    }

    // Sin duplicados y en orden de preferencia (`dedup` solo quita
    // consecutivos, y "half-life-2" puede generar "half-life-2" dos veces).
    let mut seen = std::collections::HashSet::new();
    keys.retain(|k| seen.insert(k.clone()));
    keys.join("|")
}

/// Réplica simplificada de `slugify` de Lutris: minúsculas, sin acentos, los
/// caracteres no alfanuméricos fuera y espacios convertidos en guiones.
fn slugify(value: &str) -> String {
    let ascii: String = value
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || *c == '-')
        .collect();

    let mut slug = String::new();
    let mut last_dash = false;
    for c in ascii.chars() {
        if c.is_whitespace() || c == '-' {
            if !slug.is_empty() && !last_dash {
                slug.push('-');
                last_dash = true;
            }
        } else {
            slug.push(c);
            last_dash = false;
        }
    }
    slug.trim_matches('-').to_string()
}

pub fn scan_lutris_games() -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let Some(lutris_dir) = find_lutris_dir() else {
        println!("[Lutris] No se encontró pga.db (nativo ni Flatpak)");
        return games;
    };

    let db_path = lutris_dir.join("pga.db");
    println!("[Lutris] Leyendo: {:?}", db_path);

    // Solo lectura: nunca modifica ni bloquea la base de datos de Lutris
    let conn = match Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            println!("[Lutris] ⚠ Error abriendo la base de datos: {}", e);
            return games;
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT name, runner, directory, configpath, service, service_id, id
         FROM games WHERE installed = 1",
    ) {
        Ok(s) => s,
        Err(e) => {
            println!("[Lutris] ⚠ Error en la consulta: {}", e);
            return games;
        }
    };

    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?, // name
            row.get::<_, Option<String>>(1)?, // runner
            row.get::<_, Option<String>>(2)?, // directory
            row.get::<_, Option<String>>(3)?, // configpath
            row.get::<_, Option<String>>(4)?, // service
            row.get::<_, Option<String>>(5)?, // service_id
            row.get::<_, Option<i64>>(6)?,    // id
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            println!("[Lutris] ⚠ Error leyendo filas: {}", e);
            return games;
        }
    };

    for row in rows.flatten() {
        let (name, runner, directory, configpath, service, service_id, db_id) = row;

        let Some(name) = name.filter(|n| !n.trim().is_empty()) else {
            continue;
        };

        // Si tiene configpath, su .yml debe existir; si no, el registro está roto
        let yml = configpath
            .as_deref()
            .filter(|c| !c.is_empty())
            .map(|c| lutris_dir.join("games").join(format!("{}.yml", c)));

        if let Some(y) = &yml {
            if !y.exists() {
                println!("[Lutris] Ignorado (sin .yml): {}", name);
                continue;
            }
        }

        // appid de Steam: service_id, o el número dentro de "steam-<appid>-<timestamp>"
        let is_steam = service.as_deref() == Some("steam") || runner.as_deref() == Some("steam");
        let appid = if is_steam {
            service_id
                .as_deref()
                .and_then(|id| id.parse::<u32>().ok())
                .or_else(|| {
                    configpath
                        .as_deref()
                        .and_then(|c| c.strip_prefix("steam-"))
                        .and_then(|c| c.split('-').next())
                        .and_then(|id| id.parse::<u32>().ok())
                })
        } else {
            None
        };

        // `directory` viene vacío en casi todos los juegos de Wine: se usa el exe del .yml
        let install_path = directory
            .filter(|d| !d.is_empty())
            .or_else(|| yml.as_deref().and_then(read_lutris_exe));

        // Nombre de la carátula en disco. No basta con `configpath`: para un
        // juego añadido a mano el .yml se llama `<slug>-<timestamp>`, mientras
        // que la imagen es `<slug>.jpg`. Se prueban las dos formas más el id
        // numérico de la base de datos, que Lutris también usa como nombre.
        let cover_key = build_lutris_cover_key(&name, configpath.as_deref(), db_id);

        println!("[Lutris] ✓ {} (AppID: {:?}) [carátula: {}]", name, appid, cover_key);

        games.push(ExternalGame {
            name,
            appid,
            launcher: "lutris".to_string(),
            install_path,
            runner,
            cover_key: Some(cover_key),
        });
    }

    println!("[Lutris] Total juegos encontrados: {}", games.len());
    games
}

// ==================== HEROIC ====================

/// Encuentra el directorio de Heroic (nativo o Flatpak)
fn find_heroic_dir() -> Option<PathBuf> {
    let home = home_dir();
    [
        home.join(".config/heroic"),
        home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

/// Escanea juegos de Epic instalados en Heroic
/// Usa legendaryConfig/legendary/installed.json (archivo central de juegos instalados)
fn scan_heroic_epic_games(dir: &Path) -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let installed_file = dir.join("legendaryConfig/legendary/installed.json");
    println!("[Heroic] Buscando Epic en: {:?}", installed_file);

    if !installed_file.exists() {
        println!("[Heroic] ⚠ installed.json no existe");
        return games;
    }

    let Some(value) = read_json(&installed_file) else {
        println!("[Heroic] ⚠ Error parseando installed.json");
        return games;
    };

    let Some(map) = value.as_object() else {
        println!("[Heroic] ⚠ installed.json no es un objeto");
        return games;
    };

    for (id, game_data) in map {
        // Saltar DLCs
        if game_data
            .get("is_dlc")
            .and_then(|d| d.as_bool())
            .unwrap_or(false)
        {
            continue;
        }

        let title = json_str(game_data, "title").unwrap_or_else(|| id.clone());
        let install_path = json_str(game_data, "install_path");

        // Si la carpeta se borró a mano, el registro queda huérfano
        if !path_exists(&install_path) {
            println!("[Heroic] Ignorado (carpeta no existe): {}", title);
            continue;
        }

        println!("[Heroic] ✓ Epic: {} (id: {})", title, id);

        games.push(ExternalGame {
            name: title,
            appid: None,
            launcher: "heroic".to_string(),
            install_path,
            runner: Some(format!("epic-{}", id)),
            cover_key: Some(id.clone()),
        });
    }

    println!("[Heroic] Total Epic: {}", games.len());
    games
}

/// Carga el mapa app_name -> install_path de los juegos GOG realmente instalados.
/// Heroic lo guarda en gog_store/installed.json ({ "installed": [ {appName, install_path}, ... ] }).
fn load_gog_installed(dir: &Path) -> Option<HashMap<String, String>> {
    let file = dir.join("gog_store/installed.json");
    if !file.exists() {
        return None;
    }

    let value = read_json(&file)?;
    let arr = value
        .get("installed")
        .and_then(|a| a.as_array())
        .or_else(|| value.as_array())?;

    let mut map = HashMap::new();
    for item in arr {
        let id = json_str(item, "appName").or_else(|| json_str(item, "app_name"));
        let path = json_str(item, "install_path");
        if let (Some(id), Some(path)) = (id, path) {
            map.insert(id, path);
        }
    }
    Some(map)
}

/// Escanea juegos de GOG instalados en Heroic
fn scan_heroic_gog_games(dir: &Path) -> Vec<ExternalGame> {
    let mut games = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Si existe installed.json es la fuente de verdad de lo instalado
    let installed_map = load_gog_installed(dir);

    // La librería (títulos) puede estar en varias ubicaciones según la versión
    let gog_paths = [
        dir.join("gog_store/library.json"),
        dir.join("gogStore/library.json"),
        dir.join("GOGGames/library.json"),
        dir.join("store/gog/library.json"),
    ];

    for gog_file in gog_paths {
        if !gog_file.exists() {
            continue;
        }

        println!("[Heroic] Buscando GOG en: {:?}", gog_file);

        let Some(value) = read_json(&gog_file) else {
            continue;
        };

        // Puede ser un array directo o un objeto con clave "games"
        let games_array = if let Some(arr) = value.as_array() {
            arr.clone()
        } else if let Some(arr) = value.get("games").and_then(|g| g.as_array()) {
            arr.clone()
        } else {
            continue;
        };

        for game_data in &games_array {
            let Some(title) = json_str(game_data, "title") else {
                continue;
            };
            let app_name = json_str(game_data, "app_name").unwrap_or_default();

            if !seen.insert(app_name.clone()) {
                continue; // ya visto en otra ubicación
            }

            // Ruta de instalación: installed.json manda; si no existe, se usa la propia librería
            let install_path = match &installed_map {
                Some(map) => match map.get(&app_name) {
                    Some(p) => Some(p.clone()),
                    None => continue, // en la librería pero no instalado
                },
                None => {
                    // Sin installed.json, exigir is_installed == true explícito
                    let is_installed = game_data
                        .get("is_installed")
                        .and_then(|i| i.as_bool())
                        .unwrap_or(false);
                    if !is_installed {
                        continue;
                    }
                    json_str(game_data, "install_path")
                        .or_else(|| json_str(game_data, "folder_name"))
                }
            };

            if !path_exists(&install_path) {
                println!("[Heroic] Ignorado (carpeta no existe): {}", title);
                continue;
            }

            println!("[Heroic] ✓ GOG: {} (app: {})", title, app_name);

            games.push(ExternalGame {
                name: title,
                appid: None,
                launcher: "heroic".to_string(),
                install_path,
                runner: Some(format!("gog-{}", app_name)),
                cover_key: Some(app_name.clone()),
            });
        }
    }

    if games.is_empty() {
        println!("[Heroic] ⚠ No se encontraron juegos de GOG");
    } else {
        println!("[Heroic] Total GOG: {}", games.len());
    }

    games
}

/// Convierte una entrada de Amazon (objeto JSON) en un ExternalGame, si es válida
fn amazon_entry(id: &str, game_data: &serde_json::Value) -> Option<ExternalGame> {
    let title = json_str(game_data, "title")?;
    let install_path = json_str(game_data, "install_path").or_else(|| json_str(game_data, "path"));

    if !path_exists(&install_path) {
        println!("[Heroic] Ignorado (carpeta no existe): {}", title);
        return None;
    }

    println!("[Heroic] ✓ Amazon: {} (id: {})", title, id);

    Some(ExternalGame {
        name: title,
        appid: None,
        launcher: "heroic".to_string(),
        install_path,
        runner: Some(format!("amazon-{}", id)),
        cover_key: Some(id.to_string()),
    })
}

/// Escanea juegos de Amazon instalados en Heroic
fn scan_heroic_amazon_games(dir: &Path) -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let amazon_paths = [
        dir.join("nile_config/nile/installed.json"),
        dir.join("amazonStore/library.json"),
        dir.join("store/amazon/library.json"),
    ];

    for amazon_file in amazon_paths {
        if !amazon_file.exists() {
            continue;
        }

        println!("[Heroic] Buscando Amazon en: {:?}", amazon_file);

        let Some(value) = read_json(&amazon_file) else {
            continue;
        };

        // Puede ser un objeto (como installed.json de Epic) o un array
        if let Some(map) = value.as_object() {
            for (id, game_data) in map {
                if let Some(g) = amazon_entry(id, game_data) {
                    games.push(g);
                }
            }
        } else if let Some(arr) = value.as_array() {
            for game_data in arr {
                let id = json_str(game_data, "id")
                    .or_else(|| json_str(game_data, "app_name"))
                    .unwrap_or_default();
                if let Some(g) = amazon_entry(&id, game_data) {
                    games.push(g);
                }
            }
        }
    }

    if games.is_empty() {
        println!("[Heroic] ⚠ No se encontraron juegos de Amazon");
    } else {
        println!("[Heroic] Total Amazon: {}", games.len());
    }

    games
}

/// Escanea juegos sideloaded en Heroic
fn scan_heroic_sideload_games(dir: &Path) -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let sideload_file = dir.join("sideload_apps/library.json");
    println!("[Heroic] Buscando sideloaded en: {:?}", sideload_file);

    if !sideload_file.exists() {
        println!("[Heroic] ⚠ sideload_apps/library.json no existe");
        return games;
    }

    let Some(value) = read_json(&sideload_file) else {
        println!("[Heroic] ⚠ Error parseando sideload library.json");
        return games;
    };

    // La estructura es { "games": [...] }
    let Some(games_array) = value.get("games").and_then(|a| a.as_array()) else {
        println!("[Heroic] ⚠ No se encontró array 'games' en sideload");
        return games;
    };

    for game_data in games_array {
        // Solo incluir juegos instalados
        if let Some(installed) = game_data.get("is_installed") {
            if !installed.as_bool().unwrap_or(true) {
                continue;
            }
        }

        let id = json_str(game_data, "app_name").unwrap_or_default();
        let title = json_str(game_data, "title").unwrap_or_else(|| id.clone());
        let install_path = json_str(game_data, "folder_name");

        // Si la ruta es absoluta y ya no existe, el juego se borró a mano
        if let Some(p) = &install_path {
            let p = Path::new(p);
            if p.is_absolute() && !p.exists() {
                println!("[Heroic] Ignorado (carpeta no existe): {}", title);
                continue;
            }
        }

        println!("[Heroic] ✓ Sideloaded: {} (id: {})", title, id);

        games.push(ExternalGame {
            name: title,
            appid: None,
            launcher: "heroic".to_string(),
            install_path,
            runner: Some("sideloaded".to_string()),
            cover_key: Some(id),
        });
    }

    println!("[Heroic] Total Sideloaded: {}", games.len());
    games
}

/// Función principal para escanear todos los juegos de Heroic
pub fn scan_heroic_games() -> Vec<ExternalGame> {
    let mut all_games = Vec::new();

    let Some(heroic_dir) = find_heroic_dir() else {
        let home = home_dir();
        println!("[Heroic] ✗ No se encontró la carpeta de Heroic");
        println!("[Heroic]   Buscado en:");
        println!("[Heroic]   - {}/.config/heroic", home.display());
        println!(
            "[Heroic]   - {}/.var/app/com.heroicgameslauncher.hgl/config/heroic",
            home.display()
        );
        return all_games;
    };

    println!("[Heroic] ============================================");
    println!("[Heroic] Directorio encontrado: {:?}", heroic_dir);
    println!("[Heroic] ============================================");

    all_games.extend(scan_heroic_epic_games(&heroic_dir));
    all_games.extend(scan_heroic_gog_games(&heroic_dir));
    all_games.extend(scan_heroic_amazon_games(&heroic_dir));
    all_games.extend(scan_heroic_sideload_games(&heroic_dir));

    println!("[Heroic] ============================================");
    println!("[Heroic] Total juegos de Heroic: {}", all_games.len());
    println!("[Heroic] ============================================");

    all_games
}

// ==================== HERONITE ====================

/// Entradas de `library.json`, con la clave con la que Heronite las indexa.
///
/// El formato ha cambiado entre versiones: las nuevas guardan un objeto
/// `{"games": {"<clave>": {...}}}` y las antiguas un array
/// `{"games": [{...}]}`, donde la clave es el `appName` del propio juego. Sin
/// soportar los dos, el `is_installed` de las entradas del array nunca se leía
/// y los juegos desinstalados seguían apareciendo.
fn heronite_entries(games: serde_json::Value) -> Vec<(String, serde_json::Value)> {
    match games {
        serde_json::Value::Object(map) => map.into_iter().collect(),
        serde_json::Value::Array(list) => list
            .into_iter()
            .map(|entry| {
                let key = entry
                    .get("appName")
                    .or_else(|| entry.get("app_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                (key, entry)
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[derive(Deserialize, Debug)]
struct HeroniteGame {
    title: String,
    // Puede venir como string o como número según la versión. Las fuentes de
    // Heronite lo escriben en snake_case (`steam_app_id`, como `GameInfo`);
    // `local_library` y el JSON de Heroic usan camelCase.
    #[serde(rename = "steam_app_id", alias = "steamAppId", default)]
    steam_app_id: Option<serde_json::Value>,
    #[serde(default)]
    runner: Option<String>,
    #[serde(rename = "launcher_args", alias = "launcherArgs", default)]
    launcher_args: Option<String>,
    /// Identificador interno del juego en Heronite (`5WzVLqXjCNJWsvmPCz4M4N`).
    /// Es el nombre que usan sus archivos de arte.
    // Ojo: `sideload_apps/library.json` y las cachés escriben `app_name`
    // (snake_case, el nombre de `GameInfo`); `local_library` usa `appName`.
    // Leyendo solo `appName` el valor salía siempre `None` y la clave de
    // carátula quedaba vacía.
    #[serde(rename = "app_name", alias = "appName", default)]
    app_name: Option<String>,
    /// Heronite no borra la entrada al desinstalar: la marca `false` para
    /// conservar horas y estado. Sin leerlo aparecen juegos ya desinstalados.
    #[serde(rename = "is_installed", default)]
    is_installed: Option<bool>,
    /// Carpeta donde está instalado. Heronite solo la comprueba para las
    /// tiendas (Epic/GOG/Amazon); si el usuario borró el juego por fuera, el
    /// JSON sigue diciendo `is_installed: true` y sin este `stat` el juego
    /// aparece como instalado.
    #[serde(default)]
    install: Option<serde_json::Value>,
    #[serde(rename = "folder_name", default)]
    folder_name: Option<String>,
}

impl HeroniteGame {
    /// La carpeta de instalación que guarda el launcher, si la hay.
    fn install_folder(&self) -> Option<&str> {
        let field = |key: &str| {
            self.install
                .as_ref()
                .and_then(|i| i.get(key))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|p| !p.is_empty())
        };
        // `install_path` es lo que guardan las tiendas; los juegos añadidos a
        // mano solo tienen el ejecutable, y su carpeta es la de al lado.
        field("install_path")
            .or_else(|| self.folder_name.as_deref().map(str::trim).filter(|p| !p.is_empty()))
            .or_else(|| field("executable"))
    }

    /// El ejecutable concreto que lanza Heronite, si lo hay.
    ///
    /// Hace falta aparte de [`Self::install_folder`]: en `sideload_apps` el
    /// `install_path` es la carpeta (`/home/thor/Descargas`), así que solo el
    /// ejecutable dice que el "juego" es en realidad `EAappInstaller.exe`.
    fn executable(&self) -> Option<&str> {
        self.install
            .as_ref()
            .and_then(|i| i.get("executable"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|p| !p.is_empty())
    }
}

fn parse_app_id(v: &Option<serde_json::Value>) -> Option<u32> {
    match v.as_ref()? {
        serde_json::Value::String(s) => s.trim().parse::<u32>().ok(),
        serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

/// Juegos que el usuario quitó de la Colección de Heronite: se guardan en
/// `local_library/launcher-imports.json` como `removed: {"<launcher>": [...]}`
/// (y los de las tiendas de clientes en `local_library/clients.json`).
/// Heronite los deja fuera de su biblioteca, así que aquí también.
/// `true` si la ruta que guarda Heronite sigue en el disco.
///
/// Para un ejecutable vale con que exista el archivo; para una carpeta, con
/// que exista el directorio. Las rutas con `~` se expanden antes de mirar.
fn install_target_exists(raw: &str) -> bool {
    let expanded = if let Some(rest) = raw.strip_prefix("~/") {
        home_dir().join(rest)
    } else {
        PathBuf::from(raw)
    };
    expanded.exists()
}

/// Runners de tienda: los que Heronite sincroniza desde un cliente externo
/// (`legendary` = Epic, `gog`, `nile` = Amazon). Solo para ellos tiene sentido
/// mirar si la carpeta de instalación desapareció.
fn is_store_runner(runner: Option<&str>) -> bool {
    matches!(runner, Some("legendary" | "gog" | "nile" | "zoom"))
}

/// Nombres de ejecutable que son instaladores o clientes de tienda, no juegos.
///
/// Un juego añadido a mano puede apuntar a cualquier cosa, y a veces se añade el
/// instalador del launcher (`EAappInstaller.exe`) en vez de un juego. Se comparan
/// en minúsculas y sin ruta: solo el nombre del archivo.
const NO_ES_UN_JUEGO: &[&str] = &[
    // Clientes de tienda
    "eaapp.exe",
    "eadesktop.exe",
    "origin.exe",
    "steam.exe",
    "steam.sh",
    "steam",
    "epicgameslauncher.exe",
    "galaxyclient.exe",
    "battle.net.exe",
    "ubisoftconnect.exe",
    "socialclubhelper.exe",
    "lutris",
    "heroic",
    "bottles",
    "faugus-launcher",
    // Instaladores (terminan en `installer.exe` o son el conocido de EA)
    "eaappinstaller.exe",
    "eadesktopinstaller.exe",
    "origininstaller.exe",
    "epicgameslauncherinstaller.exe",
    "battle.net-setup.exe",
    "ubisoftconnectinstaller.exe",
];

/// `true` si el ejecutable del juego es en realidad un launcher o un instalador.
///
/// Heronite no lo filtra (los muestra en su Colección), pero en Thoriapp un
/// "juego" que abre el instalador de EA no sirve: no se le puede aplicar un
/// perfil ni lanzarlo.
fn is_not_a_game_executable(raw: &str) -> bool {
    let path = Path::new(raw.trim());
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let name = name.to_lowercase();

    NO_ES_UN_JUEGO.contains(&name.as_str())
        // Un instalador cualquiera (`*installer.exe`, `*setup.exe`).
        || (name.ends_with(".exe") && (name.contains("installer") || name.ends_with("setup.exe")))
}

/// `dir` es la carpeta `local_library` de Heronite.
fn heronite_removed_apps(local_library: &Path) -> HashSet<String> {
    let mut removed = HashSet::new();
    for file in ["launcher-imports.json", "clients.json"] {
        let Ok(text) = fs::read_to_string(local_library.join(file)) else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let Some(groups) = json.get("removed").and_then(|r| r.as_object()) else { continue };
        for apps in groups.values() {
            for app in apps.as_array().into_iter().flatten() {
                if let Some(name) = app.as_str() {
                    removed.insert(name.to_string());
                }
            }
        }
    }
    removed
}

/// Juegos que Heronite tiene en `library.json` pero marcados como
/// desinstalados. Se guardan para poder avisar al usuario en vez de que
/// sospechen que la app les ha perdido juegos.
static HERONITE_DESINSTALADOS: AtomicUsize = AtomicUsize::new(0);

/// Cuántos juegos de Heronite se descartaron por estar desinstalados en el
/// último escaneo.
pub fn heronite_desinstalados() -> usize {
    HERONITE_DESINSTALADOS.load(Ordering::Relaxed)
}

/// Convierte una entrada de `library.json` en un juego, o `None` si no debe
/// listarse.
///
/// Heronite no borra la entrada al desinstalar: la deja en `library.json` con
/// `is_installed: false` para conservar horas y estado. Sin este filtro la
/// biblioteca mostraba juegos ya desinstalados (o que nunca estuvieron
/// instalados) como si se pudieran lanzar.
#[cfg(test)]
fn heronite_game_from_entry(key: &str, raw: serde_json::Value) -> Option<ExternalGame> {
    heronite_game_from_entry_removed(key, raw, &HashSet::new())
}

/// Igual, descartando además los juegos que el usuario quitó de la Colección.
fn heronite_game_from_entry_removed(
    key: &str,
    raw: serde_json::Value,
    removed: &HashSet<String>,
) -> Option<ExternalGame> {
    let game: HeroniteGame = match serde_json::from_value(raw) {
        Ok(g) => g,
        Err(e) => {
            println!("[Heronite] Ignorada entrada '{}': {}", key, e);
            return None;
        }
    };

    if game.is_installed == Some(false) {
        println!("[Heronite] - {} descartado: is_installed = false", game.title);
        HERONITE_DESINSTALADOS.fetch_add(1, Ordering::Relaxed);
        return None;
    }

    // Heronite solo comprueba que la carpeta siga existiendo en las tiendas
    // (Epic, GOG, Amazon). Un juego borrado por fuera —desde Steam o a mano—
    // conserva `is_installed: true` en su JSON; el disco es la verdad.
    if let (Some(folder), true) = (game.install_folder(), game.is_installed == Some(true)) {
        if is_store_runner(game.runner.as_deref()) && !install_target_exists(folder) {
            println!(
                "[Heronite] - {} descartado: su carpeta ya no existe ({})",
                game.title, folder
            );
            HERONITE_DESINSTALADOS.fetch_add(1, Ordering::Relaxed);
            return None;
        }
    }

    // Un instalador o el propio cliente de la tienda no es un juego: no se
    // puede lanzar ni tiene perfiles. Heronite sí los lista; aquí no aportan.
    let ejecutable = game.executable().or_else(|| game.install_folder());
    if ejecutable.is_some_and(is_not_a_game_executable) {
        println!("[Heronite] - {} descartado: su ejecutable es un instalador o un launcher", game.title);
        HERONITE_DESINSTALADOS.fetch_add(1, Ordering::Relaxed);
        return None;
    }

    // El usuario lo quitó de la Colección de Heronite: tampoco aquí.
    let app_name = game.app_name.as_deref().unwrap_or(key);
    if removed.contains(app_name) || removed.contains(key) {
        println!("[Heronite] - {} descartado: quitado de la Colección", game.title);
        HERONITE_DESINSTALADOS.fetch_add(1, Ordering::Relaxed);
        return None;
    }

    let appid = parse_app_id(&game.steam_app_id);

    println!("[Heronite] ✓ {} (AppID: {:?})", game.title, appid);

    Some(ExternalGame {
        name: game.title.clone(),
        appid,
        launcher: "heronite".to_string(),
        install_path: game.launcher_args,
        runner: game.runner,
        // Se envían varias candidatas separadas por `|`: el `appName`
        // interno, la clave del library.json y el título (Heronite también
        // nombra el arte por título en algunos casos).
        cover_key: Some(
            [game.app_name.as_deref(), Some(key), Some(game.title.as_str())]
                .into_iter()
                .flatten()
                .filter(|k| !k.trim().is_empty())
                .collect::<Vec<_>>()
                .join("|"),
        ),
    })
}

/// Juegos que Heronite muestra en su Colección, leyendo las **fuentes reales**.
///
/// `local_library/library.json` NO es la biblioteca: es un fichero de
/// metadatos (título, runner, appName, argumentos) que ni siquiera tiene
/// `is_installed`. Los juegos están en:
///
/// - `sideload_apps/library.json`    → juegos locales/importados (Steam, Lutris…)
/// - `store_cache/legendary_library.json` → Epic
/// - `store_cache/gog_library.json`       → GOG
/// - `store_cache/nile_library.json`      → Amazon
///
/// Es el mismo conjunto que arma `Library::load` en Heronite. Leer solo
/// `local_library/library.json` mostraba entradas fantasma: juegos borrados
/// cuya metadata sobrevivía (Fortnite, NBA 2K21, AION 2…).
fn heronite_library_files(data_dir: &Path) -> Vec<PathBuf> {
    ["sideload_apps/library.json", "store_cache/legendary_library.json", "store_cache/gog_library.json", "store_cache/nile_library.json"]
        .iter()
        .map(|rel| data_dir.join(rel))
        .filter(|p| p.is_file())
        .collect()
}

/// Lee una fuente y devuelve sus entradas, aceptando objeto (`games`) y array
/// (`library` / `games`), que es como escriben las distintas versiones.
fn heronite_store_entries(path: &Path) -> Vec<(String, serde_json::Value)> {
    let Ok(text) = fs::read_to_string(path) else {
        println!("[Heronite] ⚠ No se pudo leer {}", path.display());
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        println!("[Heronite] ⚠ JSON inválido en {}", path.display());
        return Vec::new();
    };
    let Some(pool) = json.get("games").or_else(|| json.get("library")) else {
        return Vec::new();
    };
    heronite_entries(pool.clone())
}

pub fn scan_heronite_games() -> Vec<ExternalGame> {
    HERONITE_DESINSTALADOS.store(0, Ordering::Relaxed);
    let data_dir = home_dir().join(".local/share/heronite");

    // Metadata de presentación (título/runner) para los juegos cuyo fichero de
    // tienda no los trae.
    let meta = heronite_local_meta(&data_dir);
    let removed = heronite_removed_apps(&data_dir.join("local_library"));

    let mut games = Vec::new();
    for file in heronite_library_files(&data_dir) {
        println!("[Heronite] Escaneando: {:?}", file);
        for (key, raw) in heronite_store_entries(&file) {
            // La metadata manda para el título: es la que el usuario ve.
            let merged = heronite_merge_meta(raw, &meta);
            if let Some(game) = heronite_game_from_entry_removed(&key, merged, &removed) {
                games.push(game);
            }
        }
    }

    println!(
        "[Heronite] {} juegos, {} desinstalados ocultos",
        games.len(),
        heronite_desinstalados()
    );
    games
}

/// `local_library/library.json` por `appName`, para completar título y runner.
fn heronite_local_meta(data_dir: &Path) -> HashMap<String, serde_json::Value> {
    let Ok(text) = fs::read_to_string(data_dir.join("local_library/library.json")) else {
        return HashMap::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return HashMap::new();
    };
    let Some(pool) = json.get("games") else { return HashMap::new() };
    heronite_entries(pool.clone()).into_iter().collect()
}

/// Completa la entrada de la tienda con la metadata local sin pisar lo que ya
/// trae (el estado de instalación y la ruta son de la tienda).
fn heronite_merge_meta(entry: serde_json::Value, meta: &HashMap<String, serde_json::Value>) -> serde_json::Value {
    let Some(obj) = entry.as_object() else { return entry };
    let app_name = obj.get("appName").or_else(|| obj.get("app_name")).and_then(|v| v.as_str());
    let Some(m) = app_name.and_then(|a| meta.get(a)).and_then(|v| v.as_object()) else {
        return entry;
    };
    let mut out = obj.clone();
    for (k, v) in m {
        // `appName`/`app_name` son la clave: no se tocan.
        if k == "appName" || k == "app_name" {
            continue;
        }
        out.entry(k.clone()).or_insert_with(|| v.clone());
    }
    serde_json::Value::Object(out)
}

// ==================== SCAN ALL ====================

pub fn scan_all_launchers() -> Vec<ExternalGame> {
    let mut all_games = Vec::new();

    println!("[Thoriapp] ============================================");
    println!("[Thoriapp] Iniciando escaneo de launchers externos...");
    println!("[Thoriapp] ============================================");

    let lutris_games = scan_lutris_games();
    println!("[Thoriapp] Lutris: {} juegos encontrados", lutris_games.len());
    all_games.extend(lutris_games);

    let heroic_games = scan_heroic_games();
    println!("[Thoriapp] Heroic: {} juegos encontrados", heroic_games.len());
    all_games.extend(heroic_games);

    let heronite_games = scan_heronite_games();
    println!("[Thoriapp] Heronite: {} juegos encontrados", heronite_games.len());
    all_games.extend(heronite_games);

    // Orden estable e insensible a mayúsculas; deduplica por (nombre, launcher)
    // para no perder el mismo juego si está en dos launchers distintos
    all_games.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.launcher.cmp(&b.launcher))
    });
    all_games.dedup_by(|a, b| {
        a.launcher == b.launcher && a.name.to_lowercase() == b.name.to_lowercase()
    });

    println!("[Thoriapp] ============================================");
    println!(
        "[Thoriapp] Total: {} juegos únicos de launchers externos",
        all_games.len()
    );
    println!("[Thoriapp] ============================================");

    all_games
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_imita_a_lutris() {
        // Mismas reglas que lutris.util.strings.slugify
        assert_eq!(slugify("Soulmask"), "soulmask");
        assert_eq!(slugify("Assassin's Creed"), "assassins-creed");
        assert_eq!(slugify("Pokémon Edición"), "pokemon-edicion");
        assert_eq!(slugify("  Muchos   espacios  "), "muchos-espacios");
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn clave_de_lutris_para_juego_de_steam() {
        // Juegos de servicio: configpath ya es el slug limpio
        let key = build_lutris_cover_key("Soulmask", Some("soulmask"), Some(12));
        assert_eq!(key, "soulmask|12");
    }

    #[test]
    fn clave_de_lutris_para_juego_anadido_a_mano() {
        // Regresión: el .yml se llama `<slug>-<timestamp>` pero la carátula que
        // el usuario ve en Lutris es `<slug>.jpg`. Antes se buscaba solo el
        // configpath completo y los juegos manuales salían sin carátula.
        let key = build_lutris_cover_key("Soulmask", Some("soulmask-1700000000"), Some(7));
        assert_eq!(key, "soulmask-1700000000|soulmask|7");
    }

    #[test]
    fn clave_de_lutris_sin_configpath_usa_el_nombre() {
        let key = build_lutris_cover_key("Mi Juego", None, Some(3));
        assert_eq!(key, "mi-juego|3");
    }

    #[test]
    fn clave_de_lutris_sin_nada_utilizable() {
        // Nombre solo con símbolos y sin id: no hay nada que probar.
        assert_eq!(build_lutris_cover_key("***", None, None), "");
    }

    #[test]
    fn un_timestamp_no_se_confunde_con_un_slug() {
        // "half-life-2" acaba en dígito pero NO es un timestamp: no se recorta
        // a "half-life", que sería otro slug y otra carátula.
        let key = build_lutris_cover_key("Half-Life 2", Some("half-life-2"), None);
        assert_eq!(key, "half-life-2");
    }

    // ==================== HERONITE: JUEGOS DESINSTALADOS ====================

    #[test]
    fn heronite_descarta_juegos_desinstalados() {
        // Regresión: al borrar un juego, Heronite NO quita la entrada de
        // library.json, solo la marca `is_installed: false` (conserva horas y
        // estado). Thoriapp listaba esos juegos como si se pudieran lanzar
        // (Aion 2, Geoplanetical...).
        let raw = serde_json::json!({
            "title": "Aion 2",
            "appName": "5WzVLqXjCNJWsvmPCz4M4N",
            "runner": "sideload",
            "is_installed": false
        });

        assert!(heronite_game_from_entry("5WzVLqXjCNJWsvmPCz4M4N", raw).is_none());
    }

    #[test]
    fn heronite_conserva_los_juegos_instalados() {
        let raw = serde_json::json!({
            "title": "Soulmask",
            "appName": "steam_2646460",
            "runner": "sideload",
            "steamAppId": "2646460",
            "is_installed": true
        });

        let game = heronite_game_from_entry("steam_2646460", raw).expect("debe listarse");
        assert_eq!(game.name, "Soulmask");
        assert_eq!(game.appid, Some(2646460));
        assert_eq!(game.launcher, "heronite");
        assert_eq!(game.runner.as_deref(), Some("sideload"));
    }

    #[test]
    fn heronite_sin_el_campo_is_installed_se_lista() {
        // Versiones antiguas de Heronite no escribían `is_installed`. Descartar
        // por defecto dejaría la biblioteca vacía tras una actualización, así
        // que solo se descarta el `false` explícito.
        let raw = serde_json::json!({
            "title": "Juego Antiguo",
            "appName": "abc"
        });

        assert!(heronite_game_from_entry("abc", raw).is_some());
    }

    #[test]
    fn heronite_sin_runner_pero_instalado_se_lista() {
        // Los juegos de Steam importados no tienen runner, pero se listan.
        let raw = serde_json::json!({
            "title": "Fortnite",
            "appName": "Fortnite",
            "source": "epic",
            "is_installed": true
        });

        let game = heronite_game_from_entry("Fortnite", raw).expect("debe listarse");
        assert!(game.runner.is_none());
    }

    #[test]
    fn heronite_un_juego_desinstalado_no_aporta_candidatas_de_caratula() {
        // Si se filtrase después de construir el juego, el diagnóstico y el
        // contador seguirían viéndolo. Debe cortarse antes de construir nada.
        let raw = serde_json::json!({
            "title": "NBA 2K21",
            "appName": "steam_1091500",
            "steamAppId": 1091500,
            "is_installed": false
        });

        assert!(heronite_game_from_entry("steam_1091500", raw).is_none());
        assert!(heronite_game_from_entry(
            "steam_1091500",
            serde_json::json!({ "title": "NBA 2K21", "is_installed": false })
        )
        .is_none());
    }

    /// Carpeta temporal única por test: los tests de Rust corren en paralelo.
    fn dir_temporal_launchers(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "thoriapp_launchers_{}_{}",
            nombre,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ==================== HERONITE: FORMATO DE library.json ====================

    #[test]
    fn heronite_lee_el_formato_de_objeto_indexado_por_clave() {
        // Versiones nuevas: {"games": {"<clave>": {...}}}.
        let games = serde_json::json!({
            "steam_2646460": { "title": "Soulmask", "is_installed": true },
            "Fortnite": { "title": "Fortnite", "is_installed": true }
        });

        let entries = heronite_entries(games);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|(k, v)| k == "steam_2646460" && v["title"] == "Soulmask"));
    }

    #[test]
    fn heronite_lee_el_formato_de_array_con_appname() {
        // Versiones antiguas: {"games": [{...}]}, la clave es el `appName`.
        let games = serde_json::json!([
            { "appName": "steam_400", "title": "Portal", "is_installed": false },
            { "appName": "abc", "title": "Otro", "is_installed": true }
        ]);

        let entries = heronite_entries(games);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "steam_400");
        assert_eq!(entries[1].0, "abc");
    }

    #[test]
    fn heronite_un_array_con_desinstalados_se_filtra_igual() {
        // El bug: al leer solo el formato de objeto, el array se descartaba
        // entero (o el `is_installed` nunca se miraba) y los desinstalados
        // seguían apareciendo.
        let games = serde_json::json!([
            { "appName": "aion2", "title": "AION 2", "is_installed": false },
            { "appName": "dune", "title": "Dune Awakening", "is_installed": true }
        ]);

        let listed: Vec<String> = heronite_entries(games)
            .into_iter()
            .filter_map(|(k, raw)| heronite_game_from_entry(&k, raw).map(|g| g.name))
            .collect();

        assert_eq!(listed, vec!["Dune Awakening"]);
    }

    #[test]
    fn heronite_sin_games_no_rompe() {
        assert!(heronite_entries(serde_json::json!({})).is_empty());
        assert!(heronite_entries(serde_json::json!(null)).is_empty());
        assert!(heronite_entries(serde_json::json!("texto")).is_empty());
    }

    // ==================== HERONITE: QUITADOS DE LA COLECCIÓN ====================

    #[test]
    fn heronite_descarta_los_juegos_quitados_de_la_coleccion() {
        // El usuario los quitó con "Remove from Collection": Heronite los
        // guarda en `launcher-imports.json` -> `removed` y deja de listarlos.
        let base = dir_temporal_launchers("heronite_removed");
        let dir = base.join("local_library");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("launcher-imports.json"),
            serde_json::json!({ "removed": { "lutris": ["lutris_7", "lutris_9"] } }).to_string(),
        )
        .unwrap();

        let removed = heronite_removed_apps(&dir);
        assert!(removed.contains("lutris_7"));
        assert!(removed.contains("lutris_9"));

        let raw = serde_json::json!({ "title": "Heroes III", "appName": "lutris_7", "is_installed": true });
        assert!(heronite_game_from_entry_removed("lutris_7", raw, &removed).is_none());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn heronite_descarta_los_quitados_de_una_tienda() {
        // clients.json guarda los quitados de Epic/Battle.net/etc. por cliente.
        let base = dir_temporal_launchers("heronite_removed_clients");
        let dir = base.join("local_library");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("clients.json"),
            serde_json::json!({ "removed": { "battlenet": ["battlenet_wow"] } }).to_string(),
        )
        .unwrap();

        let removed = heronite_removed_apps(&dir);
        assert!(removed.contains("battlenet_wow"));

        let raw = serde_json::json!({ "title": "WoW", "appName": "battlenet_wow", "is_installed": true });
        assert!(heronite_game_from_entry_removed("battlenet_wow", raw, &removed).is_none());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn heronite_sin_archivos_de_quitados_no_descarta_nada() {
        let base = dir_temporal_launchers("heronite_removed_vacio");
        fs::create_dir_all(base.join("local_library")).unwrap();

        assert!(heronite_removed_apps(&base.join("local_library")).is_empty());

        let raw = serde_json::json!({ "title": "Soulmask", "appName": "steam_2646460", "is_installed": true });
        assert!(heronite_game_from_entry("steam_2646460", raw).is_some());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn un_quitado_mal_formado_no_tira_abajo_el_escaneo() {
        let base = dir_temporal_launchers("heronite_removed_roto");
        let dir = base.join("local_library");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("launcher-imports.json"), "{ no es json").unwrap();

        assert!(heronite_removed_apps(&dir).is_empty());

        let _ = fs::remove_dir_all(&base);
    }

    // ==================== HERONITE: CARPETA DE INSTALACIÓN ====================

    #[test]
    fn heronite_oculta_un_juego_de_tienda_borrado_por_fuera() {
        // Regresión (Fortnite / NBA 2K21): Epic deja `is_installed: true` en el
        // JSON, pero la carpeta de instalación ya no está. Heronite lo detecta
        // con `install_folder_missing()` (un `stat`); sin eso, Thoriapp los
        // listaba como instalados.
        let raw = serde_json::json!({
            "title": "NBA 2K21",
            "appName": "639977eecfd2497c941b71af949b5067",
            "runner": "legendary",
            "is_installed": true,
            "install": { "install_path": "/juegos/que/ya/no/existe/NBA2K21" }
        });

        assert!(heronite_game_from_entry("nba", raw).is_none());
    }

    #[test]
    fn heronite_muestra_un_juego_de_tienda_cuya_carpeta_existe() {
        // El caso contrario: si la carpeta sigue en disco, se lista.
        let base = dir_temporal_launchers("heronite_instalado");
        let raw = serde_json::json!({
            "title": "Fortnite",
            "appName": "Fortnite",
            "runner": "legendary",
            "is_installed": true,
            "install": { "install_path": base.to_string_lossy() }
        });

        let game = heronite_game_from_entry("Fortnite", raw).expect("debe listarse");
        assert_eq!(game.name, "Fortnite");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn heronite_sin_carpeta_de_instalacion_se_lista() {
        // Sin ruta no hay nada que comprobar: no se puede asumir que falta.
        // (Antes de tener `install_path` Heronite no lo guardaba.)
        let raw = serde_json::json!({
            "title": "Juego",
            "appName": "x",
            "runner": "legendary",
            "is_installed": true
        });

        assert!(heronite_game_from_entry("x", raw).is_some());
    }

    #[test]
    fn heronite_un_sideload_no_se_oculta_por_la_ruta() {
        // Los juegos añadidos a mano guardan en `executable` un comando
        // (`wine`, `flatpak`, un script), no una carpeta: comprobarlo ocultaría
        // juegos que funcionan.
        let raw = serde_json::json!({
            "title": "AION 2",
            "appName": "steam_3393110",
            "runner": "sideload",
            "is_installed": true,
            "install": { "executable": "steam steam://rungameid/3393110" }
        });

        assert!(heronite_game_from_entry("steam_3393110", raw).is_some());
    }

    #[test]
    fn heronite_expande_la_tilde_en_la_carpeta() {
        // Heronite guarda a veces `~/Games/...`; sin expandir nunca existiría y
        // ocultaría juegos instalados de verdad.
        let base = dir_temporal_launchers("heronite_tilde");
        let name = base.file_name().unwrap().to_string_lossy().to_string();

        std::env::set_var("HOME", base.parent().unwrap());
        assert!(install_target_exists(&format!("~/{name}")));
        assert!(!install_target_exists("~/carpeta_que_no_existe_xyz"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn solo_los_runners_de_tienda_comprueban_la_carpeta() {
        assert!(is_store_runner(Some("legendary")));
        assert!(is_store_runner(Some("gog")));
        assert!(is_store_runner(Some("nile")));
        assert!(!is_store_runner(Some("sideload")));
        assert!(!is_store_runner(None));
    }

    // ==================== HERONITE: FUENTES REALES ====================

    #[test]
    fn heronite_lee_los_juegos_de_sideload_y_no_de_la_metadata() {
        // Regresión del bug de los juegos fantasma: `local_library/library.json`
        // es SOLO metadata (no tiene `is_installed`). Los juegos están en
        // `sideload_apps/library.json`. Leyendo la metadata aparecían juegos ya
        // borrados (Fortnite, NBA 2K21, AION 2...).
        let base = dir_temporal_launchers("heronite_fuentes");
        fs::create_dir_all(base.join("local_library")).unwrap();
        fs::create_dir_all(base.join("sideload_apps")).unwrap();

        // Metadata: 3 entradas, una de ellas un juego ya borrado.
        fs::write(
            base.join("local_library/library.json"),
            serde_json::json!({
                "games": {
                    "steam_2646460": { "title": "Soulmask", "runner": "sideload", "appName": "steam_2646460" },
                    "borrado": { "title": "AION 2", "runner": "sideload", "appName": "borrado" }
                }
            })
            .to_string(),
        )
        .unwrap();

        // Biblioteca real: solo Soulmask.
        fs::write(
            base.join("sideload_apps/library.json"),
            serde_json::json!({
                "games": [
                    { "appName": "steam_2646460", "title": "Soulmask", "runner": "sideload", "is_installed": true }
                ]
            })
            .to_string(),
        )
        .unwrap();

        let files = heronite_library_files(&base);
        assert_eq!(files.len(), 1, "solo la fuente que existe");
        assert!(files[0].ends_with("sideload_apps/library.json"));

        let entries = heronite_store_entries(&files[0]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "steam_2646460");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn heronite_lee_el_library_de_epic_y_sus_desinstalados() {
        // `store_cache/legendary_library.json` usa la clave `library`.
        let base = dir_temporal_launchers("heronite_epic");
        fs::create_dir_all(base.join("store_cache")).unwrap();
        fs::write(
            base.join("store_cache/legendary_library.json"),
            serde_json::json!({
                "library": [
                    { "app_name": "Fortnite", "title": "Fortnite", "is_installed": false },
                    { "app_name": "abc", "title": "Otro", "is_installed": true }
                ]
            })
            .to_string(),
        )
        .unwrap();

        let entries = heronite_store_entries(&base.join("store_cache/legendary_library.json"));
        assert_eq!(entries.len(), 2);

        // Fortnite está desinstalado: no debe listarse.
        let raw = entries.into_iter().find(|(k, _)| k == "Fortnite").unwrap().1;
        assert!(heronite_game_from_entry("Fortnite", raw).is_none());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn heronite_la_metadata_completa_pero_no_pisa_el_estado() {
        // La metadata da el título bueno; el estado y la ruta son de la tienda.
        let mut meta = HashMap::new();
        meta.insert(
            "steam_480".to_string(),
            serde_json::json!({ "title": "Spacewar", "runner": "sideload", "appName": "steam_480" }),
        );

        let merged = heronite_merge_meta(
            serde_json::json!({ "app_name": "steam_480", "is_installed": false }),
            &meta,
        );

        assert_eq!(merged["title"], "Spacewar");
        assert_eq!(merged["runner"], "sideload");
        // Lo que trae la tienda manda: no se pisa el estado.
        assert_eq!(merged["is_installed"], false);
    }

    #[test]
    fn heronite_sin_ficheros_de_tienda_no_devuelve_juegos() {
        let base = dir_temporal_launchers("heronite_vacio");
        fs::create_dir_all(base.join("local_library")).unwrap();
        fs::write(
            base.join("local_library/library.json"),
            serde_json::json!({ "games": { "x": { "title": "Fantasma" } } }).to_string(),
        )
        .unwrap();

        // Sin `sideload_apps` ni `store_cache` no hay biblioteca real.
        assert!(heronite_library_files(&base).is_empty());

        let _ = fs::remove_dir_all(&base);
    }

    // ==================== HERONITE: INSTALADORES Y LAUNCHERS ====================

    #[test]
    fn heronite_descarta_el_instalador_de_ea_app() {
        // Regresión: el usuario añadió `EAappInstaller.exe` como juego y salía
        // en la biblioteca como si fuera un juego jugable.
        // Claves exactas que escribe Heronite en `sideload_apps/library.json`
        // (snake_case: `app_name`, `canRunOffline`… ver `build_game`).
        let raw = serde_json::json!({
            "runner": "sideload",
            "app_name": "q3iWPNp1GJwvxLb7kovdjo",
            "title": "EA App",
            "install": {
                "executable": "/home/thor/Descargas/EAappInstaller.exe",
                "platform": "linux",
                "is_dlc": false,
                "install_path": "/home/thor/Descargas"
            },
            "folder_name": "/home/thor/Descargas",
            "is_installed": true,
            "canRunOffline": true,
            "browserUrl": "",
            "launchFullScreen": false,
            "is_linux_native": false
        });

        assert!(heronite_game_from_entry("q3iWPNp1GJwvxLb7kovdjo", raw).is_none());
    }

    #[test]
    fn heronite_descarta_clientes_de_tienda_anadidos_como_juego() {
        // Añadir el cliente de la tienda en vez de un juego es el mismo error.
        for exe in [
            "C:/Program Files (x86)/Steam/steam.exe",
            "/usr/bin/steam",
            "C:/Program Files (x86)/Epic Games/Launcher/EpicGamesLauncher.exe",
            "C:/Program Files (x86)/GOG Galaxy/GalaxyClient.exe",
        ] {
            let raw = serde_json::json!({
                "title": "Cliente",
                "appName": "x",
                "runner": "sideload",
                "is_installed": true,
                "install": { "executable": exe }
            });
            assert!(
                heronite_game_from_entry("x", raw).is_none(),
                "{exe} debería descartarse"
            );
        }
    }

    #[test]
    fn heronite_descarta_cualquier_instalador() {
        assert!(is_not_a_game_executable("/home/u/Descargas/setup.exe"));
        assert!(is_not_a_game_executable("/home/u/Descargas/GameInstaller.exe"));
        assert!(is_not_a_game_executable("UbisoftConnectInstaller.exe"));
    }

    #[test]
    fn heronite_conserva_los_juegos_de_verdad() {
        // Los ejecutables reales del usuario no se tocan.
        for exe in [
            "/home/thor/Games/Dune Awakening/DuneSandbox.exe",
            "/home/thor/Games/Ostranauts/Ostranauts.exe",
            "/home/thor/Games/ZERO Sievert/ZERO Sievert.exe",
            "/home/thor/Games/Eslabong/eslabong.exe",
            "/home/thor/Games/Esports Manager 2026/EsportsManager.exe",
            "/home/thor/Games/Into the Dead/IntoTheDeadOurDarkestDays.exe",
            "/home/thor/Games/Probably Stolen/Probably Stolen.exe",
        ] {
            assert!(
                !is_not_a_game_executable(exe),
                "{exe} es un juego y no debe descartarse"
            );
        }
    }

    #[test]
    fn un_juego_de_steam_abierto_desde_heronite_se_conserva() {
        // `steam_1623730` es Palworld lanzado con `/usr/bin/steam`: el ejecutable
        // es el cliente, así que se descarta; pero eso lo decide la ruta, no el
        // nombre del juego.
        let palworld = serde_json::json!({
            "title": "Palworld",
            "appName": "steam_1623730",
            "runner": "sideload",
            "is_installed": true,
            "install": { "executable": "/usr/bin/steam" }
        });
        assert!(heronite_game_from_entry("steam_1623730", palworld).is_none());

        // Con su ejecutable real sí se lista.
        let real = serde_json::json!({
            "title": "Palworld",
            "appName": "steam_1623730",
            "runner": "sideload",
            "is_installed": true,
            "install": { "executable": "/home/thor/Games/Palworld/Palworld.exe" }
        });
        assert!(heronite_game_from_entry("steam_1623730", real).is_some());
    }

    // ==================== HERONITE: CLAVES SNAKE_CASE ====================

    #[test]
    fn heronite_lee_app_name_en_snake_case() {
        // Regresión: las fuentes de Heronite escriben `app_name` (snake_case,
        // nombre de `GameInfo`), no `appName`. Leyendo solo `appName` salía
        // siempre `None` y esto además despistaba al diagnóstico.
        let raw = serde_json::json!({
            "runner": "sideload",
            "app_name": "steam_2646460",
            "title": "Soulmask",
            "is_installed": true,
            "install": { "executable": "/home/thor/Games/Soulmask/Soulmask.exe" }
        });

        let game = heronite_game_from_entry("otra-clave", raw).unwrap();
        assert_eq!(game.cover_key.as_deref(), Some("steam_2646460|otra-clave|Soulmask"));
    }

    #[test]
    fn heronite_acepta_app_name_en_camel_case() {
        // `local_library/library.json` y Heroic sí usan `appName`.
        let raw = serde_json::json!({
            "runner": "sideload",
            "appName": "steam_2646460",
            "title": "Soulmask",
            "is_installed": true,
            "install": { "executable": "/home/thor/Games/Soulmask/Soulmask.exe" }
        });

        let game = heronite_game_from_entry("k", raw).unwrap();
        assert!(game.cover_key.unwrap().starts_with("steam_2646460|"));
    }

    #[test]
    fn heronite_lee_steam_app_id_en_snake_case() {
        // El AppID es lo que permite buscar la carátula en SteamGridDB.
        let raw = serde_json::json!({
            "runner": "sideload",
            "app_name": "steam_2646460",
            "title": "Soulmask",
            "steam_app_id": "2646460",
            "is_installed": true,
            "install": { "executable": "/home/thor/Games/Soulmask/Soulmask.exe" }
        });

        assert_eq!(heronite_game_from_entry("k", raw).unwrap().appid, Some(2646460));
    }

    #[test]
    fn heronite_usa_el_ejecutable_y_no_la_carpeta_para_detectar_instaladores() {
        // `install_path` es la carpeta que contiene el instalador: solo el
        // ejecutable revela que no es un juego.
        let raw = serde_json::json!({
            "runner": "sideload",
            "app_name": "q3iWPNp1GJwvxLb7kovdjo",
            "title": "EA App",
            "is_installed": true,
            "install": {
                "executable": "/home/thor/Descargas/EAappInstaller.exe",
                "install_path": "/home/thor/Descargas"
            },
            "folder_name": "/home/thor/Descargas"
        });

        let g: HeroniteGame = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(g.install_folder(), Some("/home/thor/Descargas"), "la carpeta es la buena para el stat");
        assert_eq!(g.executable(), Some("/home/thor/Descargas/EAappInstaller.exe"));
        assert!(heronite_game_from_entry("q3iWPNp1GJwvxLb7kovdjo", raw).is_none());
    }

    #[test]
    fn heronite_no_confunde_un_juego_en_la_carpeta_de_descargas_con_un_instalador() {
        // Un juego de verdad puede vivir en Descargas: se mira el ejecutable.
        let raw = serde_json::json!({
            "runner": "sideload",
            "app_name": "x",
            "title": "Indie",
            "is_installed": true,
            "install": {
                "executable": "/home/thor/Descargas/Indie/Indie.exe",
                "install_path": "/home/thor/Descargas/Indie"
            }
        });

        assert!(heronite_game_from_entry("x", raw).is_some());
    }
}
