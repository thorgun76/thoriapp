use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
pub struct ExternalGame {
    pub name: String,
    pub appid: Option<u32>,
    pub launcher: String,
    pub install_path: Option<String>,
    pub runner: Option<String>,
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
        "SELECT name, runner, directory, configpath, service, service_id
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
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            println!("[Lutris] ⚠ Error leyendo filas: {}", e);
            return games;
        }
    };

    for row in rows.flatten() {
        let (name, runner, directory, configpath, service, service_id) = row;

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

        println!("[Lutris] ✓ {} (AppID: {:?})", name, appid);

        games.push(ExternalGame {
            name,
            appid,
            launcher: "lutris".to_string(),
            install_path,
            runner,
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

#[derive(Deserialize, Debug)]
struct HeroniteLibrary {
    // Se deserializa como Value para que una entrada rara no tire abajo toda la librería
    games: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct HeroniteGame {
    title: String,
    // Puede venir como string o como número según la versión
    #[serde(rename = "steamAppId", default)]
    steam_app_id: Option<serde_json::Value>,
    #[serde(default)]
    runner: Option<String>,
    #[serde(rename = "launcherArgs", default)]
    launcher_args: Option<String>,
}

fn parse_app_id(v: &Option<serde_json::Value>) -> Option<u32> {
    match v.as_ref()? {
        serde_json::Value::String(s) => s.trim().parse::<u32>().ok(),
        serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

pub fn scan_heronite_games() -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let library_file = home_dir().join(".local/share/heronite/local_library/library.json");

    if !library_file.exists() {
        println!("[Heronite] Archivo library.json no existe: {:?}", library_file);
        return games;
    }

    println!("[Heronite] Escaneando: {:?}", library_file);

    let content = match fs::read_to_string(&library_file) {
        Ok(c) => c,
        Err(e) => {
            println!("[Heronite] ⚠ Error al leer library.json: {}", e);
            return games;
        }
    };

    let library = match serde_json::from_str::<HeroniteLibrary>(&content) {
        Ok(l) => l,
        Err(e) => {
            println!("[Heronite] ⚠ Error al parsear library.json: {}", e);
            return games;
        }
    };

    for (key, raw) in library.games {
        let game: HeroniteGame = match serde_json::from_value(raw) {
            Ok(g) => g,
            Err(e) => {
                println!("[Heronite] Ignorada entrada '{}': {}", key, e);
                continue;
            }
        };

        let appid = parse_app_id(&game.steam_app_id);

        println!("[Heronite] ✓ {} (AppID: {:?})", game.title, appid);

        games.push(ExternalGame {
            name: game.title,
            appid,
            launcher: "heronite".to_string(),
            install_path: game.launcher_args,
            runner: game.runner,
        });
    }

    println!("[Heronite] Total juegos encontrados: {}", games.len());
    games
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
