use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct Param {
    pub kind: String,
    pub value: String,
    pub title: String,
    pub why: String,
    pub impact: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub default_on: bool,
    #[serde(default)]
    pub warn: Option<String>,
    #[serde(default = "heur")]
    pub evidence: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub proton_only: bool,
    #[serde(default)]
    pub recommended_for: Vec<String>,
    #[serde(default)]
    pub tested_on: Option<String>,
    #[serde(default)]
    pub validated_date: Option<String>,
    #[serde(default)]
    pub measured_improvement: Option<String>,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub not_recommended_for: Vec<String>,
    #[serde(default)]
    pub essential: Option<bool>,
    #[serde(default)]
    pub simple_why: Option<String>,
}

fn heur() -> String {
    "heuristica".into()
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Game {
    pub name: String,
    pub appid: u32,
    pub bottleneck: String,
    pub summary: String,
    pub params: Vec<Param>,
    #[serde(default)]
    pub tips: Vec<String>,
    #[serde(default)]
    pub has_rules: bool,
}

fn is_proton_only(v: &str) -> bool {
    ["PROTON_", "DXVK_", "VKD3D_", "STAGING_", "WINE"].iter().any(|p| v.starts_with(p))
}

pub fn for_appid(appid: u32) -> Game {
    let games: Vec<Game> = serde_json::from_str(include_str!("../games.json")).expect("games.json inválido");
    let found = games.iter().find(|g| g.appid == appid).cloned();
    let has_rules = found.is_some();
    let mut g = found.unwrap_or_else(|| games.into_iter().find(|g| g.appid == 0).expect("falta perfil genérico"));
    g.has_rules = has_rules;
    for p in &mut g.params {
        p.proton_only = is_proton_only(&p.value);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todos() -> Vec<Game> {
        serde_json::from_str(include_str!("../games.json")).expect("games.json inválido")
    }

    #[test]
    fn hay_un_perfil_generico_y_no_esta_duplicado() {
        let games = todos();
        let genericos: Vec<_> = games.iter().filter(|g| g.appid == 0).collect();
        assert_eq!(genericos.len(), 1, "exactamente un perfil genérico (appid 0)");
    }

    #[test]
    fn cada_juego_tiene_reglas_propias_y_no_solo_el_generico() {
        // El bug original: `games.json` solo tenía "Genérico", así que la app no
        // añadía nada específico por juego y todo el mundo recibía el mismo perfil.
        let games = todos();
        let con_reglas: Vec<_> = games.iter().filter(|g| g.appid != 0).collect();
        assert!(
            con_reglas.len() >= 5,
            "se esperan varios juegos con reglas propias, hay {}",
            con_reglas.len()
        );
    }

    #[test]
    fn no_hay_appids_repetidos() {
        // Un appid duplicado hace que el segundo nunca se use (o el primero, según
        // el orden): el juego recibiría el perfil de otro.
        let games = todos();
        let mut ids: Vec<u32> = games.iter().map(|g| g.appid).collect();
        ids.sort_unstable();
        let antes = ids.len();
        ids.dedup();
        assert_eq!(antes, ids.len(), "appid repetido en games.json");
    }

    #[test]
    fn ningun_juego_se_llama_igual_que_el_generico() {
        let games = todos();
        for g in games.iter().filter(|g| g.appid != 0) {
            assert_ne!(g.name, "Genérico (cualquier juego)", "appid {}", g.appid);
            assert!(!g.summary.trim().is_empty(), "{} sin resumen", g.name);
            assert!(!g.params.is_empty(), "{} sin parámetros", g.name);
        }
    }

    #[test]
    fn un_parametro_a_evitar_nunca_viene_activado() {
        // `impact: "evitar"` son justo los que la app desaconseja (RT forzado,
        // fast clears, tocar el anti-cheat). Activarlos por defecto sería el bug de
        // `VKD3D_CONFIG=dxr` otra vez, uno por juego.
        for g in todos() {
            for p in &g.params {
                if p.impact == "evitar" {
                    assert!(!p.default_on, "{} activa por defecto un «evitar»: {}", g.name, p.value);
                }
            }
        }
    }

    #[test]
    fn ningun_juego_recomienda_rt_por_defecto() {
        // El RT se decide por GPU en `engine.rs`, no por juego: aquí solo puede
        // aparecer como advertencia.
        for g in todos() {
            for p in &g.params {
                if p.value.to_lowercase().contains("vkd3d_config") && p.value.contains("dxr") {
                    assert!(!p.default_on, "{} activa VKD3D_CONFIG=dxr por defecto", g.name);
                    assert!(
                        p.warn.is_some(),
                        "{} ofrece VKD3D_CONFIG=dxr sin aviso", g.name
                    );
                }
            }
        }
    }

    #[test]
    fn los_parametros_marcan_proton_only_segun_la_variable() {
        let g = for_appid(1091500);
        let nvapi = g.params.iter().find(|p| p.value.contains("PROTON_ENABLE_NVAPI")).unwrap();
        assert!(nvapi.proton_only, "PROTON_* es solo-Proton");
        let mangohud = g.params.iter().find(|p| p.value == "mangohud").unwrap();
        assert!(!mangohud.proton_only, "un wrapper no es solo-Proton");
    }

    #[test]
    fn un_juego_sin_reglas_cae_al_generico_sin_decirlo() {
        let g = for_appid(999999);
        assert!(!g.has_rules, "sin reglas propias, `has_rules` debe ser false");
        assert_eq!(g.appid, 0, "y se usa el perfil genérico");
    }

    #[test]
    fn un_juego_con_reglas_las_usa() {
        let g = for_appid(1091500);
        assert!(g.has_rules);
        assert_eq!(g.name, "Cyberpunk 2077");
        assert_eq!(g.appid, 1091500);
    }

    #[test]
    fn el_generico_tambien_marca_proton_only() {
        // La marca se aplica después de elegir el perfil: el camino del genérico
        // no debe quedarse sin ella.
        let g = for_appid(999999);
        for p in &g.params {
            assert_eq!(p.proton_only, is_proton_only(&p.value), "{} mal marcado", p.value);
        }
    }
}
