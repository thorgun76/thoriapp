use serde::{Deserialize, Serialize};

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

/// Firma de un motor en disco: nombres/rutas que delatan su presencia.
///
/// La deteccion se basa en ficheros que existen de verdad en la instalacion,
/// no en una tabla `appid -> motor` de memoria. La tabla anterior decia, por
/// ejemplo, que Elden Ring era Unreal Engine cuando usa el motor propio de
/// FromSoftware sobre DirectX 12, y asignaba a Stardew Valley Unity y Godot a
/// la vez. Adivinar la API grafica con datos falsos contamina el margen de
/// Proton que consume la tarjeta de requisitos.
pub struct EngineSignature {
    pub name: &'static str,
    /// Todas las agujas deben aparecer entre los nombres de la instalacion.
    pub required: &'static [&'static str],
    /// Basta con que aparezca una.
    pub any_of: &'static [&'static str],
    pub api: &'static str,
}

/// Firmas en orden de prioridad: la primera que encaje gana.
///
/// Los nombres van en minusculas porque Linux distingue mayusculas y hay juegos
/// que instalan `UnityPlayer.dll` y otros `unityplayer.dll`.
pub const SIGNATURES: &[EngineSignature] = &[
    // Unity: siempre deja `UnityPlayer.dll`. El nombre de la carpeta de datos es
    // el del ejecutable, no un literal "unity", asi que no sirve como aguja.
    EngineSignature {
        name: "Unity",
        required: &["unityplayer.dll"],
        any_of: &[],
        api: "dxvk",
    },
    // Unreal 4/5: el ejecutable vive en `Binaries/Win64`.
    EngineSignature {
        name: "Unreal Engine",
        required: &[],
        any_of: &["binaries/win64", "engine/binaries", "ue4game.exe", "ue5game.exe"],
        api: "dxvk",
    },
    // Godot: el export deja el `.pck` con el nombre del juego, pero un `.pck`
    // suelto es ambiguo (otros motores lo usan). Solo se detecta cuando ademas
    // aparece el binario exportado de Godot, que si es inequivoco.
    EngineSignature {
        name: "Godot",
        required: &[],
        any_of: &["godot_console.exe", "godot_engine", ".godot/", "godot.windows"],
        api: "vulkan",
    },
    // Source / Source 2: `bin/engine.dll` + algun contenido reconocible.
    EngineSignature {
        name: "Source",
        required: &["engine.dll"],
        any_of: &["platform/", "hl2", "csgo", "gameinfo.txt"],
        api: "vulkan",
    },
    // FromSoftware: Elden Ring, Dark Souls 3 y Sekiro son DirectX 12 (VKD3D),
    // no DXVK como decia la tabla anterior.
    EngineSignature {
        name: "FromSoftware",
        required: &[],
        any_of: &["nightreign.exe", "eldenring.exe", "darksoulsiii.exe", "sekiro.exe"],
        api: "vkd3d",
    },
    // REDengine 4 (Cyberpunk 2077).
    EngineSignature {
        name: "REDengine",
        required: &[],
        any_of: &["cyberpunk2077.exe", "redprelauncher.exe"],
        api: "vkd3d",
    },
    // RPG Maker.
    EngineSignature {
        name: "RPG Maker",
        required: &[],
        any_of: &["game.rgss3a", "game.rgssad", "rgss300.dll", "nw.dll"],
        api: "dxvk",
    },
];

/// Normaliza un nombre de fichero o ruta relativa para compararlo.
pub fn normalize_entry(entry: &str) -> String {
    entry.to_lowercase().replace('\\', "/").to_string()
}

/// Deteccion pura: recibe los nombres de la instalacion y devuelve el motor.
///
/// Se separa de la lectura de disco para poder testearla con listas fijas.
pub fn detect_from_entries<I, S>(entries: I) -> Option<GameEngine>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let files: Vec<String> = entries
        .into_iter()
        .map(|e| normalize_entry(e.as_ref()))
        .collect();

    for sig in SIGNATURES {
        let required_ok = sig
            .required
            .iter()
            .all(|needle| files.iter().any(|f| f.contains(needle)));
        if !required_ok {
            continue;
        }
        let any_ok = sig.any_of.is_empty()
            || sig
                .any_of
                .iter()
                .any(|needle| files.iter().any(|f| f.contains(needle)));
        if any_ok {
            return Some(GameEngine {
                name: sig.name.to_string(),
                version: None,
                api: sig.api.to_string(),
            });
        }
    }
    None
}

/// Recorre la instalacion (hasta 4 niveles) y aplica las firmas.
///
/// `None` si la ruta no existe, no se puede leer o no encaja ninguna firma. La
/// UI debe mostrar "sin dato", no asumir un motor.
pub fn detect_from_install_path(install_path: &str) -> Option<GameEngine> {
    use std::path::Path;

    let root = Path::new(install_path);
    if !root.is_dir() {
        return None;
    }
    let mut entries: Vec<String> = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0u8)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > 4 || entries.len() > 20_000 {
            continue;
        }
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(root) else { continue };
            let rel_str = rel.to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            entries.push(rel_str);
            if is_dir {
                stack.push((path, depth + 1));
            }
        }
    }
    detect_from_entries(entries)
}

/// Deteccion por `appid` + nombre.
///
/// Ya **no** usa una tabla de `appid -> motor`: sin ruta de instalacion solo se
/// puede mirar el nombre, y eso casi nunca funciona (un juego de Unity no se
/// llama "Unity"). Devuelve `None` en vez de inventar, para que el margen caiga
/// al 10 % por defecto de forma explicita.
pub fn detect_engine(_appid: u32, game_name: &str) -> Option<GameEngine> {
    detect_from_entries([game_name.to_string()])
}

/// Recomendaciones propias del motor, o `None` si el motor no esta reconocido.
///
/// Devolver `Option` en vez de `Vec` es deliberado: asi "motor desconocido" y
/// "motor conocido sin parametros utiles" dejan de ser indistinguibles. El bug
/// de `"REDengine 4"` (la deteccion emitia `"REDengine"` y el match no lo
/// cubria) era invisible porque ambos casos daban lista vacia.
fn engine_params(name: &str) -> Option<Vec<String>> {
    let params = match name {
        "Unity" => vec![
            "PROTON_FORCE_LARGE_ADDRESS_AWARE=1".to_string(),
            "PROTON_USE_NTSYNC=1".to_string(),
        ],
        // `DXVK_ASYNC=1` fue retirado de DXVK hace anos y hoy no hace nada: se
        // elimina en vez de recomendar un parametro muerto.
        "Unreal Engine" => vec!["PROTON_USE_NTSYNC=1".to_string()],
        "Source" | "Source 2" => vec!["-vulkan".to_string(), "-novid".to_string()],
        // El nombre que emite la deteccion por disco es `REDengine`.
        //
        // No se pone aqui `VKD3D_CONFIG=dxr` (lo que hacia antes): activa el ray
        // tracing y baja los FPS en GPUs sin RT. Lo anade
        // `get_engine_recommendations_for` solo si la GPU del equipo lo aguanta.
        "REDengine" | "REDengine 4" => vec!["PROTON_ENABLE_NVAPI=1".to_string()],
        "Godot" => vec!["PROTON_USE_NTSYNC=1".to_string()],
        "FromSoftware" | "FromSoftware Engine" => vec!["PROTON_USE_NTSYNC=1".to_string()],
        // RPG Maker: sin parametro fiable. NW.js y RGSS no leen flags de Proton
        // utiles; se devuelve vacio a proposito en vez de inventar uno.
        "RPG Maker" => vec![],
        _ => return None,
    };
    Some(params)
}

/// `true` si el motor tiene rama propia (aunque no aporte parametros).
pub fn engine_es_conocido(name: &str) -> bool {
    engine_params(name).is_some()
}

pub fn get_engine_recommendations(engine: &GameEngine) -> Vec<String> {
    // Una deteccion que emite un nombre sin rama se queda en `_ => return None` y el
    // motor se descarta **en silencio**: justo el bug de "REDengine 4", que estuvo
    // devolviendo cero parametros sin que nadie se enterase. Avisar por stderr (solo
    // en debug) lo deja ver sin tumbar la app: perder los parametros de un motor es
    // molesto, pero no es motivo para cerrar la ventana del usuario.
    #[cfg(debug_assertions)]
    if !engine_es_conocido(&engine.name) {
        eprintln!(
            "[engine] el motor '{}' se detecta pero no tiene rama en engine_params: sus parametros se pierden en silencio",
            engine.name
        );
    }
    engine_params(&engine.name).unwrap_or_default()
}

/// Como `get_engine_recommendations`, pero ajustando el ray tracing a esta máquina.
///
/// El caso concreto que arregla esto: `VKD3D_CONFIG=dxr` activa el ray tracing de
/// VKD3D-Proton. En Cyberpunk 2077 eso baja los FPS en cualquier GPU que no tenga RT
/// de sobra, así que recomendarlo siempre era lo contrario de lo que busca la app.
///
/// Por eso el flag **no está** en `engine_params` y se decide aquí en dos pasos:
///
///   1. Se quita siempre de lo que devuelve el motor, por si vuelve a colarse en la
///      fuente de parámetros por motor.
///   2. Se **añade** solo si el motor es VKD3D, la GPU está en la tabla y su familia
///      tiene RT utilizable (`Rt::Si`).
///
/// Con GPU sin RT, RT de primera generación o modelo que no está en la tabla, el flag
/// no se recomienda: es una opción que cuesta FPS y no hay dato que la respalde.
pub fn get_engine_recommendations_for(
    engine: &GameEngine,
    gpu_name: Option<&str>,
) -> Vec<String> {
    let rt_suficiente = gpu_name
        .and_then(crate::gpu_rank::rt_for_gpu)
        .map(|rt| rt.es_suficiente())
        .unwrap_or(false);

    let mut recs: Vec<String> = get_engine_recommendations(engine)
        .into_iter()
        .filter(|p| !activa_ray_tracing(p))
        .collect();

    // Solo tiene sentido en motores que compilan D3D12 sobre VKD3D: en un juego
    // Vulkan nativo (RDR2) o sobre DXVK, `VKD3D_CONFIG` no lo lee nadie y el
    // parámetro quedaría en el comando como ruido.
    if rt_suficiente && engine.api == "vkd3d" {
        recs.push("VKD3D_CONFIG=dxr".to_string());
    }
    recs
}

/// `VKD3D_CONFIG` es una lista separada por comas (`dxr,nofp16`): se mira el token
/// exacto y no la subcadena, para no confundir `dxr` con una futura opción que lo
/// contenga.
fn activa_ray_tracing(param: &str) -> bool {
    let Some((clave, valor)) = param.split_once('=') else {
        return false;
    };
    clave.eq_ignore_ascii_case("VKD3D_CONFIG")
        && valor.split(',').any(|v| v.trim().eq_ignore_ascii_case("dxr"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_unity_por_el_player() {
        let e = detect_from_entries([
            "Game.exe",
            "UnityPlayer.dll",
            "Game_Data/Managed/Assembly-CSharp.dll",
        ])
        .expect("deberia detectar Unity");
        assert_eq!(e.name, "Unity");
        assert_eq!(e.api, "dxvk");
    }

    #[test]
    fn unity_va_en_minusculas_y_mayusculas() {
        // Linux distingue mayusculas: hay juegos con `unityplayer.dll`.
        let e = detect_from_entries(["game.exe", "unityplayer.dll"]).unwrap();
        assert_eq!(e.name, "Unity");
    }

    #[test]
    fn detecta_unreal_por_binaries_win64() {
        let e = detect_from_entries([
            "Engine/Content/Paks/global.utoc",
            "GameName/Binaries/Win64/GameName-Win64-Shipping.exe",
        ])
        .unwrap();
        assert_eq!(e.name, "Unreal Engine");
        assert_eq!(e.api, "dxvk");
    }

    #[test]
    fn detecta_godot_solo_con_marcador_inequivoco() {
        // Un `.pck` suelto no basta: el nombre lo pone el juego y otros motores
        // tambien lo usan.
        assert!(detect_from_entries(["Game.exe", "game.pck"]).is_none());
        let e = detect_from_entries(["Game.exe", "Game.pck", "godot_console.exe"]).unwrap();
        assert_eq!(e.name, "Godot");
        assert_eq!(e.api, "vulkan");
    }

    #[test]
    fn detecta_elden_ring_como_fromsoftware_y_vkd3d() {
        // La tabla anterior decia "Unreal Engine" + "dxvk". Elden Ring usa el
        // motor propio sobre DirectX 12: el margen correcto es VKD3D (20 %).
        let e = detect_from_entries(["Game/eldenring.exe", "Game/Data1.bhd"]).unwrap();
        assert_eq!(e.name, "FromSoftware");
        assert_eq!(e.api, "vkd3d");
    }

    #[test]
    fn detecta_cyberpunk_como_vkd3d() {
        let e = detect_from_entries(["Cyberpunk2077.exe", "bin/x64/Cyberpunk2077.exe"]).unwrap();
        assert_eq!(e.name, "REDengine");
        assert_eq!(e.api, "vkd3d");
    }

    #[test]
    fn detecta_source_por_engine_dll() {
        let e = detect_from_entries(["bin/engine.dll", "platform/", "hl2/hl2_misc.vpk"]).unwrap();
        assert_eq!(e.name, "Source");
        assert_eq!(e.api, "vulkan");
    }

    #[test]
    fn engine_dll_solo_no_basta_para_source() {
        // Un juego cualquiera puede tener un `engine.dll` suelto; sin contenido
        // de Source reconocible no se etiqueta.
        assert!(detect_from_entries(["bin/engine.dll"]).is_none());
    }

    #[test]
    fn sin_firmas_no_inventa_motor() {
        assert!(detect_from_entries(["Game.exe", "readme.txt", "data/"]).is_none());
        assert!(detect_from_entries(Vec::<String>::new()).is_none());
    }

    #[test]
    fn unity_gana_a_unreal_si_aparecen_las_dos() {
        // Orden de prioridad fijo: Unity se comprueba antes.
        let e = detect_from_entries(["UnityPlayer.dll", "Game/Binaries/Win64/x.exe"]).unwrap();
        assert_eq!(e.name, "Unity");
    }

    #[test]
    fn detect_engine_por_nombre_solo_ve_lo_obvio() {
        // "ELDEN RING" no contiene la aguja `eldenring.exe`: sin ruta el nombre
        // no basta, y es correcto devolver None en vez de adivinar.
        assert!(detect_engine(0, "ELDEN RING").is_none());
        // Un juego de Unity no se llama "Unity": sin ruta no hay deteccion.
        assert!(detect_engine(730, "Counter-Strike 2").is_none());
        // Si el nombre trae la firma literal, si se detecta.
        assert!(detect_engine(0, "eldenring.exe").is_some());
    }

    #[test]
    fn ruta_inexistente_no_paniquea() {
        assert!(detect_from_install_path("/ruta/que/no/existe/12345").is_none());
        assert!(detect_from_install_path("").is_none());
    }

    #[test]
    fn lee_de_verdad_del_disco() {
        // Fixture real: se crea un arbol temporal con la firma de Unity.
        let base = std::env::temp_dir().join("thoriapp_engine_fixture");
        let _ = std::fs::remove_dir_all(&base);
        let data = base.join("Game_Data");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(base.join("UnityPlayer.dll"), b"x").unwrap();
        std::fs::write(base.join("Game.exe"), b"x").unwrap();

        let e = detect_from_install_path(&base.to_string_lossy()).expect("deberia leer el disco");
        assert_eq!(e.name, "Unity");

        // Y una carpeta sin firmas devuelve None, no un motor por defecto.
        let vacia = base.join("vacia");
        std::fs::create_dir_all(&vacia).unwrap();
        assert!(detect_from_install_path(&vacia.to_string_lossy()).is_none());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ningun_motor_cae_en_el_brazo_comodin() {
        // El bug de "REDengine 4": la deteccion emitia un nombre que el match no
        // cubria, y el motor se quedaba en el `_ => vec![]` en silencio.
        //
        // No se exige que haya recomendaciones: RPG Maker devuelve vacio a
        // proposito. Lo que se exige es que el nombre este **reconocido**, y eso
        // lo dice `engine_es_conocido`, que es la unica fuente de verdad.
        for sig in SIGNATURES {
            assert!(
                engine_es_conocido(sig.name),
                "el motor {} no tiene rama propia en get_engine_recommendations",
                sig.name
            );
        }
    }

    #[test]
    fn no_recomienda_dxvk_async_que_ya_no_existe() {
        let e = GameEngine {
            name: "Unreal Engine".into(),
            version: None,
            api: "dxvk".into(),
        };
        assert!(!get_engine_recommendations(&e).iter().any(|r| r.contains("DXVK_ASYNC")));
    }

    #[test]
    fn normaliza_barras_de_windows() {
        assert_eq!(normalize_entry("Game\\Binaries\\Win64"), "game/binaries/win64");
        assert_eq!(normalize_entry("./Game.exe"), "./game.exe");
    }

    // ==================== VKD3D_CONFIG=dxr SOLO CON RT ====================

    fn redengine() -> GameEngine {
        GameEngine {
            name: "REDengine".into(),
            version: None,
            api: "vkd3d".into(),
        }
    }

    #[test]
    fn el_base_del_motor_no_lleva_dxr_a_secas() {
        // La fuente de parametros por motor no debe volver a colar el flag: el
        // filtro por GPU es el unico que puede anadirlo.
        assert!(!get_engine_recommendations(&redengine())
            .iter()
            .any(|p| activa_ray_tracing(p)));
    }

    #[test]
    fn con_rt_utilizable_se_recomienda_dxr() {
        let recs = get_engine_recommendations_for(&redengine(), Some("NVIDIA GeForce RTX 3070"));
        assert!(recs.iter().any(|p| p == "VKD3D_CONFIG=dxr"), "{recs:?}");
        // Y el resto de parametros del motor sigue ahi.
        assert!(recs.iter().any(|p| p == "PROTON_ENABLE_NVAPI=1"));
    }

    #[test]
    fn el_flag_se_decide_por_api_y_no_por_nombre_de_motor() {
        // FromSoftware tambien compila D3D12 sobre VKD3D: si la GPU aguanta el RT,
        // el parametro es igual de valido que en Cyberpunk.
        let fromsoft = GameEngine {
            name: "FromSoftware".into(),
            version: None,
            api: "vkd3d".into(),
        };
        let recs = get_engine_recommendations_for(&fromsoft, Some("AMD Radeon RX 7800 XT"));
        assert!(recs.iter().any(|p| p == "VKD3D_CONFIG=dxr"), "{recs:?}");
        // Pero no en una 6600, que es RDNA 2: RT justo, cuesta FPS.
        let recs = get_engine_recommendations_for(
            &fromsoft,
            Some("AMD Radeon RX 6600 (RADV NAVI23)"),
        );
        assert!(!recs.iter().any(|p| activa_ray_tracing(p)), "{recs:?}");
        // Y con DXVK tampoco, aunque la GPU sea tope de gama.
        let unity = GameEngine {
            name: "Unity".into(),
            version: None,
            api: "dxvk".into(),
        };
        let recs = get_engine_recommendations_for(&unity, Some("NVIDIA GeForce RTX 4090"));
        assert!(!recs.iter().any(|p| activa_ray_tracing(p)), "{recs:?}");
    }

    #[test]
    fn el_flag_no_se_repite_ni_se_suma_a_lo_que_ya_estaba() {
        // Si algun dia la fuente por motor vuelve a traer el flag, no debe salir dos
        // veces en el comando.
        let recs = get_engine_recommendations_for(&redengine(), Some("NVIDIA GeForce RTX 4070"));
        assert_eq!(
            recs.iter().filter(|p| p.as_str() == "VKD3D_CONFIG=dxr").count(),
            1,
            "{recs:?}"
        );
    }

    #[test]
    fn en_un_motor_vulkan_nativo_no_se_toca_vkd3d() {
        // Godot exporta a Vulkan nativo: `VKD3D_CONFIG` no lo lee nadie, ni con una
        // 4080. Se comprueba con un motor que si tiene parametros propios, para
        // distinguir «no se anadio el flag» de «no se devolvio nada».
        let godot = GameEngine {
            name: "Godot".into(),
            version: None,
            api: "vulkan".into(),
        };
        let recs = get_engine_recommendations_for(&godot, Some("NVIDIA GeForce RTX 4080"));
        assert!(!recs.iter().any(|p| activa_ray_tracing(p)), "{recs:?}");
        assert!(recs.iter().any(|p| p == "PROTON_USE_NTSYNC=1"), "{recs:?}");
    }

    #[test]
    fn sin_rt_no_se_recomienda_dxr() {
        // El bug reportado: una GTX 1660 Ti recibia el flag igualmente y perdia FPS.
        let recs = get_engine_recommendations_for(&redengine(), Some("NVIDIA GeForce GTX 1660 Ti"));
        assert!(!recs.iter().any(|p| activa_ray_tracing(p)), "{recs:?}");
        assert!(!recs.is_empty(), "los demas parametros del motor siguen aplicando");
    }

    #[test]
    fn con_rt_de_primera_generacion_tampoco_se_recomienda() {
        for gpu in [
            "AMD Radeon RX 6800 XT (RADV NAVI21)",
            "NVIDIA GeForce RTX 2060",
        ] {
            let recs = get_engine_recommendations_for(&redengine(), Some(gpu));
            assert!(!recs.iter().any(|p| activa_ray_tracing(p)), "{gpu}: {recs:?}");
        }
    }

    #[test]
    fn sin_gpu_conocida_no_se_inventa_el_flag() {
        // El flag puede costar FPS: sin dato que lo respalde, no se recomienda.
        assert!(!get_engine_recommendations_for(&redengine(), None)
            .iter()
            .any(|p| activa_ray_tracing(p)));
        assert!(!get_engine_recommendations_for(&redengine(), Some("Intel Arc A380"))
            .iter()
            .any(|p| activa_ray_tracing(p)));
    }

    #[test]
    fn el_flag_se_reconoce_dentro_de_una_lista() {
        // `VKD3D_CONFIG` es una lista por comas: quitar solo "dxr" sin cargarse
        // el resto de opciones es lo que importa si algun dia se parte.
        assert!(activa_ray_tracing("VKD3D_CONFIG=dxr"));
        assert!(activa_ray_tracing("VKD3D_CONFIG=no_fp16,dxr"));
        assert!(activa_ray_tracing("vkd3d_config=DXR"));
        assert!(!activa_ray_tracing("VKD3D_CONFIG=no_fp16"));
        assert!(!activa_ray_tracing("PROTON_ENABLE_NVAPI=1"));
        assert!(!activa_ray_tracing("dxr"));
    }
}
