//! Reportes reales de Proton Pulse, filtrados por la GPU del equipo.
//!
//! La tabla genérica de `engine::get_hardware_recommendations` dice «esto suele ir
//! bien», sin saber qué tarjeta tienes. Proton Pulse es lo contrario: son reportes
//! de gente que ha lanzado ESE juego en Linux, y cada uno trae su GPU, su versión
//! de Proton, su nota y sus opciones de lanzamiento. Filtrarlos por la GPU del
//! equipo convierte «una RX 580 va bien» en «esta RX 580 va bien en este juego, con
//! esta versión de Proton y estas opciones».
//!
//! Fuente: <https://data.proton-pulse.com/data/{appId}/latest.json>. Se cachea en
//! disco y se refresca una vez al día, igual que la lista de anti-cheat.
//!
//! Decisiones de honestidad:
//!
//!   - `gpuVendor` **no es fiable**: en el feed real falta en la mayoría de los
//!     reportes (164 reportes de Cyberpunk 2077, uno solo con `gpuVendor`). Por eso
//!     el vendor se deduce del **nombre** de la GPU, que sí viene siempre, y
//!     `gpuVendor` solo se usa como confirmación cuando existe.
//!   - Nunca se dice «funciona en tu GPU» a partir de un reporte suelto si no hay
//!     ninguno de tu vendor: se dice cuántos hay, sin veredicto.
//!   - Las opciones de lanzamiento de otros usuarios se muestran tal cual, sin
//!     fusionarlas ni recomendarlas: cada una fue escrita para un juego y una GPU
//!     concretos.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const BASE_URL: &str = "https://data.proton-pulse.com/data";

/// Los reportes de un juego cambian despacio; se refresca una vez al día.
const MAX_AGE_SECONDS: i64 = 24 * 60 * 60;

// ───────────────────────────── Tipos (API) ─────────────────────────────

/// Un reporte, ya recortado a lo que se muestra. Casi todos los campos del feed
/// sobran para esta app; se ignoran al deserializar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Report {
    /// Nombre de la GPU tal cual lo reportó el usuario (con el driver incluido).
    pub gpu: String,
    /// `amd`, `nvidia` o `intel`, deducido del nombre si el feed no lo trae.
    pub gpu_vendor: String,
    /// Nota de Proton Pulse: `platinum`, `gold`, `silver`, `bronze` o `borked`.
    pub rating: String,
    pub proton_version: String,
    pub duration: Option<String>,
    /// Opciones de lanzamiento del usuario. `None` si no puso ninguna.
    pub launch_options: Option<String>,
}

/// Resumen para la UI: cuántos reportes hay y cómo salen en esta GPU.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportSummary {
    /// Vendor que se buscó (`amd`/`nvidia`/`intel`), o `None` si no se supo.
    pub vendor: Option<String>,
    /// Reportes del juego en total, de todos los vendors.
    pub total: usize,
    /// Reportes de este vendor.
    pub matching: usize,
    /// Cuántos de los anteriores son `platinum`/`gold`.
    pub good: usize,
    /// Cuántos son `borked` (no arrancan con esa GPU).
    pub borked: usize,
    /// Cuántos usan la misma familia de GPU (`RX 6800 XT` → familia `rx 6000`).
    pub same_family: usize,
    /// Los reportes de este vendor, ordenados de mejor a peor nota.
    pub reports: Vec<ReportSummaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportSummaryEntry {
    pub gpu: String,
    pub rating: String,
    pub proton_version: String,
    pub launch_options: Option<String>,
}

/// Identificador del juego en el feed. Proton Pulse usa el appid de Steam como
/// texto; para otros launchers no hay ficha, y se devuelve `None`.
pub fn feed_id(appid: u32) -> Option<u32> {
    (appid != 0).then_some(appid)
}

// ───────────────────────────── Lógica pura ─────────────────────────────

/// El vendor de una GPU por su nombre.
///
/// Se busca por marca y no por orden de aparición porque los nombres reales del
/// feed son de todos los gustos: `AMD Radeon RX 7800 XT (radeonsi, navi32, …)`,
/// `NVIDIA GeForce RTX 3070`, `Mesa zink Vulkan 1.4(NVIDIA GeForce RTX 5070 (…))`.
/// Una iGPU de Intel aparece como `Intel(R) UHD Graphics 630`.
pub fn vendor_from_gpu_name(gpu: &str) -> Option<&'static str> {
    let l = gpu.to_lowercase();
    if l.is_empty() {
        return None;
    }
    // `zink` sobre NVIDIA es el driver, pero sigue siendo una tarjeta NVIDIA: el
    // nombre del modelo gana al driver. Se comprueba NVIDIA/AMD antes que Mesa.
    if l.contains("nvidia") || l.contains("geforce") || l.contains("quadro") || l.contains("rtx") || l.contains("gtx") {
        return Some("nvidia");
    }
    if l.contains("amd") || l.contains("radeon") || l.contains("ati ") || l.contains("ryzen") {
        return Some("amd");
    }
    if l.contains("intel") || l.contains("arc a") || l.contains("iris") || l.contains("uhd graphics") {
        return Some("intel");
    }
    // Drivers sin marca reconocible (`llvmpipe`, `lavapipe`): sin vendor, y por
    // tanto fuera del filtro.
    None
}

/// Familias de GPU que se consideran «la misma tarjeta» a efectos de rendimiento.
///
/// `RX 6800 XT` y `RX 6600` no rinden igual, pero un reporte de una 6700 XT es una
/// referencia mucho más útil para un dueño de 6800 XT que el promedio de todas las
/// GPUs del mundo. Se agrupa por generación, no por modelo exacto.
pub fn family_key(gpu: &str) -> Option<String> {
    let l = gpu.to_lowercase();
    for (prefijo, familia) in [
        // NVIDIA por generación.
        ("rtx 50", "nvidia-rtx-50"),
        ("rtx 40", "nvidia-rtx-40"),
        ("rtx 30", "nvidia-rtx-30"),
        ("rtx 20", "nvidia-rtx-20"),
        ("gtx 16", "nvidia-gtx-16"),
        ("gtx 10", "nvidia-gtx-10"),
        // AMD por generación (RDNA 4, 3, 2, 1) y las Polaris/Vega.
        ("rx 9", "amd-rdna4"),
        ("rx 8", "amd-rdna4"),
        ("rx 7", "amd-rdna3"),
        ("rx 6", "amd-rdna2"),
        ("rx 5", "amd-rdna1-polaris"),
        ("vega", "amd-vega"),
    ] {
        if l.contains(prefijo) {
            return Some(familia.to_string());
        }
    }
    None
}

/// Peso de una nota: para ordenar sin inventar. Las notas son las de ProtonDB.
fn rating_weight(rating: &str) -> u8 {
    match rating.trim().to_lowercase().as_str() {
        "platinum" => 5,
        "gold" => 4,
        "silver" => 3,
        "bronze" => 2,
        "borked" => 1,
        _ => 0,
    }
}

/// Resumen de los reportes de un vendor, listo para la UI.
///
/// Devuelve `None` si no hay ningún reporte utilizable: sin dato, no hay tarjeta
/// que mostrar (mejor nada que un «0 reportes» indistinguible de un error de red).
pub fn summarize(reports: &[Report], vendor: Option<&str>) -> Option<ReportSummary> {
    if reports.is_empty() {
        return None;
    }
    let vendor = vendor.map(|v| v.to_lowercase());

    let mut matching: Vec<&Report> = reports
        .iter()
        .filter(|r| match vendor.as_deref() {
            Some(v) => r.gpu_vendor.eq_ignore_ascii_case(v),
            // Sin vendor conocido no se puede filtrar: se usan todos.
            None => true,
        })
        .collect();

    if matching.is_empty() {
        return None;
    }

    // Mejor nota primero; a igualdad, la versión de Proton más nueva primero (los
    // reportes viejos son justo los que ya no se reproducen).
    matching.sort_by(|a, b| {
        rating_weight(&b.rating)
            .cmp(&rating_weight(&a.rating))
            .then_with(|| b.proton_version.cmp(&a.proton_version))
    });

    // Familia «representativa»: la del modelo más concreto del vendor. Se usa el
    // reporte con más peso de nota y, a igualdad, el nombre más largo, porque un
    // nombre corto suele ser genérico («AMD Radeon») y agruparía medio catálogo.
    let referencia: Option<String> = matching
        .iter()
        .max_by_key(|r| (rating_weight(&r.rating), r.gpu.len()))
        .and_then(|r| family_key(&r.gpu));
    let same_family = match referencia.as_deref() {
        Some(fam) => matching
            .iter()
            .filter(|r| family_key(&r.gpu).as_deref() == Some(fam))
            .count(),
        None => 0,
    };

    let good = matching
        .iter()
        .filter(|r| matches!(rating_weight(&r.rating), 4 | 5))
        .count();
    let borked = matching
        .iter()
        .filter(|r| rating_weight(&r.rating) == 1)
        .count();

    Some(ReportSummary {
        vendor: vendor.clone(),
        total: reports.len(),
        matching: matching.len(),
        good,
        borked,
        same_family,
        reports: matching
            .into_iter()
            .map(|r| ReportSummaryEntry {
                gpu: r.gpu.clone(),
                rating: r.rating.clone(),
                proton_version: r.proton_version.clone(),
                launch_options: r.launch_options.clone(),
            })
            .collect(),
    })
}

// ───────────────────────────── Feed (red + caché) ─────────────────────────────

/// Entrada cruda del feed. Casi todos los campos se ignoran: `formResponses` es
/// un objeto enorme que no aporta nada a esta app.
#[derive(Debug, Clone, Deserialize)]
struct RawReport {
    #[serde(default)]
    gpu: String,
    #[serde(default, rename = "gpuVendor")]
    gpu_vendor: Option<String>,
    #[serde(default)]
    rating: String,
    #[serde(default, rename = "protonVersion")]
    proton_version: String,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default, rename = "launchOptions")]
    launch_options: Option<String>,
}

/// Convierte el JSON del feed en reportes. `gpu_vendor` se deduce del nombre
/// cuando el feed no lo trae (que es lo normal).
pub fn parse_feed(json: &str) -> Vec<Report> {
    let raw: Vec<RawReport> = serde_json::from_str(json).unwrap_or_default();
    raw.into_iter()
        .filter_map(|r| {
            let gpu = r.gpu.trim().to_string();
            let vendor = vendor_from_gpu_name(&gpu)
                .map(|v| v.to_string())
                // Si el nombre no dice nada, se acepta el del feed; si tampoco hay,
                // el reporte no se puede filtrar por hardware y se conserva con
                // vendor vacío (se cuenta en `total`, no en `matching`).
                .or_else(|| {
                    r.gpu_vendor
                        .as_deref()
                        .map(|v| v.trim().to_lowercase())
                        .filter(|v| !v.is_empty())
                })
                .unwrap_or_default();
            let rating = r.rating.trim().to_lowercase();
            if gpu.is_empty() || rating.is_empty() {
                return None;
            }
            Some(Report {
                gpu,
                gpu_vendor: vendor,
                rating,
                proton_version: r.proton_version.trim().to_string(),
                duration: r.duration.and_then(|d| non_empty(&d)),
                launch_options: r.launch_options.as_deref().and_then(non_empty),
            })
        })
        .collect()
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn cache_dir() -> PathBuf {
    let dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("thoriapp")
        .join("proton-pulse");
    if !dir.exists() {
        fs::create_dir_all(&dir).ok();
    }
    dir
}

fn cache_file(appid: u32) -> PathBuf {
    cache_dir().join(format!("{appid}.json"))
}

fn cache_age_seconds(appid: u32) -> Option<i64> {
    let meta = fs::metadata(cache_file(appid)).ok()?;
    let age = meta.modified().ok()?.elapsed().ok()?;
    Some(age.as_secs() as i64)
}

fn download(appid: u32) -> Result<String, String> {
    let url = format!("{BASE_URL}/{appid}/latest.json");
    let body = ureq::get(&url)
        .set("User-Agent", "thoriapp/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| format!("no se pudieron leer los reportes de Proton Pulse: {e}"))?
        .into_string()
        .map_err(|e| format!("respuesta ilegible de Proton Pulse: {e}"))?;

    // El feed de un juego sin reportes es `[]`, que es válido: se guarda para no
    // repedirlo en cada apertura. Lo que no se guarda es algo que no parsea.
    if serde_json::from_str::<Vec<RawReport>>(&body).is_err() {
        return Err("los reportes descargados no tienen el formato esperado".to_string());
    }
    fs::write(cache_file(appid), &body).ok();
    Ok(body)
}

/// JSON del feed: caché fresca; si no, red; y si la red falla, caché vieja.
fn load_feed(appid: u32) -> Result<(Vec<Report>, &'static str), String> {
    let fresh = matches!(cache_age_seconds(appid), Some(age) if age < MAX_AGE_SECONDS);
    if fresh {
        if let Ok(text) = fs::read_to_string(cache_file(appid)) {
            return Ok((parse_feed(&text), "cache"));
        }
    }
    match download(appid) {
        Ok(text) => Ok((parse_feed(&text), "network")),
        Err(e) => match fs::read_to_string(cache_file(appid)) {
            // Un dato de hace una semana sigue siendo mejor que ningún dato.
            Ok(text) => Ok((parse_feed(&text), "cache")),
            Err(_) => Err(e),
        },
    }
}

/// Reportes del juego en la GPU del equipo.
///
/// `gpu` es el nombre que devuelve `get_hardware`. `Ok(None)` = el juego no tiene
/// reportes (o no tiene appid de Steam); `Err` = no se pudo leer el feed.
#[tauri::command]
pub fn get_proton_pulse_reports(
    appid: u32,
    gpu: Option<String>,
) -> Result<Option<ReportSummary>, String> {
    let Some(id) = feed_id(appid) else {
        return Ok(None);
    };
    let (reports, _source) = load_feed(id)?;
    if reports.is_empty() {
        return Ok(None);
    }
    // El vendor sale del nombre de la GPU del equipo (el mismo criterio que usa el
    // filtro de los reportes), no de los tags: un tag puede faltar y el nombre no.
    let vendor = gpu.as_deref().and_then(vendor_from_gpu_name);
    Ok(summarize(&reports, vendor))
}

// ───────────────────────────── Tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"[
        {"gpu":"NVIDIA GeForce GTX 1660 Ti","rating":"gold","protonVersion":"10.0-3"},
        {"gpu":"AMD Radeon RX 7800 XT (radeonsi, navi32, LLVM 17.0.6, DRM 3.57)","rating":"gold","protonVersion":"10.0-3","launchOptions":"PROTON_FSR4_UPGRADE=1 %command%"},
        {"gpu":"AMD Radeon (radeonsi, renoir, ACO, DRM 3.64)","rating":"borked","protonVersion":"10.0-3"},
        {"gpu":"Mesa zink Vulkan 1.4(NVIDIA GeForce RTX 5070 (NVIDIA_PROPRIETARY))","rating":"bronze","protonVersion":"10.0-3"},
        {"gpu":"AMD Radeon RX 6800 XT (RADV NAVI21)","rating":"platinum","protonVersion":"9.0-4"}
    ]"#;

    #[test]
    fn el_vendor_sale_del_nombre_y_no_del_campo_opcional() {
        // En el feed real `gpuVendor` falta casi siempre: si se dependiera de él,
        // el filtro por GPU no encontraría nada.
        assert_eq!(vendor_from_gpu_name("NVIDIA GeForce RTX 3070"), Some("nvidia"));
        assert_eq!(
            vendor_from_gpu_name("AMD Radeon RX 7800 XT (radeonsi, navi32, LLVM 17.0.6)"),
            Some("amd")
        );
        assert_eq!(vendor_from_gpu_name("Intel(R) UHD Graphics 630"), Some("intel"));
        // zink sobre NVIDIA sigue siendo NVIDIA.
        assert_eq!(
            vendor_from_gpu_name("Mesa zink Vulkan 1.4(NVIDIA GeForce RTX 5070 (NVIDIA_PROPRIETARY))"),
            Some("nvidia")
        );
        // Sin marca reconocible no hay vendor: fuera del filtro.
        assert_eq!(vendor_from_gpu_name("llvmpipe (LLVM 17.0.6, 256 bits)"), None);
        assert_eq!(vendor_from_gpu_name(""), None);
    }

    #[test]
    fn parse_feed_deduce_el_vendor_cuando_el_feed_no_lo_trae() {
        let reports = parse_feed(FEED);
        assert_eq!(reports.len(), 5);
        let amd = reports.iter().find(|r| r.gpu.starts_with("AMD Radeon RX 7800")).unwrap();
        assert_eq!(amd.gpu_vendor, "amd");
        assert_eq!(amd.rating, "gold");
        assert_eq!(amd.proton_version, "10.0-3");
        assert_eq!(
            amd.launch_options.as_deref(),
            Some("PROTON_FSR4_UPGRADE=1 %command%")
        );
    }

    #[test]
    fn parse_feed_usa_el_vendor_del_feed_si_el_nombre_no_dice_nada() {
        let json = r#"[{"gpu":"GPU rara","gpuVendor":"amd","rating":"gold","protonVersion":"9"}]"#;
        let r = parse_feed(json);
        assert_eq!(r[0].gpu_vendor, "amd");
    }

    #[test]
    fn parse_feed_descarta_reportes_sin_gpu_o_sin_nota() {
        // Un reporte sin GPU no se puede filtrar y una nota vacía no dice nada.
        let json = r#"[
            {"gpu":"","rating":"gold","protonVersion":"9"},
            {"gpu":"NVIDIA GeForce RTX 3060","rating":"","protonVersion":"9"},
            {"gpu":"NVIDIA GeForce RTX 3060","rating":"gold","protonVersion":"9"}
        ]"#;
        assert_eq!(parse_feed(json).len(), 1);
    }

    #[test]
    fn parse_feed_no_revienta_con_json_roto() {
        assert!(parse_feed("no es json").is_empty());
        assert!(parse_feed("").is_empty());
    }

    #[test]
    fn summarize_filtra_por_el_vendor_del_equipo() {
        let reports = parse_feed(FEED);
        let s = summarize(&reports, Some("amd")).unwrap();

        assert_eq!(s.total, 5);
        assert_eq!(s.matching, 3, "tres reportes AMD en el feed");
        assert!(s.reports.iter().all(|r| {
            let l = r.gpu.to_lowercase();
            l.contains("amd") || l.contains("radeon")
        }));
        // platinum (6800 XT) + gold (7800 XT) son los dos buenos.
        assert_eq!(s.good, 2);
        assert_eq!(s.borked, 1, "la iGPU renoir viene como borked");
    }

    #[test]
    fn summarize_ordena_de_mejor_a_peor_nota() {
        let reports = parse_feed(FEED);
        let s = summarize(&reports, Some("amd")).unwrap();
        let notas: Vec<&str> = s.reports.iter().map(|r| r.rating.as_str()).collect();
        assert_eq!(notas, vec!["platinum", "gold", "borked"]);
    }

    #[test]
    fn summarize_cuenta_la_misma_familia_de_gpu() {
        // Dos RDNA 3 (7800 XT + 7600) frente a una RDNA 2. La familia que se cuenta
        // es la del modelo de referencia (la mejor nota), no la de la tabla.
        let json = r#"[
            {"gpu":"AMD Radeon RX 7800 XT","rating":"platinum","protonVersion":"10"},
            {"gpu":"AMD Radeon RX 7600 (RADV NAVI33)","rating":"silver","protonVersion":"10"},
            {"gpu":"AMD Radeon RX 6800 XT (RADV NAVI21)","rating":"gold","protonVersion":"10"}
        ]"#;
        let s = summarize(&parse_feed(json), Some("amd")).unwrap();
        // platinum primero: es la 7800 XT, y su familia (RDNA 3) tiene dos reportes.
        assert_eq!(s.reports[0].rating, "platinum");
        assert!(s.reports[0].gpu.contains("7800"));
        assert_eq!(s.same_family, 2, "la 6800 XT es otra generacion");
    }

    #[test]
    fn sin_reportes_de_tu_vendor_no_hay_resumen() {
        // Un feed de solo NVIDIA no permite decir nada de una GPU AMD. Mejor no
        // enseñar tarjeta que enseñar «0 reportes de tu GPU» como si fuera un dato.
        let json = r#"[{"gpu":"NVIDIA GeForce RTX 3070","rating":"gold","protonVersion":"10"}]"#;
        assert!(summarize(&parse_feed(json), Some("amd")).is_none());
    }

    #[test]
    fn sin_vendor_conocido_se_usan_todos_los_reportes() {
        let reports = parse_feed(FEED);
        let s = summarize(&reports, None).unwrap();
        assert_eq!(s.matching, 5);
        assert_eq!(s.vendor, None);
    }

    #[test]
    fn un_feed_vacio_no_produce_resumen() {
        assert!(summarize(&[], Some("amd")).is_none());
    }

    #[test]
    fn family_key_agrupa_por_generacion() {
        assert_eq!(family_key("AMD Radeon RX 7800 XT"), family_key("RX 7600"));
        assert_ne!(family_key("RX 7800 XT"), family_key("RX 6800 XT"));
        assert_eq!(family_key("NVIDIA GeForce RTX 3060"), family_key("RTX 3090"));
        assert_eq!(family_key("llvmpipe"), None);
    }

    #[test]
    fn feed_id_no_inventa_appid() {
        assert_eq!(feed_id(292030), Some(292030));
        // appid 0 = juego de otro launcher: no hay ficha en Proton Pulse.
        assert_eq!(feed_id(0), None);
    }
}
