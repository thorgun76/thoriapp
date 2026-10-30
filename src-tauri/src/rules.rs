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
