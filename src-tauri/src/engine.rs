use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEngine {
    pub name: String,
    pub version: Option<String>,
    pub api: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityRecommendation {
    pub parameter: String,
    pub description: String,
    pub optimized_for: Vec<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub recommended: bool,
}

fn get_known_engines() -> HashMap<u32, (&'static str, &'static str)> {
    let mut engines = HashMap::new();
    
    engines.insert(730, ("Unity", "vulkan"));
    engines.insert(251570, ("Unity", "dxvk"));
    engines.insert(1145360, ("Unity", "dxvk"));
    engines.insert(588650, ("Unity", "dxvk"));
    engines.insert(1057090, ("Unity", "dxvk"));
    engines.insert(367520, ("Unity", "dxvk"));
    engines.insert(1092790, ("Unity", "dxvk"));
    engines.insert(1794680, ("Unity", "dxvk"));
    engines.insert(413150, ("Unity", "dxvk"));
    engines.insert(105600, ("Unity", "dxvk"));
    engines.insert(268910, ("Unity", "dxvk"));
    engines.insert(504230, ("Unity", "dxvk"));
    
    engines.insert(1174180, ("Unreal Engine", "dxvk"));
    engines.insert(271590, ("Unreal Engine", "dxvk"));
    engines.insert(1245620, ("Unreal Engine", "dxvk"));
    engines.insert(1086940, ("Unreal Engine", "dxvk"));
    engines.insert(1222670, ("Unreal Engine", "dxvk"));
    engines.insert(892970, ("Unreal Engine", "dxvk"));
    engines.insert(553850, ("Unreal Engine", "dxvk"));
    engines.insert(1172470, ("Unreal Engine", "dxvk"));
    engines.insert(1085660, ("Unreal Engine", "dxvk"));
    engines.insert(381210, ("Unreal Engine", "dxvk"));
    
    engines.insert(570, ("Source 2", "vulkan"));
    engines.insert(440, ("Source", "opengl"));
    engines.insert(220, ("Source", "opengl"));
    engines.insert(400, ("Source", "opengl"));
    engines.insert(620, ("Source", "opengl"));
    engines.insert(500, ("Source", "opengl"));
    engines.insert(550, ("Source", "opengl"));
    
    engines.insert(413150, ("Godot", "vulkan"));
    engines.insert(1091500, ("REDengine 4", "vkd3d"));
    engines.insert(292030, ("Witcher Engine", "dxvk"));
    
    engines
}

pub fn detect_engine(appid: u32, game_name: &str) -> Option<GameEngine> {
    let engines = get_known_engines();
    
    if let Some((name, api)) = engines.get(&appid) {
        return Some(GameEngine {
            name: name.to_string(),
            version: None,
            api: api.to_string(),
        });
    }
    
    let name_lower = game_name.to_lowercase();
    
    if name_lower.contains("unity") {
        return Some(GameEngine {
            name: "Unity".to_string(),
            version: None,
            api: "dxvk".to_string(),
        });
    }
    
    if name_lower.contains("unreal") {
        return Some(GameEngine {
            name: "Unreal Engine".to_string(),
            version: None,
            api: "dxvk".to_string(),
        });
    }
    
    if name_lower.contains("godot") {
        return Some(GameEngine {
            name: "Godot".to_string(),
            version: None,
            api: "vulkan".to_string(),
        });
    }
    
    if name_lower.contains("rpg maker") {
        return Some(GameEngine {
            name: "RPG Maker".to_string(),
            version: None,
            api: "dxvk".to_string(),
        });
    }
    
    None
}

pub fn get_engine_recommendations(engine: &GameEngine) -> Vec<String> {
    match engine.name.as_str() {
        "Unity" => vec![
            "PROTON_FORCE_LARGE_ADDRESS_AWARE=1".to_string(),
            "PROTON_USE_NTSYNC=1".to_string(),
        ],
        "Unreal Engine" => vec![
            "PROTON_USE_NTSYNC=1".to_string(),
            "DXVK_ASYNC=1".to_string(),
        ],
        "Source" | "Source 2" => vec![
            "-vulkan".to_string(),
            "-novid".to_string(),
        ],
        "REDengine 4" => vec![
            "VKD3D_CONFIG=dxr".to_string(),
            "PROTON_ENABLE_NVAPI=1".to_string(),
        ],
        "Godot" => vec![
            "PROTON_USE_NTSYNC=1".to_string(),
        ],
        "FromSoftware Engine" => vec![
            "PROTON_USE_NTSYNC=1".to_string(),
        ],
        _ => vec![],
    }
}

pub fn get_hardware_recommendations() -> Vec<CommunityRecommendation> {
    vec![
        CommunityRecommendation {
            parameter: "AMD_VULKAN_ICD=RADV".to_string(),
            description: "Fuerza el uso del driver RADV en lugar de AMDVLK".to_string(),
            optimized_for: vec!["amd".to_string()],
            notes: Some("RADV es el driver Vulkan open-source de Mesa, generalmente más rápido para gaming que AMDVLK. No afecta a juegos nativos de Linux que usan OpenGL".to_string()),
            recommended: true,
        },
        CommunityRecommendation {
            parameter: "RADV_PERFTEST=sam".to_string(),
            description: "Activa Smart Access Memory para mejor rendimiento CPU-GPU".to_string(),
            optimized_for: vec!["amd".to_string(), "ryzen".to_string()],
            notes: Some("Permite a la CPU acceder a toda la VRAM de la GPU de una vez. Solo funciona con CPU AMD Ryzen serie 3000+ y GPU AMD RX 5000+. Puede dar 5-10% más de FPS en juegos limitados por CPU".to_string()),
            recommended: false,
        },
        CommunityRecommendation {
            parameter: "mesa_glthread=true".to_string(),
            description: "Habilita el threaded OpenGL para mejor rendimiento en juegos OpenGL".to_string(),
            optimized_for: vec!["amd".to_string(), "intel".to_string()],
            notes: Some("Mueve el procesamiento de OpenGL a un hilo separado, reduciendo el cuello de botella del hilo principal. Útil para juegos antiguos que usan OpenGL en lugar de Vulkan. Puede dar 10-20% más de FPS en algunos títulos".to_string()),
            recommended: true,
        },
        CommunityRecommendation {
            parameter: "PROTON_ENABLE_NVAPI=1".to_string(),
            description: "Habilita NVAPI para usar DLSS y otras tecnologías NVIDIA".to_string(),
            optimized_for: vec!["nvidia".to_string(), "rtx".to_string()],
            notes: Some("Requiere GPU NVIDIA RTX serie 2000 o superior. Habilita DLSS (Deep Learning Super Sampling) que puede dar 30-50% más de FPS con mínima pérdida de calidad. También habilita Reflex para menor input lag".to_string()),
            recommended: false,
        },
        CommunityRecommendation {
            parameter: "__GLX_VENDOR_LIBRARY_NAME=nvidia".to_string(),
            description: "Fuerza el uso del driver propietario NVIDIA en lugar de Nouveau".to_string(),
            optimized_for: vec!["nvidia".to_string()],
            notes: Some("Asegura que se use el driver propietario NVIDIA (mejor rendimiento) en lugar del driver open-source Nouveau. Importante si tienes ambos instalados".to_string()),
            recommended: true,
        },
        CommunityRecommendation {
            parameter: "__NV_PRIME_RENDER_OFFLOAD=1".to_string(),
            description: "Activa render offload para laptops con GPU dedicada NVIDIA".to_string(),
            optimized_for: vec!["nvidia".to_string(), "laptop".to_string()],
            notes: Some("Solo para laptops con tecnología NVIDIA Optimus (Intel/AMD iGPU + NVIDIA dGPU). Permite use la GPU dedicada para el juego mientras la integrada maneja el display. Ahorra batería cuando no gaming".to_string()),
            recommended: false,
        },
        CommunityRecommendation {
            parameter: "PROTON_USE_NTSYNC=1".to_string(),
            description: "Sincronización de hilos mejorada del kernel para mejor rendimiento multihilo".to_string(),
            optimized_for: vec!["all".to_string()],
            notes: Some("Requiere kernel Linux 6.14 o superior. Reemplaza las antiguas técnicas esync/fsync con una implementación nativa del kernel. Reduce la sobrecarga de sincronización entre hilos, mejorando FPS en juegos multihilo. Si tu kernel es antiguo, ignora este parámetro".to_string()),
            recommended: true,
        },
        CommunityRecommendation {
            parameter: "gamemoderun".to_string(),
            description: "Feral GameMode: optimiza el sistema para gaming".to_string(),
            optimized_for: vec!["all".to_string()],
            notes: Some("Cambia el governor de CPU a performance, ajusta prioridades de I/O, desactiva screensavers y optimiza el sistema para el juego. Requiere instalar: sudo apt install gamemode (Debian/Ubuntu) o sudo pacman -S gamemode (Arch). Muy recomendado para juegos limitados por CPU".to_string()),
            recommended: true,
        },
        CommunityRecommendation {
            parameter: "mangohud".to_string(),
            description: "Overlay de monitoreo de rendimiento en tiempo real".to_string(),
            optimized_for: vec!["all".to_string()],
            notes: Some("Muestra FPS, frametime, uso de CPU/GPU, temperaturas y más. Presiona Shift+F2 para grabar logs y analizar rendimiento. Esencial para medir el impacto real de cada parámetro. Requiere instalar: sudo apt install mangohud (Debian/Ubuntu) o sudo pacman -S mangohud (Arch)".to_string()),
            recommended: true,
        },
        CommunityRecommendation {
            parameter: "PROTON_LOCAL_SHADER_CACHE=1".to_string(),
            description: "Crea una caché de shaders específica para cada juego".to_string(),
            optimized_for: vec!["all".to_string()],
            notes: Some("En lugar de compartir la caché de shaders entre juegos, crea una caché local para cada título. Reduce el stuttering al cargar zonas nuevas o después de actualizar drivers. Los shaders se compilan una vez y se reutilizan. Muy útil en juegos de mundo abierto".to_string()),
            recommended: true,
        },
        CommunityRecommendation {
            parameter: "WINE_FULLSCREEN_FSR=1".to_string(),
            description: "Activa AMD FidelityFX Super Resolution para upscaling en pantalla completa".to_string(),
            optimized_for: vec!["all".to_string()],
            notes: Some("Permite renderizar el juego a menor resolución y escalarlo con FSR, dando más FPS con buena calidad visual. Funciona en todas las GPUs (AMD, NVIDIA, Intel). Ajusta la calidad con WINE_FULLSCREEN_FSR_STRENGTH=0-3 (0=más blur, 3=más nítido). Ideal para GPUs de gama baja/media".to_string()),
            recommended: false,
        },
    ]
}

pub fn filter_recommendations_by_hardware(
    recommendations: Vec<CommunityRecommendation>,
    hw_tags: &[String],
) -> Vec<CommunityRecommendation> {
    let mut filtered: Vec<CommunityRecommendation> = recommendations
        .into_iter()
        .filter(|rec| {
            rec.optimized_for.contains(&"all".to_string()) ||
            rec.optimized_for.iter().any(|opt| hw_tags.contains(opt))
        })
        .collect();
    
    filtered.sort_by(|a, b| {
        let a_specific = !a.optimized_for.contains(&"all".to_string());
        let b_specific = !b.optimized_for.contains(&"all".to_string());
        
        match (a_specific, b_specific) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    });
    
    filtered
}
