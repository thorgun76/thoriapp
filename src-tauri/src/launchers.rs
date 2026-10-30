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

#[derive(Deserialize, Debug)]
struct HeroicLegendaryLibrary {
    library: Vec<HeroicGame>,
}

#[derive(Deserialize, Debug)]
struct HeroicGame {
    app_name: String,
    title: String,
    #[serde(rename = "is_installed")]
    is_installed: Option<bool>,
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

    // Buscar archivos .yml en la carpeta games/
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

                            // Extraer información del YAML
                            let mut game_name = String::new();
                            let mut exe_path = None;
                            let mut runner = None;

                            for line in content.lines() {
                                let line = line.trim();

                                // Buscar el nombre del juego
                                if line.starts_with("name:") {
                                    game_name = line.trim_start_matches("name:")
                                        .trim()
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .to_string();
                                }

                                // Buscar la ruta del ejecutable
                                if line.contains("exe:") {
                                    exe_path = line.split("exe:")
                                        .nth(1)
                                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string());
                                }

                                // Buscar el runner (wine, linux, etc)
                                if line.starts_with("runner:") {
                                    runner = Some(line.trim_start_matches("runner:")
                                        .trim()
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .to_string());
                                }
                            }

                            // Si no se encontró el nombre en el YAML, extraerlo del nombre del archivo
                            if game_name.is_empty() {
                                // Formato del archivo: nombre-juego-timestamp.yml o steam-appid-timestamp.yml
                                // Ejemplos:
                                // - bellwright-1769529934.yml -> Bellwright
                                // - steam-251570-1782398094.yml -> Steam Game 251570
                                // - star-valor-1766789644.yml -> Star Valor

                                let name_part = if filename.starts_with("steam-") {
                                    // Es un juego de Steam, usar el AppID como identificador
                                    format!("Steam Game {}", filename.split('-').nth(1).unwrap_or("Unknown"))
                                } else {
                                    // Extraer el nombre antes del timestamp
                                    let parts: Vec<&str> = filename.rsplitn(2, '-').collect();
                                    if parts.len() == 2 {
                                        parts[1].to_string()
                                    } else {
                                        filename.clone()
                                    }
                                };

                                // Convertir slug a nombre legible
                                // Reemplazar guiones por espacios y capitalizar palabras
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

                            // Extraer Steam AppID del nombre del archivo si existe
                            // Formato: steam-APPID-TIMESTAMP.yml
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

pub fn scan_heroic_games() -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let heroic_dir = PathBuf::from(&home).join(".config/heroic");

    if !heroic_dir.exists() {
        println!("[Heroic] Directorio no existe: {:?}", heroic_dir);
        return games;
    }

    println!("[Heroic] Escaneando: {:?}", heroic_dir);

    // Buscar juegos de Epic Games en legendaryConfig
    let legendary_metadata_dir = heroic_dir.join("legendaryConfig/legendary/metadata");
    if legendary_metadata_dir.exists() {
        println!("[Heroic] Escaneando metadata de Epic Games: {:?}", legendary_metadata_dir);

        if let Ok(entries) = fs::read_dir(&legendary_metadata_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "json" {
                            if let Ok(content) = fs::read_to_string(&path) {
                                // Intentar parsear como metadata de juego
                                if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&content) {
                                    // Buscar el título del juego
                                    if let Some(title) = metadata.get("title").and_then(|t| t.as_str()) {
                                        let app_name = path.file_stem()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("")
                                            .to_string();

                                        println!("[Heroic] Juego Epic encontrado: {} ({})", title, app_name);

                                        games.push(ExternalGame {
                                            name: title.to_string(),
                                            appid: None,
                                            launcher: "heroic".to_string(),
                                            install_path: None,
                                            runner: Some(format!("epic-{}", app_name)),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Buscar juegos de GOG en gog_store
    let gog_store_dir = heroic_dir.join("gog_store");
    if gog_store_dir.exists() {
        println!("[Heroic] Escaneando GOG store: {:?}", gog_store_dir);

        if let Ok(entries) = fs::read_dir(&gog_store_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(game_data) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(title) = game_data.get("title").and_then(|t| t.as_str()) {
                                println!("[Heroic] Juego GOG encontrado: {}", title);

                                games.push(ExternalGame {
                                    name: title.to_string(),
                                    appid: None,
                                    launcher: "heroic".to_string(),
                                    install_path: None,
                                    runner: Some("gog".to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Buscar juegos sideloaded
    let sideload_file = heroic_dir.join("sideload_apps/library.json");
    if sideload_file.exists() {
        println!("[Heroic] Escaneando sideloaded apps: {:?}", sideload_file);

        if let Ok(content) = fs::read_to_string(&sideload_file) {
            if let Ok(sideload_data) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(games_array) = sideload_data.as_array() {
                    for game_data in games_array {
                        if let Some(title) = game_data.get("app_name").and_then(|t| t.as_str()) {
                            println!("[Heroic] Juego sideloaded encontrado: {}", title);

                            games.push(ExternalGame {
                                name: title.to_string(),
                                appid: None,
                                launcher: "heroic".to_string(),
                                install_path: None,
                                runner: Some("sideloaded".to_string()),
                            });
                        }
                    }
                }
            }
        }
    }

    println!("[Heroic] Total juegos encontrados: {}", games.len());
    games
}

pub fn scan_heronite_games() -> Vec<ExternalGame> {
    let mut games = Vec::new();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    // Heronite guarda los juegos en ~/.local/share/heronite/local_library/library.json
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

    // También buscar juegos de Epic Games en legendary_library.json
    let legendary_file = PathBuf::from(&home).join(".local/share/heronite/store_cache/legendary_library.json");

    if legendary_file.exists() {
        println!("[Heronite] Escaneando Epic Games: {:?}", legendary_file);

        if let Ok(content) = fs::read_to_string(&legendary_file) {
            if let Ok(library) = serde_json::from_str::<HeroicLegendaryLibrary>(&content) {
                for game in library.library {
                    // Solo incluir juegos instalados
                    if game.is_installed.unwrap_or(false) {
                        println!("[Heronite] Juego Epic encontrado: {}", game.title);

                        games.push(ExternalGame {
                            name: game.title,
                            appid: None,
                            launcher: "heronite".to_string(),
                            install_path: None,
                            runner: Some(format!("epic-{}", game.app_name)),
                        });
                    }
                }
            }
        }
    }

    println!("[Heronite] Total juegos encontrados: {}", games.len());
    games
}

pub fn scan_all_launchers() -> Vec<ExternalGame> {
    let mut all_games = Vec::new();

    println!("[Thoriapp] Iniciando escaneo de launchers externos...");

    // Escanear todos los launchers
    let lutris_games = scan_lutris_games();
    println!("[Thoriapp] Lutris: {} juegos encontrados", lutris_games.len());
    all_games.extend(lutris_games);

    let heroic_games = scan_heroic_games();
    println!("[Thoriapp] Heroic: {} juegos encontrados", heroic_games.len());
    all_games.extend(heroic_games);

    let heronite_games = scan_heronite_games();
    println!("[Thoriapp] Heronite: {} juegos encontrados", heronite_games.len());
    all_games.extend(heronite_games);

    // Eliminar duplicados por nombre
    all_games.sort_by(|a, b| a.name.cmp(&b.name));
    all_games.dedup_by(|a, b| a.name == b.name);

    println!("[Thoriapp] Total: {} juegos únicos de launchers externos", all_games.len());

    all_games
}
