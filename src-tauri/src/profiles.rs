use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

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

/// Tamaño máximo de un archivo de perfiles a importar (un perfil ocupa ~1 KB).
const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;
const DEFAULT_EXPORT_NAME: &str = "thoriapp_perfiles_backup.json";

/// Convierte lo que escribe el usuario en una ruta real: quita espacios y comillas,
/// acepta `file://` (al arrastrar un archivo) y expande `~` a la carpeta personal.
fn expand_path(raw: &str) -> PathBuf {
    let mut s = raw.trim();

    if s.len() >= 2 {
        let b = s.as_bytes();
        let first = b[0];
        let last = b[s.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            s = &s[1..s.len() - 1];
        }
    }

    if let Some(rest) = s.strip_prefix("file://") {
        s = rest;
    }

    if s == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(s)
}

/// Un usuario normal no puede escribir directamente en `/home/` (solo en `/home/<su_usuario>/`).
/// Si la ruta es un archivo colgando de `/home/`, se usa la carpeta personal en su lugar.
fn redirect_from_home_root(path: PathBuf) -> PathBuf {
    if path.extension().is_some() && path.parent() == Some(Path::new("/home")) {
        if let (Some(name), Some(home)) = (path.file_name(), dirs::home_dir()) {
            return home.join(name);
        }
    }
    path
}

/// Nombre de archivo seguro a partir de un texto cualquiera.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "perfil".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Mensaje de error de archivo comprensible para el usuario.
fn io_error_message(action: &str, path: &Path, e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::NotFound => format!("{}: no existe «{}»", action, path.display()),
        ErrorKind::PermissionDenied => format!(
            "{}: sin permiso para «{}». Prueba con una ruta dentro de tu carpeta personal (por ejemplo ~/Descargas).",
            action,
            path.display()
        ),
        _ => format!("{}: {} ({})", action, e, path.display()),
    }
}

/// Escritura atómica: se escribe un archivo temporal y se renombra, así un corte a mitad
/// de guardado no deja el archivo real a medias.
fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let mut tmp_name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    fs::write(&tmp, content)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn load_profiles_data() -> ProfilesData {
    let file = get_profiles_file();

    if file.exists() {
        match fs::read_to_string(&file) {
            Ok(content) => match serde_json::from_str::<ProfilesData>(&content) {
                Ok(data) => return data,
                Err(e) => {
                    // El archivo existe pero no se pudo leer: se guarda una copia antes de
                    // que un guardado posterior lo sobrescriba con una lista vacía.
                    println!("[Perfiles] ⚠️ profiles.json ilegible: {}", e);
                    let backup = file.with_extension("json.corrupt.bak");
                    fs::write(&backup, &content).ok();
                }
            },
            Err(e) => println!("[Perfiles] ⚠️ No se pudo leer profiles.json: {}", e),
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

    write_atomic(&file, &content).map_err(|e| format!("Error al guardar: {}", e))?;

    Ok(())
}

/// Calcula la ruta final de una exportación y comprueba que se puede usar.
fn resolve_export_path(raw: &str, default_name: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("No se indicó ninguna ruta de archivo.".to_string());
    }

    let mut path = redirect_from_home_root(expand_path(raw));

    // Si indicaron solo una carpeta, se crea el archivo dentro con un nombre por defecto
    if path.is_dir() {
        path = path.join(default_name);
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(format!("La carpeta «{}» no existe.", parent.display()));
        }
    }

    Ok(path)
}

// La firma es plana a propósito: es el contrato que invoca el frontend.
// Cambiarla por un struct obligaría a tocar api.ts y todos los llamadores.
#[allow(clippy::too_many_arguments)]
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

/// Exporta un perfil. Devuelve la ruta donde se guardó realmente.
#[tauri::command]
pub fn export_profile(profile_id: String, path: String) -> Result<String, String> {
    let data = load_profiles_data();

    let profile = data
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or("Perfil no encontrado")?;

    let default_name = format!(
        "{}_{}.json",
        sanitize_filename(&profile.game_name),
        sanitize_filename(&profile.name)
    );
    let target = resolve_export_path(&path, &default_name)?;

    let content = serde_json::to_string_pretty(profile)
        .map_err(|e| format!("Error al serializar: {}", e))?;

    write_atomic(&target, &content)
        .map_err(|e| io_error_message("No se pudo exportar", &target, &e))?;

    Ok(target.display().to_string())
}

/// Importa perfiles desde un archivo con UN perfil (`{...}`) o una LISTA (`[...]`, la que
/// genera "Exportar todos"). Los perfiles que ya existen (mismo ID) se omiten.
#[tauri::command]
pub fn import_profile(path: String) -> Result<Vec<Profile>, String> {
    if path.trim().is_empty() {
        return Err("No se indicó ninguna ruta de archivo.".to_string());
    }

    let requested = expand_path(&path);
    let mut file_path = requested.clone();

    // Si no existe y cuelga de /home/, se prueba en la carpeta personal
    if !file_path.exists() {
        let alt = redirect_from_home_root(requested.clone());
        if alt != requested && alt.exists() {
            file_path = alt;
        }
    }

    if file_path.is_dir() {
        return Err(format!(
            "«{}» es una carpeta. Indica la ruta del archivo .json.",
            file_path.display()
        ));
    }

    let meta = fs::metadata(&file_path)
        .map_err(|e| io_error_message("No se pudo abrir el archivo", &file_path, &e))?;
    if meta.len() > MAX_IMPORT_BYTES {
        return Err("El archivo es demasiado grande para ser un perfil (máximo 5 MB).".to_string());
    }

    let raw = fs::read_to_string(&file_path)
        .map_err(|e| io_error_message("No se pudo leer el archivo", &file_path, &e))?;
    import_from_content(&raw)
}

/// Lógica común de importación: recibe el TEXTO del archivo (un perfil `{...}` o una lista `[...]`).
fn import_from_content(raw: &str) -> Result<Vec<Profile>, String> {
    let content = raw.trim_start_matches('\u{feff}');

    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("El archivo no es un JSON válido: {}", e))?;

    let profiles: Vec<Profile> = if value.is_array() {
        serde_json::from_value::<Vec<Profile>>(value).map_err(|e| {
            format!("Algún perfil del archivo no tiene el formato correcto: {}", e)
        })?
    } else if value.is_object() {
        let single = serde_json::from_value::<Profile>(value).map_err(|e| {
            format!("El archivo no tiene el formato de un perfil de Thoriapp: {}", e)
        })?;
        vec![single]
    } else {
        return Err("El archivo no contiene perfiles de Thoriapp.".to_string());
    };

    if profiles.is_empty() {
        return Err("El archivo no contiene ningún perfil.".to_string());
    }

    let total = profiles.len();
    let mut data = load_profiles_data();
    let mut next_order = data
        .profiles
        .iter()
        .map(|p| p.order)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);
    let mut imported: Vec<Profile> = Vec::new();

    for mut profile in profiles {
        if data.profiles.iter().any(|p| p.id == profile.id) {
            continue;
        }
        // Se colocan al final de la lista, sin pisar el orden de los existentes
        profile.order = next_order;
        next_order += 1;

        data.profiles.push(profile.clone());
        imported.push(profile);
    }

    if imported.is_empty() {
        let msg = if total == 1 {
            "Este perfil ya está guardado (mismo ID), no se importó nada.".to_string()
        } else {
            format!(
                "No se importó nada: los {} perfiles del archivo ya están guardados.",
                total
            )
        };
        return Err(msg);
    }

    save_profiles_data(&data)?;

    Ok(imported)
}

/// Importa perfiles a partir del CONTENIDO de un archivo ya leído por la pantalla.
/// Es lo que usa el botón "Importar perfil": el navegador solo da el nombre del archivo,
/// no su ruta completa, así que la pantalla lee el archivo y envía su texto.
#[tauri::command]
pub fn import_profile_from_text(content: String) -> Result<Vec<Profile>, String> {
    if content.len() as u64 > MAX_IMPORT_BYTES {
        return Err("El archivo es demasiado grande para ser un perfil (máximo 5 MB).".to_string());
    }
    import_from_content(&content)
}

// Mismo contrato plano que save_profile (ver comentario allí).
#[allow(clippy::too_many_arguments)]
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

/// Exporta todos los perfiles en una lista. Devuelve la ruta donde se guardó realmente.
#[tauri::command]
pub fn export_all_profiles(path: String) -> Result<String, String> {
    let data = load_profiles_data();

    if data.profiles.is_empty() {
        return Err("No hay perfiles para exportar.".to_string());
    }

    let target = resolve_export_path(&path, DEFAULT_EXPORT_NAME)?;

    let content = serde_json::to_string_pretty(&data.profiles)
        .map_err(|e| format!("Error al serializar: {}", e))?;

    write_atomic(&target, &content)
        .map_err(|e| io_error_message("No se pudo exportar", &target, &e))?;

    Ok(target.display().to_string())
}
