//! Avisos de anti-cheat por juego, a partir de AreWeAntiCheatYet.
//!
//! Es el hueco más caro de la app: un juego con Easy Anti-Cheat «Denied» puede recibir
//! recomendaciones de Proton perfectamente razonables y aun así no arrancar nunca. El
//! dato viene de <https://github.com/AreWeAntiCheatYet/AreWeAntiCheatYet> (`games.json`),
//! que se descarga **una vez** y se guarda en caché: la lista cambia despacio y no tiene
//! sentido pedirla cada vez que se abre un juego.
//!
//! La clasificación (`classify`) es pura y se prueba aparte: la red y el disco están
//! separados de la lógica.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SOURCE_URL: &str =
    "https://raw.githubusercontent.com/AreWeAntiCheatYet/AreWeAntiCheatYet/master/games.json";

/// La lista se refresca como mucho una vez al día.
const MAX_AGE_SECONDS: i64 = 24 * 60 * 60;

/// Si el archivo descargado no parece esta lista, no se guarda.
const MIN_ENTRIES: usize = 20;

// ───────────────────────────── Tipos ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AntiCheatInfo {
    /// Estado: `Supported`, `Running`, `Planned`, `Broken` o `Denied` (los de la fuente).
    pub status: String,
    /// Anticheats que usa el juego (`Easy Anti-Cheat`, `BattlEye`…).
    pub anticheats: Vec<String>,
    /// Enlace de referencia de la fuente (noticia, hilo oficial…).
    pub reference: Option<String>,
    /// Segunda nota de la fuente: matices útiles («single-player works, multiplayer not»).
    pub note: Option<String>,
    /// Cuándo se actualizó el dato en la fuente (ISO 8601 que da AreWeAntiCheatYet).
    pub updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiCheatReport {
    /// `None` = el juego no está en la lista (no se sabe nada de su anti-cheat).
    pub info: Option<AntiCheatInfo>,
    /// Cuántos juegos tiene el catálogo cargado (para poder decirlo en la UI).
    pub catalog_size: usize,
    /// De dónde salió el dato: `cache` o `network`.
    pub source: String,
}

/// Entrada de `games.json`. Se permite que falte casi todo: es una fuente externa.
#[derive(Debug, Clone, Deserialize)]
struct RawGame {
    /// El nombre no se usa para buscar (se busca por appid), pero se mantiene para que
    /// los mensajes del modo de prueba manual digan algo legible.
    #[serde(default)]
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    reference: String,
    #[serde(default)]
    anticheats: Vec<String>,
    /// Pares `[texto, enlace]`; el enlace puede ser `null`.
    #[serde(default)]
    notes: Vec<Vec<Option<String>>>,
    #[serde(default)]
    updates: Vec<RawUpdate>,
    #[serde(default, rename = "storeIds")]
    store_ids: serde_json::Value,
    #[serde(default, rename = "dateChanged")]
    date_changed: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawUpdate {
    #[serde(default)]
    name: String,
    #[serde(default)]
    date: String,
}

// ───────────────────────────── Clasificación (pura) ─────────────────────────────

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Saca el appid de Steam de `storeIds`, que puede ser `{"steam": "730"}` o
/// `{"steam": {"id": "730"}}` según la entrada.
fn steam_appid(store_ids: &serde_json::Value) -> Option<u32> {
    let steam = &store_ids["steam"];
    steam
        .as_str()
        .and_then(|s| s.parse::<u32>().ok())
        .or_else(|| steam["id"].as_str().and_then(|s| s.parse::<u32>().ok()))
        .or_else(|| steam.as_u64().map(|n| n as u32))
}

/// Convierte una entrada cruda en el `AntiCheatInfo` que ve la interfaz.
///
/// La «nota» no se inventa: se prefiere la más reciente de `updates` (cuenta lo que
/// pasó al final) y, si no hay, la primera de `notes`.
fn classify(raw: &RawGame) -> AntiCheatInfo {
    let note = raw
        .updates
        .last()
        .and_then(|u| non_empty(&format!("{} ({})", u.name.trim(), u.date.trim())))
        .or_else(|| {
            raw.notes.first().and_then(|pair| {
                pair.first()
                    .and_then(|s| s.as_deref())
                    .and_then(non_empty)
            })
        });

    AntiCheatInfo {
        status: raw.status.trim().to_string(),
        anticheats: raw
            .anticheats
            .iter()
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty())
            .collect(),
        reference: non_empty(&raw.reference),
        note,
        updated: non_empty(&raw.date_changed),
    }
}

/// Busca el juego en la lista SIN parsearla: solo hace falta el appid de Steam, y así
/// no se construye la lista entera de juegos para encontrar uno.
fn find_for_appid(json: &str, appid: u32) -> Option<AntiCheatInfo> {
    let games: Vec<RawGame> = serde_json::from_str(json).ok()?;
    games
        .iter()
        .find(|g| steam_appid(&g.store_ids) == Some(appid))
        .map(classify)
        .filter(|info| !info.status.is_empty())
}

fn catalog_size(json: &str) -> usize {
    serde_json::from_str::<Vec<serde_json::Value>>(json)
        .map(|v| v.len())
        .unwrap_or(0)
}

// ───────────────────────────── Caché en disco ─────────────────────────────

fn cache_dir() -> PathBuf {
    let dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("thoriapp");
    if !dir.exists() {
        fs::create_dir_all(&dir).ok();
    }
    dir
}

fn cache_file() -> PathBuf {
    cache_dir().join("anticheat.json")
}

fn cache_age_seconds() -> Option<i64> {
    let meta = fs::metadata(cache_file()).ok()?;
    let modified = meta.modified().ok()?;
    let age = modified.elapsed().ok()?;
    Some(age.as_secs() as i64)
}

fn read_cache() -> Option<String> {
    let text = fs::read_to_string(cache_file()).ok()?;
    // Un archivo vacío o truncado por un fallo anterior no sirve.
    (catalog_size(&text) >= MIN_ENTRIES).then_some(text)
}

/// Descarga la lista y la guarda. Devuelve el texto tal cual llegó.
fn download() -> Result<String, String> {
    let body = ureq::get(SOURCE_URL)
        .set("User-Agent", "thoriapp/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| format!("no se pudo descargar la lista: {e}"))?
        .into_string()
        .map_err(|e| format!("respuesta ilegible: {e}"))?;

    if catalog_size(&body) < MIN_ENTRIES {
        return Err("la lista descargada vino vacía o incompleta".to_string());
    }
    // Si no se puede escribir la caché, el dato de esta sesión sigue siendo válido.
    fs::write(cache_file(), &body).ok();
    Ok(body)
}

/// Lista lista para usar: caché fresca si la hay; si no, red; y si la red falla, caché
/// vieja (mejor un dato de hace una semana que ningún dato).
fn load_catalog() -> Result<(String, &'static str), String> {
    let fresh = matches!(cache_age_seconds(), Some(age) if age < MAX_AGE_SECONDS);

    if fresh {
        if let Some(text) = read_cache() {
            return Ok((text, "cache"));
        }
    }

    match download() {
        Ok(text) => Ok((text, "network")),
        Err(e) => match read_cache() {
            Some(text) => Ok((text, "cache")),
            None => Err(e),
        },
    }
}

#[tauri::command]
pub fn get_anticheat_info(appid: u32) -> Result<AntiCheatReport, String> {
    if appid == 0 {
        return Ok(AntiCheatReport {
            info: None,
            catalog_size: 0,
            source: "none".to_string(),
        });
    }

    let (json, source) = load_catalog()?;
    Ok(AntiCheatReport {
        info: find_for_appid(&json, appid),
        catalog_size: catalog_size(&json),
        source: source.to_string(),
    })
}

// ───────────────────────────── Tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(json: &str) -> RawGame {
        serde_json::from_str(json).expect("JSON de prueba inválido")
    }

    #[test]
    fn steam_appid_acepta_las_dos_formas() {
        let a: serde_json::Value = serde_json::from_str(r#"{"steam":"730"}"#).unwrap();
        let b: serde_json::Value = serde_json::from_str(r#"{"steam":{"id":"570"}}"#).unwrap();
        let c: serde_json::Value = serde_json::from_str(r#"{"epic":{}}"#).unwrap();

        assert_eq!(steam_appid(&a), Some(730));
        assert_eq!(steam_appid(&b), Some(570));
        assert_eq!(steam_appid(&c), None);
    }

    #[test]
    fn classify_lee_estado_y_anticheats() {
        let g = raw(
            r#"{
                "name": "Battlefield 6",
                "status": "Denied",
                "reference": "https://example.com/bf",
                "anticheats": ["EA anticheat"],
                "storeIds": {"steam": "2807960"},
                "dateChanged": "2025-10-10T10:00:00.000Z"
            }"#,
        );

        let info = classify(&g);
        assert_eq!(info.status, "Denied");
        assert_eq!(info.anticheats, vec!["EA anticheat"]);
        assert_eq!(info.reference.as_deref(), Some("https://example.com/bf"));
        assert_eq!(info.updated.as_deref(), Some("2025-10-10T10:00:00.000Z"));
        // Sin notas ni updates no se inventa texto.
        assert!(info.note.is_none());
    }

    #[test]
    fn classify_prefiere_el_ultimo_cambio_como_nota() {
        let g = raw(
            r#"{
                "name": "Paladins",
                "status": "Running",
                "anticheats": ["Easy Anti-Cheat"],
                "notes": [["Nota vieja", null]],
                "updates": [
                    {"name": "Sin soporte", "date": "2023-02-14"},
                    {"name": "EAC actualizado", "date": "2025-04-01"}
                ],
                "storeIds": {"steam": "444090"}
            }"#,
        );

        let info = classify(&g);
        assert_eq!(info.note.as_deref(), Some("EAC actualizado (2025-04-01)"));
    }

    #[test]
    fn classify_usa_la_primera_nota_si_no_hay_cambios() {
        let g = raw(
            r#"{
                "name": "Juego",
                "status": "Broken",
                "anticheats": [],
                "notes": [["Funciona solo el modo un jugador", null]],
                "storeIds": {"steam": "1"}
            }"#,
        );
        assert_eq!(
            classify(&g).note.as_deref(),
            Some("Funciona solo el modo un jugador")
        );
    }

    #[test]
    fn find_for_appid_encuentra_el_juego_correcto() {
        let json = r#"[
            {"name":"A","status":"Supported","anticheats":["VAC"],"storeIds":{"steam":"10"}},
            {"name":"B","status":"Denied","anticheats":["EAC"],"storeIds":{"steam":"20"}}
        ]"#;

        assert_eq!(find_for_appid(json, 20).unwrap().status, "Denied");
        assert!(find_for_appid(json, 999).is_none());
    }

    #[test]
    fn find_for_appid_ignora_entradas_sin_estado() {
        let json = r#"[{"name":"A","status":"","storeIds":{"steam":"10"}}]"#;
        assert!(find_for_appid(json, 10).is_none());
    }

    #[test]
    fn find_for_appid_no_revienta_con_json_roto() {
        assert!(find_for_appid("no es json", 1).is_none());
    }

    #[test]
    fn catalog_size_cuenta_las_entradas() {
        assert_eq!(catalog_size(r#"[{},{}]"#), 2);
        assert_eq!(catalog_size("basura"), 0);
    }

    /// Prueba manual contra la red real. No corre en `npm run check` (`#[ignore]`).
    ///
    ///   cargo test --bins anticheat::tests::manual -- --ignored --nocapture
    ///
    /// Sirve para comprobar que la descarga, la caché y el appid funcionan de verdad
    /// sin tener que abrir la app. Usa el mismo camino que la app (caché de 24 h), así
    /// que la segunda ejecución dirá `origen: cache`. Para forzar la descarga, borra
    /// `~/.cache/thoriapp/anticheat.json`.
    #[test]
    #[ignore]
    fn manual_descarga_y_busca_appids_reales() {
        let (json, source) = load_catalog().expect("no se pudo obtener la lista");
        let games: Vec<RawGame> = serde_json::from_str(&json).unwrap_or_default();
        println!("catálogo: {} juegos (origen: {source})", games.len());
        println!("caché en: {}", cache_file().display());

        let library = crate::steam::scan_library();
        if library.is_empty() {
            println!("no se encontró biblioteca de Steam; se prueban appids fijos");
        }
        let mut with_status = 0;
        for game in &library {
            if let Some(info) = find_for_appid(&json, game.appid) {
                with_status += 1;
                println!(
                    "{:>8}  {:<45} {:<10} {:?}",
                    game.appid, game.name, info.status, info.anticheats
                );
            }
        }
        println!(
            "{}/{} juegos instalados tienen dato de anti-cheat",
            with_status,
            library.len()
        );

        // Battlefield 6 y Battlefield 2042: bloqueados por el anticheat de EA.
        for appid in [2807960u32, 1517290] {
            match find_for_appid(&json, appid) {
                Some(info) => println!(
                    "{appid}: {} {:?} nota={:?}",
                    info.status, info.anticheats, info.note
                ),
                None => println!("{appid}: no está en el catálogo"),
            }
        }
    }
}
