mod hw;
mod log;
mod rules;
mod steam;
mod engine;
mod profiles;
mod benchmarks;
mod launchers;

use serde::Serialize;
use tauri::async_runtime::spawn_blocking;

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
    spawn_blocking(steam::scan_library).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_store(term: String) -> Result<Vec<steam::LibGame>, String> {
    spawn_blocking(move || steam::search_store(&term)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn game_info(appid: u32) -> Result<steam::GameInfo, String> {
    spawn_blocking(move || steam::game_info(appid)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn analyze_log(path: String) -> Result<log::Stats, String> {
    spawn_blocking(move || log::analyze(&path)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    const ALLOWED: [&str; 7] = [
        "https://www.protondb.com/",
        "https://store.steampowered.com/",
        "https://docs.kernel.org/",
        "https://github.com/",
        "https://gitlab.freedesktop.org/",
        "https://wiki.archlinux.org/",
        "https://steamspy.com/",
    ];
    if !ALLOWED.iter().any(|p| url.starts_with(p)) {
        return Err("URL no permitida".into());
    }
    let mut child = std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("No pude abrir el navegador: {e}"))?;
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
    spawn_blocking(|| launchers::scan_all_launchers())
        .await
        .map_err(|e| e.to_string())
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
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la app");
}
