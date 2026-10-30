use serde::Serialize;
use std::{env, fs, path::PathBuf};

#[derive(Serialize, Clone)]
pub struct LibGame {
    pub appid: u32,
    pub name: String,
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

pub fn scan_library() -> Vec<LibGame> {
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

    let mut out: Vec<LibGame> = vec![];
    for lib in libs {
        let Ok(rd) = fs::read_dir(&lib) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !(n.starts_with("appmanifest_") && n.ends_with(".acf")) {
                continue;
            }
            let Ok(txt) = fs::read_to_string(e.path()) else { continue };
            let mut appid = None;
            let mut name = None;
            for l in txt.lines() {
                if appid.is_none() {
                    appid = vdf_value(l, "appid").and_then(|v| v.parse::<u32>().ok());
                }
                if name.is_none() {
                    name = vdf_value(l, "name");
                }
            }
            if let (Some(appid), Some(name)) = (appid, name) {
                let skip = name.starts_with("Proton")
                    || name.starts_with("Steam Linux Runtime")
                    || name.starts_with("Steamworks Common");
                if !skip && !out.iter().any(|g| g.appid == appid) {
                    out.push(LibGame { appid, name });
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn get_json(url: &str) -> Result<serde_json::Value, String> {
    ureq::get(url)
        .set("User-Agent", "thoriapp/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| e.to_string())?
        .into_json::<serde_json::Value>()
        .map_err(|e| e.to_string())
}

pub fn search_store(term: &str) -> Result<Vec<LibGame>, String> {
    let t: String = term.chars().map(|c| if c.is_alphanumeric() { c.to_string() } else { format!("%{:02X}", c as u32 % 256) }).collect();
    let v = get_json(&format!("https://store.steampowered.com/api/storesearch/?term={t}&cc=us&l=english"))?;
    Ok(v["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|i| i["type"] == "app")
                .filter_map(|i| Some(LibGame { appid: i["id"].as_u64()? as u32, name: i["name"].as_str()?.to_string() }))
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
