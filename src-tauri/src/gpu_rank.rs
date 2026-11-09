//! Comparación de GPUs por rendimiento relativo.
//!
//! Comparar GPUs por el nombre **no funciona**: `RTX 3050` es peor que
//! `RTX 2060` aunque su número sea mayor, y `RX 580` es peor que `RX 6600`
//! aunque su "tier" parezca superior. Una regla numérica daría veredictos
//! falsos, y un falso "cumples" cuesta una compra.
//!
//! Por eso se usa una tabla curada con las GPUs que Steam menciona en los
//! requisitos, con un score de rendimiento relativo (raster, 1080p). La escala
//! es arbitraria pero consistente: **GTX 1060 = 100**.
//!
//! Reglas de honestidad:
//!   - Si tu GPU no está en la tabla → `None`, y la tarjeta sigue diciendo
//!     "compáralo a mano". Nunca se extrapola.
//!   - Intel Arc no está a propósito: sus modelos no se pueden situar con
//!     fiabilidad frente a NVIDIA/AMD con esta escala, así que quedan en
//!     `None` (opción elegida por el usuario).
//!   - Solo se comparan modelos **reconocidos**; si el texto de Steam no trae
//!     ninguna GPU de la tabla, no hay veredicto.

/// Una GPU con rendimiento relativo. `label` es el nombre para mostrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gpu {
    pub label: &'static str,
    pub score: u16,
}

/// ¿El modelo tiene hardware de ray tracing?
///
/// Se decide por **familia**, no por número: dentro de la misma generación el sufijo
/// TI/SXT no cambia si hay RT, y una RTX 2050 sigue siendo RTX. Lo que sí importa es
/// la arquitectura, que es lo que marca `nivel_rt`.
///
/// El "RT de verdad" se reserva para las generaciones donde activarlo a 1080p rinde:
/// ahí sí se puede recomendar `VKD3D_CONFIG=dxr`. Las tarjetas con RT de primera
/// generación (RTX 20, RX 6000, Arc) entran en `Rt::Justo` y **no** se recomienda
/// activarlo: el flag existe y compila, pero los FPS caen, que es justo lo contrario
/// de lo que busca la app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rt {
    /// Sin unidades de ray tracing.
    No,
    /// Tiene RT, pero activarlo en el juego cuesta FPS en la práctica.
    Justo,
    /// RT utilizable (RTX 30/40, RX 7000+).
    Si,
}

impl Rt {
    /// Igual que `Self: PartialEq`, pero el nombre se lee mejor en el sitio de llamada.
    pub fn es_suficiente(self) -> bool {
        matches!(self, Rt::Si)
    }
}

/// Familia de arquitectura de una GPU, que es lo que determina si el RT compensa.
///
/// La clasificación se hace sobre una lista de prefijos ordenada de más específico a
/// menos: `"RTX 20"` no debe capturar `"RTX 2070"` de la familia 30, y una `"GTX"`
/// nunca debe caer en NVIDIA RT por accidente.
pub fn rt_tier(label: &str) -> Rt {
    let l = normalize(label);

    // ---- NVIDIA: Ada (40) y Ampere (30) tienen RT que compensa activar. Turing
    // (20) también lo tiene, pero es el caso "justo": 1080p con RT ahí suele
    // bajar de 60 FPS, así que no se recomienda el flag.
    if l.contains("rtx 50") || l.contains("rtx 40") || l.contains("rtx 30") {
        return Rt::Si;
    }
    if l.contains("rtx 20") {
        return Rt::Justo;
    }
    if l.starts_with("rtx") {
        // Peor que no dar dato que dar uno por el número de serie del modelo.
        return Rt::No;
    }

    // ---- AMD: RDNA 3 y 4 (RX 7000/8000/9000) tienen RT usable.
    // RDNA 2 (RX 6000) tiene las unidades, pero activarlo cuesta FPS.
    for prefijo in ["rx 9", "rx 8", "rx 7"] {
        if l.contains(prefijo) {
            return Rt::Si;
        }
    }
    if l.contains("rx 6") {
        return Rt::Justo;
    }
    // Vega y RX 500/400 no tienen RT.
    Rt::No
}

/// Tabla de rendimiento relativo (raster, 1080p), escala GTX 1060 = 100.
///
/// Los valores no pretenden ser exactos al punto: basta con que el orden sea
/// correcto y las distancias razonables. Los modelos son los que aparecen en
/// los requisitos de Steam, más las GPUs habituales en equipos Linux.
pub const GPUS: &[Gpu] = &[
    // ---- NVIDIA ----
    Gpu { label: "GT 1030", score: 25 },
    Gpu { label: "GTX 1050 Ti", score: 65 },
    Gpu { label: "GTX 1050", score: 55 },
    Gpu { label: "GTX 1650 SUPER", score: 115 },
    Gpu { label: "GTX 1650", score: 85 },
    Gpu { label: "GTX 970", score: 88 },
    Gpu { label: "GTX 980 Ti", score: 140 },
    Gpu { label: "GTX 980", score: 110 },
    Gpu { label: "GTX 1060 3GB", score: 88 },
    Gpu { label: "GTX 1060 6GB", score: 100 },
    Gpu { label: "GTX 1060", score: 100 },
    Gpu { label: "GTX 1660 SUPER", score: 130 },
    Gpu { label: "GTX 1660 Ti", score: 132 },
    Gpu { label: "GTX 1660", score: 118 },
    Gpu { label: "GTX 1070 Ti", score: 152 },
    Gpu { label: "GTX 1070", score: 135 },
    Gpu { label: "GTX 1080 Ti", score: 205 },
    Gpu { label: "GTX 1080", score: 168 },
    Gpu { label: "RTX 2060 SUPER", score: 175 },
    Gpu { label: "RTX 2060", score: 160 },
    Gpu { label: "RTX 2070 SUPER", score: 205 },
    Gpu { label: "RTX 2070", score: 185 },
    Gpu { label: "RTX 2080 SUPER", score: 232 },
    Gpu { label: "RTX 2080 Ti", score: 265 },
    Gpu { label: "RTX 2080", score: 218 },
    Gpu { label: "RTX 3050", score: 130 },
    Gpu { label: "RTX 3060 Ti", score: 218 },
    Gpu { label: "RTX 3060", score: 188 },
    Gpu { label: "RTX 3070 Ti", score: 258 },
    Gpu { label: "RTX 3070", score: 248 },
    Gpu { label: "RTX 3080 Ti", score: 390 },
    Gpu { label: "RTX 3080", score: 360 },
    Gpu { label: "RTX 3090", score: 395 },
    Gpu { label: "RTX 4060 Ti", score: 245 },
    Gpu { label: "RTX 4060", score: 220 },
    Gpu { label: "RTX 4070 SUPER", score: 320 },
    Gpu { label: "RTX 4070 Ti", score: 335 },
    Gpu { label: "RTX 4070", score: 295 },
    Gpu { label: "RTX 4080", score: 405 },
    Gpu { label: "RTX 4090", score: 500 },
    // ---- AMD ----
    Gpu { label: "RX Vega 56", score: 140 },
    Gpu { label: "RX Vega 64", score: 155 },
    Gpu { label: "RX 550", score: 60 },
    Gpu { label: "RX 560", score: 65 },
    Gpu { label: "RX 570", score: 92 },
    Gpu { label: "RX 580", score: 105 },
    Gpu { label: "RX 590", score: 112 },
    Gpu { label: "RX 5500 XT", score: 115 },
    Gpu { label: "RX 5600 XT", score: 145 },
    Gpu { label: "RX 5700 XT", score: 165 },
    Gpu { label: "RX 5700", score: 155 },
    Gpu { label: "RX 6600 XT", score: 195 },
    Gpu { label: "RX 6600", score: 175 },
    Gpu { label: "RX 6650 XT", score: 205 },
    Gpu { label: "RX 6700 XT", score: 235 },
    Gpu { label: "RX 6750 XT", score: 245 },
    Gpu { label: "RX 6800 XT", score: 330 },
    Gpu { label: "RX 6800", score: 300 },
    Gpu { label: "RX 6900 XT", score: 355 },
    Gpu { label: "RX 6950 XT", score: 365 },
    Gpu { label: "RX 7600", score: 210 },
    Gpu { label: "RX 7700 XT", score: 275 },
    Gpu { label: "RX 7800 XT", score: 310 },
    Gpu { label: "RX 7900 GRE", score: 350 },
    Gpu { label: "RX 7900 XTX", score: 430 },
    Gpu { label: "RX 7900 XT", score: 390 },
];

/// Minúsculas y espacios colapsados. `"AMD Radeon RX 6800 XT (RADV NAVI21)"`
/// pasa a `"amd radeon rx 6800 xt (radv navi21)"`.
fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Busca en `text` el modelo **más específico** de la tabla.
///
/// Se queda con el nombre más largo que aparezca: con `"GTX 1660 SUPER"` no
/// debe ganar `"GTX 1660"`, y con `"RX 6800 XT"` no debe ganar `"RX 6800"`.
fn find_in(text: &str) -> Option<&'static Gpu> {
    let hay = normalize(text);
    // Sin este guard, `"".contains("")` es `true` y un nombre de GPU vacío
    // (iGPU sin reportar, sysinfo sin datos) devolvería una GPU cualquiera de la
    // tabla, dando un veredicto falso.
    if hay.is_empty() {
        return None;
    }
    GPUS.iter()
        .filter(|g| hay.contains(&normalize(g.label)))
        .max_by_key(|g| g.label.len())
}

/// GPU del equipo, si está en la tabla.
pub fn find(name: &str) -> Option<&'static Gpu> {
    find_in(name)
}

/// Nivel de ray tracing de la GPU del equipo, si el modelo está en la tabla.
///
/// Se apoya en `find` a propósito: el RT se decide sobre el nombre **canónico** del
/// modelo (`RTX 4070`, `RX 7800 XT`), no sobre el texto crudo de `lspci`, que trae
/// ruido del driver (`RADV NAVI21`, `radeonsi, navi32, LLVM 17.0.6`) y podría casar
/// con un prefijo de familia que no es el suyo.
///
/// `None` = el modelo no está en la tabla o no se supo: sin dato no se recomienda RT.
pub fn rt_for_gpu(name: &str) -> Option<Rt> {
    find(name).map(|gpu| rt_tier(gpu.label))
}

/// La GPU **más débil** que menciona el texto de Steam.
///
/// Steam lista alternativas unidas por comas o barras, y basta con llegar a una
/// de ellas: se compara contra la más baja (la más fácil de superar).
pub fn weakest(text: &str) -> Option<&'static Gpu> {
    text.split([',', '/', ';', '|'])
        .filter_map(find_in)
        .min_by_key(|g| g.score)
}

/// Resultado de comparar la GPU del equipo con la que pide el juego.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// Modelo que pide Steam (el más débil de la lista).
    pub required: &'static Gpu,
    /// Modelo del equipo.
    pub actual: &'static Gpu,
    /// `actual.score / required.score`. `1.0` = mismo rendimiento.
    pub ratio: f32,
}

/// Compara la GPU del equipo con el requisito de Steam.
///
/// Devuelve `None` si el texto no menciona ninguna GPU de la tabla o si la GPU
/// del equipo no está en ella. **Nunca** inventa: sin dato, no hay veredicto.
pub fn compare(required_text: &str, actual_name: &str) -> Option<Comparison> {
    let required = weakest(required_text)?;
    let actual = find(actual_name)?;
    Some(Comparison {
        required,
        actual,
        ratio: actual.score as f32 / required.score as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_modelos_con_el_nombre_de_linux() {
        // Lo que devuelve `lspci`/sysinfo en Linux, con sufijos del driver.
        assert_eq!(find("AMD Radeon RX 6800 XT (RADV NAVI21)").map(|g| g.label), Some("RX 6800 XT"));
        assert_eq!(find("NVIDIA GeForce RTX 2060").map(|g| g.label), Some("RTX 2060"));
        assert_eq!(find("Advanced Micro Devices, Inc. [AMD/ATI] Navi 23 [Radeon RX 6600]").map(|g| g.label), Some("RX 6600"));
    }

    #[test]
    fn el_modelo_mas_especifico_gana() {
        // "GTX 1660 SUPER" no debe degradarse a "GTX 1660".
        let super_ = find("GTX 1660 SUPER").unwrap();
        let base = find("GTX 1660").unwrap();
        assert!(super_.score > base.score, "SUPER debe puntuar mas que la base");
        // "RX 6800 XT" no debe degradarse a "RX 6800".
        assert_eq!(find("RX 6800 XT").map(|g| g.label), Some("RX 6800 XT"));
        assert_eq!(find("RX 6800").map(|g| g.label), Some("RX 6800"));
    }

    #[test]
    fn nombre_vacio_no_devuelve_una_gpu_cualquiera() {
        // "".contains("") es true: sin guard, una iGPU sin reportar daria un
        // veredicto con la GPU mas larga de la tabla.
        assert!(find("").is_none());
        assert!(find("   ").is_none());
        assert!(compare("RTX 2060", "").is_none());
    }

    #[test]
    fn intel_arc_no_se_compara() {
        // Decisión de producto: sin datos fiables, mejor sin veredicto.
        assert!(find("Intel Arc A380").is_none());
        assert!(find("Intel Iris Xe Graphics").is_none());
    }

    #[test]
    fn gpu_desconocida_no_inventa_veredicto() {
        assert!(find("NVIDIA GeForce GTX 555").is_none());
        assert!(compare("RTX 2060", "alguna gpu rara").is_none());
    }

    #[test]
    fn elige_la_gpu_mas_debil_de_la_lista() {
        // Steam lista alternativas: basta llegar a la mas baja.
        let text = "Nvidia RTX 2060, AMD Radeon RX 5600 XT 6GB, Intel Arc A380";
        assert_eq!(weakest(text).map(|g| g.label), Some("RX 5600 XT"));
    }

    #[test]
    fn no_confunde_ti_con_base() {
        assert_eq!(weakest("GTX 1080 Ti").map(|g| g.label), Some("GTX 1080 Ti"));
        assert_eq!(weakest("GTX 1070 Ti").map(|g| g.label), Some("GTX 1070 Ti"));
    }

    #[test]
    fn la_6800_xt_supera_el_requisito_del_ejemplo_real() {
        let text = "Nvidia RTX 2060, AMD Radeon RX 5600 XT 6GB, Intel Arc A380";
        let cmp = compare(text, "AMD Radeon RX 6800 XT (RADV NAVI21)").unwrap();
        assert_eq!(cmp.required.label, "RX 5600 XT");
        assert_eq!(cmp.actual.label, "RX 6800 XT");
        assert!(cmp.ratio > 2.0, "la 6800 XT dobla a la 5600 XT, ratio = {}", cmp.ratio);
    }

    #[test]
    fn una_gpu_peor_da_ratio_menor_que_uno() {
        let cmp = compare("RTX 3060", "GTX 1060").unwrap();
        assert!(cmp.ratio < 1.0);
    }

    #[test]
    fn la_tabla_no_tiene_duplicados() {
        let mut labels: Vec<&str> = GPUS.iter().map(|g| g.label).collect();
        labels.sort_unstable();
        let antes = labels.len();
        labels.dedup();
        assert_eq!(antes, labels.len(), "hay modelos repetidos en la tabla");
    }

    #[test]
    fn ningun_score_es_cero() {
        // Un cero dividiria por cero en `compare`.
        assert!(GPUS.iter().all(|g| g.score > 0));
    }

    // ==================== RAY TRACING ====================
    // El bug que cierra esto: la app recomendaba `VKD3D_CONFIG=dxr` a TODO el
    // mundo. En una GPU sin RT el flag activa ray tracing por software (o
    // directamente no hace nada) y en las de RT de primera generacion baja los
    // FPS. Justo lo contrario de lo que busca la app.

    #[test]
    fn las_rtx_de_ampere_y_ada_tienen_rt_utilizable() {
        for label in [
            "RTX 4090",
            "RTX 4060 Ti",
            "RTX 3080",
            "RTX 3050",
            "NVIDIA GeForce RTX 4060",
        ] {
            assert_eq!(rt_tier(label), Rt::Si, "{label} deberia poder usar RT");
        }
    }

    #[test]
    fn turing_tiene_rt_pero_no_compensa() {
        // RTX 20 si tiene unidades RT, pero activarlo cuesta FPS: el flag no se
        // recomienda. Se distingue de "no tiene RT" a proposito.
        for label in ["RTX 2060", "RTX 2080 Ti", "RTX 2070 SUPER"] {
            assert_eq!(rt_tier(label), Rt::Justo, "{label} es RT de primera generacion");
        }
        assert!(!Rt::Justo.es_suficiente());
    }

    #[test]
    fn las_gtx_no_tienen_ray_tracing() {
        // Ademas de no tener unidades RT, es el error clasico al buscar por
        // subcadena: "GTX 1080 Ti" no debe colarse como RTX.
        for label in ["GTX 1080 Ti", "GTX 1060 6GB", "GT 1030", "GTX 1660 SUPER"] {
            assert_eq!(rt_tier(label), Rt::No, "{label} no tiene RT");
        }
    }

    #[test]
    fn rdna_3_y_4_si_y_rdna_2_solo_justo() {
        for label in ["RX 7900 XTX", "RX 7800 XT", "RX 7600", "RX 9070 XT", "RX 8800"] {
            assert_eq!(rt_tier(label), Rt::Si, "{label} tiene RT utilizable");
        }
        for label in ["RX 6800 XT", "RX 6600", "RX 6950 XT"] {
            assert_eq!(rt_tier(label), Rt::Justo, "{label} es RDNA 2");
        }
    }

    #[test]
    fn una_rx_5500_no_se_confunde_con_rdna3() {
        // "RX 5500 XT" es RDNA 1: sin RT. Con un prefijo de un digito se colaria
        // como si tuviera unidades de ray tracing.
        for label in ["RX 5500 XT", "RX 5600 XT", "RX 580", "RX Vega 64", "RX 570"] {
            assert_eq!(rt_tier(label), Rt::No, "{label} no tiene RT");
        }
    }

    #[test]
    fn rt_for_gpu_usa_la_tabla_y_no_inventa() {
        assert_eq!(rt_for_gpu("AMD Radeon RX 6800 XT (RADV NAVI21)"), Some(Rt::Justo));
        assert_eq!(rt_for_gpu("NVIDIA GeForce RTX 3070"), Some(Rt::Si));
        assert_eq!(rt_for_gpu("NVIDIA GeForce GTX 1660 Ti"), Some(Rt::No));
        // Una GPU fuera de la tabla no recibe veredicto.
        assert_eq!(rt_for_gpu("Intel Arc A380"), None);
        assert_eq!(rt_for_gpu(""), None);
    }

    #[test]
    fn una_etiqueta_vacia_no_da_rt() {
        // `"".contains("rtx")` es false, pero conviene fijarlo: sin GPU no hay RT.
        assert_eq!(rt_tier(""), Rt::No);
        assert_eq!(rt_tier("   "), Rt::No);
    }
}
