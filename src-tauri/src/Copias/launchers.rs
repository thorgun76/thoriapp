use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Clone, Debug)]
pub struct ExternalGame {
    pub name: String,
    pub appid: Option<u32>,
    pub launcher: String,
    pub install_path: Option<String>,
    pub runner: Option<String>,
}

#[derive(Deserialize, Debug)]
struct HeroniteLibrary {
    games: HashMap<String, HeroniteGame>,
}

#[derive(Deserialize, Debug)]
struct HeroniteGame {
    title: String,
    #[serde(rename = "steamAppId")]
    steam_app_id: Option<String>,
    runner: Option<String>,
    #[serde(rename = "launcherArgs")]
    launcher_args: Option<String>,
}

pub fn scan_lutris_games() -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let lutris_games_dir = PathBuf::from(&home).join(".local/share/lutris/games");

    if !lutris_games_dir.exists() {
        println!("[Lutris] Directorio no existe: {:?}", lutris_games_dir);
        return games;
    }

    println!("[Lutris] Escaneando: {:?}", lutris_games_dir);

    if let Ok(entries) = fs::read_dir(&lutris_games_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yml" {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let filename = path.file_stem()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();

                            let mut game_name = String::new();
                            let mut exe_path = None;
                            let mut runner = None;

                            for line in content.lines() {
                                let line = line.trim();

                                if line.starts_with("name:") {
                                    game_name = line.trim_start_matches("name:")
                                        .trim()
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .to_string();
                                }

                                if line.contains("exe:") {
                                    exe_path = line.split("exe:")
                                        .nth(1)
                                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string());
                                }

                                if line.starts_with("runner:") {
                                    runner = Some(line.trim_start_matches("runner:")
                                        .trim()
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .to_string());
                                }
                            }

                            if game_name.is_empty() {
                                let name_part = if filename.starts_with("steam-") {
                                    format!("Steam Game {}", filename.split('-').nth(1).unwrap_or("Unknown"))
                                } else {
                                    let parts: Vec<&str> = filename.rsplitn(2, '-').collect();
                                    if parts.len() == 2 {
                                        parts[1].to_string()
                                    } else {
                                        filename.clone()
                                    }
                                };

                                game_name = name_part
                                    .replace('-', " ")
                                    .split_whitespace()
                                    .map(|word| {
                                        let mut chars = word.chars();
                                        match chars.next() {
                                            None => String::new(),
                                            Some(first_char) => {
                                                let capitalized = first_char.to_uppercase().to_string();
                                                capitalized + &chars.as_str().to_lowercase()
                                            }
                                        }
                                    })
                                    .collect::<Vec<String>>()
                                    .join(" ");
                            }

                            let appid = if filename.starts_with("steam-") {
                                filename.split('-')
                                    .nth(1)
                                    .and_then(|id| id.parse::<u32>().ok())
                            } else {
                                None
                            };

                            println!("[Lutris] Juego encontrado: {} (AppID: {:?})", game_name, appid);

                            games.push(ExternalGame {
                                name: game_name,
                                appid,
                                launcher: "lutris".to_string(),
                                install_path: exe_path,
                                runner,
                            });
                        }
                    }
                }
            }
        }
    }

    println!("[Lutris] Total juegos encontrados: {}", games.len());
    games
}

// ==================== HEROIC (CORREGIDO) ====================

/// Encuentra el directorio de Heroic (nativo o Flatpak)
fn find_heroic_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let home_path = PathBuf::from(&home);

    let candidates = vec![
        home_path.join(".config/heroic"),
        home_path.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
    ];

    for path in candidates {
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Lee un archivo JSON de forma segura
fn read_json(path: &PathBuf) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Extrae un string de un valor JSON
fn json_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(String::from)
}

/// Escanea juegos de Epic instalados en Heroic
/// Usa legendaryConfig/legendary/installed.json (archivo central de juegos instalados)
fn scan_heroic_epic_games(dir: &PathBuf) -> Vec<ExternalGame> {
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
        if game_data.get("is_dlc").and_then(|d| d.as_bool()).unwrap_or(false) {
            continue;
        }

        let title = json_str(game_data, "title").unwrap_or_else(|| id.clone());
        let install_path = json_str(game_data, "install_path");

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

/// Escanea juegos de GOG instalados en Heroic
fn scan_heroic_gog_games(dir: &PathBuf) -> Vec<ExternalGame> {
    let mut games = Vec::new();

    // GOG usa una estructura diferente - buscar en varias ubicaciones posibles
    let gog_paths = vec![
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
        } else if let Some(obj) = value.get("games").and_then(|g| g.as_array()) {
            obj.clone()
        } else {
            continue;
        };

        for game_data in &games_array {
            // Solo incluir juegos instalados
            if let Some(installed) = game_data.get("is_installed") {
                if !installed.as_bool().unwrap_or(false) {
                    continue;
                }
            }

            if let Some(title) = json_str(game_data, "title") {
                let app_name = json_str(game_data, "app_name").unwrap_or_default();
                let install_path = json_str(game_data, "install_path")
                    .or_else(|| json_str(game_data, "folder_name"));

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
    }

    if games.is_empty() {
        println!("[Heroic] ⚠ No se encontraron juegos de GOG");
    } else {
        println!("[Heroic] Total GOG: {}", games.len());
    }

    games
}

/// Escanea juegos de Amazon instalados en Heroic
fn scan_heroic_amazon_games(dir: &PathBuf) -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let amazon_paths = vec![
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
                if let Some(title) = json_str(game_data, "title") {
                    let install_path = json_str(game_data, "install_path");

                    println!("[Heroic] ✓ Amazon: {} (id: {})", title, id);

                    games.push(ExternalGame {
                        name: title,
                        appid: None,
                        launcher: "heroic".to_string(),
                        install_path,
                        runner: Some(format!("amazon-{}", id)),
                    });
                }
            }
        } else if let Some(arr) = value.as_array() {
            for game_data in arr {
                if let Some(title) = json_str(game_data, "title") {
                    let id = json_str(game_data, "id")
                        .or_else(|| json_str(game_data, "app_name"))
                        .unwrap_or_default();
                    let install_path = json_str(game_data, "install_path");

                    println!("[Heroic] ✓ Amazon: {} (id: {})", title, id);

                    games.push(ExternalGame {
                        name: title,
                        appid: None,
                        launcher: "heroic".to_string(),
                        install_path,
                        runner: Some(format!("amazon-{}", id)),
                    });
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
fn scan_heroic_sideload_games(dir: &PathBuf) -> Vec<ExternalGame> {
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
        println!("[Heroic] ✗ No se encontró la carpeta de Heroic");
        println!("[Heroic]   Buscado en:");
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        println!("[Heroic]   - {}/.config/heroic", home);
        println!("[Heroic]   - {}/.var/app/com.heroicgameslauncher.hgl/config/heroic", home);
        return all_games;
    };

    println!("[Heroic] ============================================");
    println!("[Heroic] Directorio encontrado: {:?}", heroic_dir);
    println!("[Heroic] ============================================");

    // Escanear cada tipo de tienda
    let epic_games = scan_heroic_epic_games(&heroic_dir);
    all_games.extend(epic_games);

    let gog_games = scan_heroic_gog_games(&heroic_dir);
    all_games.extend(gog_games);

    let amazon_games = scan_heroic_amazon_games(&heroic_dir);
    all_games.extend(amazon_games);

    let sideload_games = scan_heroic_sideload_games(&heroic_dir);
    all_games.extend(sideload_games);

    println!("[Heroic] ============================================");
    println!("[Heroic] Total juegos de Heroic: {}", all_games.len());
    println!("[Heroic] ============================================");

    all_games
}

// ==================== HERONITE ====================

pub fn scan_heronite_games() -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    let library_file = PathBuf::from(&home).join(".local/share/heronite/local_library/library.json");

    if !library_file.exists() {
        println!("[Heronite] Archivo library.json no existe: {:?}", library_file);
        return games;
    }

    println!("[Heronite] Escaneando: {:?}", library_file);

    if let Ok(content) = fs::read_to_string(&library_file) {
        if let Ok(library) = serde_json::from_str::<HeroniteLibrary>(&content) {
            for (_key, game) in library.games {
                let appid = game.steam_app_id.as_ref().and_then(|id| id.parse::<u32>().ok());

                println!("[Heronite] Juego encontrado: {} (AppID: {:?})", game.title, appid);

                games.push(ExternalGame {
                    name: game.title,
                    appid,
                    launcher: "heronite".to_string(),
                    install_path: game.launcher_args,
                    runner: game.runner,
                });
            }
        } else {
            println!("[Heronite] Error al parsear library.json");
        }
    } else {
        println!("[Heronite] Error al leer library.json");
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

    all_games.sort_by(|a, b| a.name.cmp(&b.name));
    all_games.dedup_by(|a, b| a.name == b.name);

    println!("[Thoriapp] ============================================");
    println!("[Thoriapp] Total: {} juegos únicos de launchers externos", all_games.len());
    println!("[Thoriapp] ============================================");

    all_games
}
