mod benchmarks;
mod covers;
mod engine;
mod hw;
mod launchers;
mod log;
mod profiles;
mod rules;
mod steam;

use serde::Serialize;
use tauri::async_runtime::spawn_blocking;

/// Carátula local (data URI base64) que el launcher guarda en disco, o null si
/// no tiene una. Va por comando (y no por URL de archivo) para no tener que
/// habilitar el protocolo `asset` ni tocar la CSP.
#[tauri::command]
async fn get_local_cover(
    launcher: String,
    appid: Option<u32>,
    key: Option<String>,
    runner: Option<String>,
) -> Option<String> {
    spawn_blocking(move || {
        covers::local_cover_with_runner(
            &launcher,
            appid,
            key.as_deref().unwrap_or(""),
            runner.as_deref(),
        )
    })
    .await
    .ok()
    .flatten()
}

/// Diagnóstico de las carátulas locales: qué carpetas existen y qué archivos
/// hay dentro. Se usa desde el botón de la UI para depurar sin adivinar.
#[tauri::command]
async fn diagnose_covers() -> String {
    spawn_blocking(covers::diagnose).await.unwrap_or_default()
}

#[derive(Serialize)]
struct HwInfo {
    #[serde(flatten)]
    hw: hw::Hardware,
    tags: Vec<String>,
}

#[tauri::command]
async fn get_hardware() -> Result<HwInfo, String> {
    spawn_blocking(|| {
        let hw = hw::detect();
        let tags = hw.tags();
        HwInfo { hw, tags }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_rules(appid: u32) -> rules::Game {
    rules::for_appid(appid)
}

#[tauri::command]
async fn scan_library() -> Result<Vec<steam::LibGame>, String> {
    spawn_blocking(steam::scan_library)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_store(term: String) -> Result<Vec<steam::LibGame>, String> {
    spawn_blocking(move || steam::search_store(&term))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn game_info(appid: u32) -> Result<steam::GameInfo, String> {
    spawn_blocking(move || steam::game_info(appid))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn analyze_log(path: String) -> Result<log::Stats, String> {
    spawn_blocking(move || log::analyze(&path))
        .await
        .map_err(|e| e.to_string())?
}

/// Extrae el host de una URL https y comprueba que NO sea solo el prefijo.
///
/// Antes se usaba `starts_with("https://github.com/")`, que acepta
/// `https://github.com.evil.com/` porque el texto empieza igual. Aquí se separa
/// el host real (hasta el primer `/`, `?` o `#`) y se compara exacto.
fn host_matches(url: &str, allowed_host: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };

    // El host termina en el primer `/`, `?` o `#`. Sin ninguno, es el host entero.
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");

    // Se descartan credenciales (`user@host`) y puerto (`host:443`).
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);

    host.eq_ignore_ascii_case(allowed_host)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    const ALLOWED_HOSTS: [&str; 7] = [
        "www.protondb.com",
        "store.steampowered.com",
        "docs.kernel.org",
        "github.com",
        "gitlab.freedesktop.org",
        "wiki.archlinux.org",
        "steamspy.com",
    ];

    // Solo https, y solo hacia los dominios de la lista. Comprobar el host
    // completo evita que un dominio parecido se cuele por prefijo.
    if !ALLOWED_HOSTS.iter().any(|h| host_matches(&url, h)) {
        return Err(format!("URL no permitida: {url}"));
    }

    let mut child = std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("No pude abrir el navegador: {e}"))?;
    // xdg-open puede quedarse vivo mientras el navegador esté abierto: se espera
    // en un hilo aparte para no bloquear el hilo del comando.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
fn detect_engine(appid: u32, game_name: String) -> Option<engine::GameEngine> {
    engine::detect_engine(appid, &game_name)
}

#[tauri::command]
fn get_engine_recommendations(engine: engine::GameEngine) -> Vec<String> {
    engine::get_engine_recommendations(&engine)
}

#[tauri::command]
fn get_community_recommendations(hw_tags: Vec<String>) -> Vec<engine::CommunityRecommendation> {
    let all_recommendations = engine::get_hardware_recommendations();
    engine::filter_recommendations_by_hardware(all_recommendations, &hw_tags)
}

#[tauri::command]
async fn scan_external_launchers() -> Result<Vec<launchers::ExternalGame>, String> {
    spawn_blocking(launchers::scan_all_launchers)
        .await
        .map_err(|e| e.to_string())
}

/// Juegos de Heronite descartados por estar desinstalados (`is_installed:
/// false`). La UI los menciona para que no parezca que faltan juegos.
#[tauri::command]
fn heronite_uninstalled_count() -> usize {
    launchers::heronite_desinstalados()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_hardware,
            get_rules,
            scan_library,
            search_store,
            game_info,
            analyze_log,
            open_url,
            detect_engine,
            get_engine_recommendations,
            get_community_recommendations,
            profiles::save_profile,
            profiles::load_profiles,
            profiles::load_profiles_for_game,
            profiles::delete_profile,
            profiles::export_profile,
            profiles::export_all_profiles,
            profiles::import_profile,
            profiles::import_profile_from_text,
            profiles::update_profile,
            profiles::reorder_profiles,
            benchmarks::save_benchmark,
            benchmarks::load_benchmarks,
            benchmarks::load_benchmarks_for_game,
            benchmarks::delete_benchmark,
            benchmarks::delete_all_benchmarks,
            benchmarks::compare_benchmarks,
            benchmarks::analyze_mangohud_log_command,
            benchmarks::detect_mangohud_log_path,
            benchmarks::check_mangohud_installed,
            scan_external_launchers,
            heronite_uninstalled_count,
            benchmarks::get_mangohud_log_duration,
            benchmarks::set_mangohud_log_duration,
            benchmarks::save_comparison,
            benchmarks::load_comparisons,
            benchmarks::delete_comparison,
            benchmarks::delete_all_comparisons,
            get_local_cover,
            diagnose_covers,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la app");
}

// ==================== TESTS ====================
// Solo la validación de URLs: es lógica pura y es la puerta de seguridad.

#[cfg(test)]
mod tests {
    use super::host_matches;

    #[test]
    fn acepta_el_dominio_exacto() {
        assert!(host_matches("https://github.com/foo/bar", "github.com"));
        assert!(host_matches("https://github.com", "github.com"));
        assert!(host_matches("https://github.com/", "github.com"));
    }

    #[test]
    fn rechaza_un_dominio_que_solo_empieza_igual() {
        // Este era el agujero: starts_with("https://github.com/") lo aceptaba.
        assert!(!host_matches("https://github.com.evil.com/x", "github.com"));
        assert!(!host_matches("https://github.com.evil.com/", "github.com"));
        assert!(!host_matches("https://githubXcom/x", "github.com"));
    }

    #[test]
    fn rechaza_subdominios_no_listados() {
        assert!(!host_matches("https://evil.github.com/x", "github.com"));
    }

    #[test]
    fn rechaza_http_sin_tls() {
        assert!(!host_matches("http://github.com/x", "github.com"));
        assert!(!host_matches("file:///etc/passwd", "github.com"));
        assert!(!host_matches("javascript:alert(1)", "github.com"));
    }

    #[test]
    fn ignora_credenciales_y_puerto() {
        assert!(host_matches("https://user:pass@github.com/x", "github.com"));
        assert!(host_matches("https://github.com:443/x", "github.com"));
    }

    #[test]
    fn separa_el_host_de_query_y_fragmento() {
        assert!(host_matches("https://github.com?x=1", "github.com"));
        assert!(host_matches("https://github.com#top", "github.com"));
    }

    #[test]
    fn distingue_cada_dominio_de_la_lista() {
        assert!(host_matches("https://www.protondb.com/app/440", "www.protondb.com"));
        assert!(!host_matches("https://protondb.com/app/440", "www.protondb.com"));
        assert!(host_matches("https://store.steampowered.com/app/440", "store.steampowered.com"));
    }

    #[test]
    fn una_url_vacia_no_pasa() {
        assert!(!host_matches("", "github.com"));
        assert!(!host_matches("https://", "github.com"));
    }
}
