use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub appid: u32,
    pub game_name: String,
    pub enabled_params: Vec<bool>,
    pub selected_community_recs: Vec<usize>,
    pub selected_engine_recs: Vec<usize>,
    pub force_proton: bool,
    pub expert_mode: bool,
    pub manual_command: Option<String>,
    pub use_manual_command: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilesData {
    pub profiles: Vec<Profile>,
}

fn get_profiles_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("thoriapp")
        .join("profiles");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).ok();
    }

    config_dir
}

fn get_profiles_file() -> PathBuf {
    get_profiles_dir().join("profiles.json")
}

fn load_profiles_data() -> ProfilesData {
    let file = get_profiles_file();

    if file.exists() {
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(data) = serde_json::from_str(&content) {
                return data;
            }
        }
    }

    ProfilesData {
        profiles: Vec::new(),
    }
}

fn save_profiles_data(data: &ProfilesData) -> Result<(), String> {
    let file = get_profiles_file();
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Error al serializar: {}", e))?;

    fs::write(&file, content)
        .map_err(|e| format!("Error al guardar: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn save_profile(
    name: String,
    appid: u32,
    game_name: String,
    enabled_params: Vec<bool>,
    selected_community_recs: Vec<usize>,
    selected_engine_recs: Vec<usize>,
    force_proton: bool,
    expert_mode: bool,
    manual_command: Option<String>,
    use_manual_command: bool,
    notes: Option<String>,
) -> Result<Profile, String> {
    let mut data = load_profiles_data();

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let next_order = data.profiles.iter().map(|p| p.order).max().unwrap_or(0) + 1;

    let profile = Profile {
        id: format!("{}_{}", appid, now.replace([':', ' ', '-'], "")),
        name,
        appid,
        game_name,
        enabled_params,
        selected_community_recs,
        selected_engine_recs,
        force_proton,
        expert_mode,
        manual_command,
        use_manual_command,
        notes,
        created_at: now.clone(),
        updated_at: now,
        order: next_order,
    };

    data.profiles.push(profile.clone());
    save_profiles_data(&data)?;

    Ok(profile)
}

#[tauri::command]
pub fn load_profiles() -> Result<Vec<Profile>, String> {
    let data = load_profiles_data();
    Ok(data.profiles)
}

#[tauri::command]
pub fn load_profiles_for_game(appid: u32) -> Result<Vec<Profile>, String> {
    let data = load_profiles_data();
    let profiles = data.profiles.into_iter()
        .filter(|p| p.appid == appid)
        .collect();
    Ok(profiles)
}

#[tauri::command]
pub fn delete_profile(profile_id: String) -> Result<(), String> {
    let mut data = load_profiles_data();
    data.profiles.retain(|p| p.id != profile_id);
    save_profiles_data(&data)?;
    Ok(())
}

#[tauri::command]
pub fn export_profile(profile_id: String, path: String) -> Result<(), String> {
    let data = load_profiles_data();

    let profile = data.profiles.iter()
        .find(|p| p.id == profile_id)
        .ok_or("Perfil no encontrado")?;

    let content = serde_json::to_string_pretty(profile)
        .map_err(|e| format!("Error al serializar: {}", e))?;

    fs::write(&path, content)
        .map_err(|e| format!("Error al exportar: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn import_profile(path: String) -> Result<Vec<Profile>, String> {
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Error al leer archivo: {}", e))?;

    if let Ok(profiles) = serde_json::from_str::<Vec<Profile>>(&content) {
        let mut data = load_profiles_data();
        let mut imported = Vec::new();

        for profile in profiles {
            if !data.profiles.iter().any(|p| p.id == profile.id) {
                data.profiles.push(profile.clone());
                imported.push(profile);
            }
        }

        save_profiles_data(&data)?;
        return Ok(imported);
    }

    let profile: Profile = serde_json::from_str(&content)
        .map_err(|e| format!("Error al parsear perfil: {}", e))?;

    let mut data = load_profiles_data();

    if data.profiles.iter().any(|p| p.id == profile.id) {
        return Err("Ya existe un perfil con este ID".to_string());
    }

    data.profiles.push(profile.clone());
    save_profiles_data(&data)?;

    Ok(vec![profile])
}

#[tauri::command]
pub fn update_profile(
    profile_id: String,
    name: String,
    enabled_params: Vec<bool>,
    selected_community_recs: Vec<usize>,
    selected_engine_recs: Vec<usize>,
    force_proton: bool,
    expert_mode: bool,
    manual_command: Option<String>,
    use_manual_command: bool,
    notes: Option<String>,
) -> Result<Profile, String> {
    let mut data = load_profiles_data();

    let profile = data.profiles.iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or("Perfil no encontrado")?;

    profile.name = name;
    profile.enabled_params = enabled_params;
    profile.selected_community_recs = selected_community_recs;
    profile.selected_engine_recs = selected_engine_recs;
    profile.force_proton = force_proton;
    profile.expert_mode = expert_mode;
    profile.manual_command = manual_command;
    profile.use_manual_command = use_manual_command;
    profile.notes = notes;
    profile.updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let updated_profile = profile.clone();
    save_profiles_data(&data)?;

    Ok(updated_profile)
}

#[tauri::command]
pub fn reorder_profiles(profile_ids: Vec<String>) -> Result<(), String> {
    let mut data = load_profiles_data();

    for (index, profile_id) in profile_ids.iter().enumerate() {
        if let Some(profile) = data.profiles.iter_mut().find(|p| &p.id == profile_id) {
            profile.order = index as u32;
        }
    }

    data.profiles.sort_by_key(|p| p.order);
    save_profiles_data(&data)?;

    Ok(())
}

#[tauri::command]
pub fn export_all_profiles(path: String) -> Result<(), String> {
    let data = load_profiles_data();

    let content = serde_json::to_string_pretty(&data.profiles)
        .map_err(|e| format!("Error al serializar: {}", e))?;

    fs::write(&path, content)
        .map_err(|e| format!("Error al exportar: {}", e))?;

    Ok(())
}
