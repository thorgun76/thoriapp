//! Comparación de CPUs por rendimiento relativo.
//!
//! Mismo problema que con las GPUs (`gpu_rank`), agravado: los números de Intel no
//! son comparables entre generaciones (`i3-12100` rinde más que un `i7-7700` con el
//! número mucho más alto) y Steam mezcla AMD e Intel en la misma frase
//! (`Ryzen 5 3600 / Intel i5-8400`). Una regla numérica daría veredictos falsos.
//!
//! Por eso, tabla curada con score de rendimiento relativo **por núcleo**, que es lo
//! que decide si un juego limitado por CPU va a 60 o a 30. La escala es arbitraria
//! pero consistente con la de GPUs: **Ryzen 5 3600 = 100**.
//!
//! Lo que esta tabla **no** puede saber:
//!
//!   - **Cuántos hilos tienes**. Un `i5-8400` (6/6) y un `i5-10400` (6/12) tienen casi
//!     el mismo score por núcleo, pero no rinden igual en juegos modernos. Por eso el
//!     veredicto final lo da [`compare`] con el número de hilos del equipo, y sin
//!     hilos (`None`) solo se compara el modelo.
//!   - **iGPU vs CPU**: `i5-1135G7` es un portátil de 15 W. Va en la tabla porque
//!     aparece en requisitos, pero su score es de portátil, no de escritorio.
//!
//! Reglas de honestidad (idénticas a `gpu_rank`):
//!   - CPU desconocida → `None`, y la tarjeta sigue diciendo «compáralo a mano».
//!     Nunca se extrapola del número del modelo.
//!   - Solo cuentan los modelos **reconocidos**; si el texto de Steam no trae ninguna
//!     CPU de la tabla, no hay veredicto.

/// Una CPU con rendimiento relativo por núcleo. `label` es el nombre para mostrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cpu {
    pub label: &'static str,
    pub score: u16,
    /// Núcleos físicos del modelo, cuando se conocen.
    ///
    /// Se usa para detectar el caso «tus núcleos no llegan aunque tu IPC sea mejor»:
    /// un `i3-8100` moderno tiene mejor score que el `i5-2500` que pide el juego, pero
    /// solo 4 núcleos. `None` = no se conoce, y no se usa para nada.
    pub cores: Option<u8>,
}

/// Tabla de rendimiento relativo por núcleo. Escala **Ryzen 5 3600 = 100**.
///
/// Los valores no pretenden ser exactos al punto: basta con el orden y con que las
/// distancias sean razonables. Los modelos son los que aparecen en requisitos de
/// Steam, más las CPUs habituales en equipos Linux.
pub const CPUS: &[Cpu] = &[
    // ---- AMD AM4 / AM5 ----
    Cpu { label: "Ryzen 3 1200", score: 58, cores: Some(4) },
    Cpu { label: "Ryzen 3 1300X", score: 62, cores: Some(4) },
    Cpu { label: "Ryzen 3 2200G", score: 62, cores: Some(4) },
    Cpu { label: "Ryzen 3 3100", score: 85, cores: Some(4) },
    Cpu { label: "Ryzen 3 3200G", score: 70, cores: Some(4) },
    Cpu { label: "Ryzen 5 1600", score: 76, cores: Some(6) },
    Cpu { label: "Ryzen 5 2600", score: 85, cores: Some(6) },
    Cpu { label: "Ryzen 5 3600", score: 100, cores: Some(6) },
    Cpu { label: "Ryzen 5 3600X", score: 104, cores: Some(6) },
    Cpu { label: "Ryzen 5 5500", score: 118, cores: Some(6) },
    Cpu { label: "Ryzen 5 5600", score: 128, cores: Some(6) },
    Cpu { label: "Ryzen 5 5600X", score: 132, cores: Some(6) },
    Cpu { label: "Ryzen 5 7600", score: 158, cores: Some(6) },
    Cpu { label: "Ryzen 5 7600X", score: 163, cores: Some(6) },
    Cpu { label: "Ryzen 7 1700", score: 78, cores: Some(8) },
    Cpu { label: "Ryzen 7 2700", score: 88, cores: Some(8) },
    Cpu { label: "Ryzen 7 3700X", score: 106, cores: Some(8) },
    Cpu { label: "Ryzen 7 5700X", score: 130, cores: Some(8) },
    Cpu { label: "Ryzen 7 5800X", score: 138, cores: Some(8) },
    Cpu { label: "Ryzen 7 7700X", score: 168, cores: Some(8) },
    Cpu { label: "Ryzen 7 7800X3D", score: 195, cores: Some(8) },
    Cpu { label: "Ryzen 9 3900X", score: 108, cores: Some(12) },
    Cpu { label: "Ryzen 9 5900X", score: 140, cores: Some(12) },
    Cpu { label: "Ryzen 9 7950X", score: 172, cores: Some(16) },
    // iGPU de portátil: aparece en requisitos, pero rinde muy por debajo de su número.
    Cpu { label: "Ryzen 5 2500U", score: 52, cores: Some(4) },
    Cpu { label: "Ryzen 7 5800H", score: 122, cores: Some(8) },
    // ---- AMD FX (los que aún aparecen en requisitos viejos) ----
    Cpu { label: "FX-6300", score: 42, cores: Some(6) },
    Cpu { label: "FX-8350", score: 48, cores: Some(8) },
    // ---- Intel Core, de viejo a nuevo ----
    Cpu { label: "i5-2500K", score: 55, cores: Some(4) },
    Cpu { label: "i5-3470", score: 60, cores: Some(4) },
    Cpu { label: "i5-3570K", score: 62, cores: Some(4) },
    Cpu { label: "i5-4460", score: 64, cores: Some(4) },
    Cpu { label: "i5-4590", score: 66, cores: Some(4) },
    Cpu { label: "i5-4690K", score: 70, cores: Some(4) },
    Cpu { label: "i5-6400", score: 70, cores: Some(4) },
    Cpu { label: "i5-6500", score: 74, cores: Some(4) },
    Cpu { label: "i5-6600K", score: 80, cores: Some(4) },
    Cpu { label: "i5-7400", score: 78, cores: Some(4) },
    Cpu { label: "i5-7600K", score: 88, cores: Some(4) },
    // Ojo: 6/6 sin SMT. Mismo IPC que un 10400, muchos menos hilos.
    Cpu { label: "i5-8400", score: 96, cores: Some(6) },
    Cpu { label: "i5-8500", score: 99, cores: Some(6) },
    Cpu { label: "i5-9400F", score: 98, cores: Some(6) },
    Cpu { label: "i5-9600K", score: 112, cores: Some(6) },
    Cpu { label: "i5-10400", score: 108, cores: Some(6) },
    Cpu { label: "i5-10400F", score: 108, cores: Some(6) },
    Cpu { label: "i5-11400", score: 122, cores: Some(6) },
    Cpu { label: "i5-12400", score: 142, cores: Some(6) },
    Cpu { label: "i5-12400F", score: 142, cores: Some(6) },
    Cpu { label: "i5-12600K", score: 158, cores: Some(10) },
    Cpu { label: "i5-13400F", score: 150, cores: Some(10) },
    Cpu { label: "i5-13600K", score: 172, cores: Some(14) },
    Cpu { label: "i5-14600K", score: 178, cores: Some(14) },
    Cpu { label: "i7-3770", score: 68, cores: Some(4) },
    Cpu { label: "i7-4770K", score: 78, cores: Some(4) },
    Cpu { label: "i7-4790K", score: 88, cores: Some(4) },
    Cpu { label: "i7-6700K", score: 98, cores: Some(4) },
    Cpu { label: "i7-7700K", score: 108, cores: Some(4) },
    Cpu { label: "i7-8700K", score: 118, cores: Some(6) },
    Cpu { label: "i7-9700K", score: 128, cores: Some(8) },
    Cpu { label: "i7-10700K", score: 132, cores: Some(8) },
    Cpu { label: "i7-11700K", score: 142, cores: Some(8) },
    Cpu { label: "i7-12700K", score: 162, cores: Some(12) },
    Cpu { label: "i7-13700K", score: 180, cores: Some(16) },
    Cpu { label: "i7-14700K", score: 188, cores: Some(20) },
    Cpu { label: "i3-8100", score: 82, cores: Some(4) },
    Cpu { label: "i3-10100", score: 92, cores: Some(4) },
    Cpu { label: "i3-12100", score: 128, cores: Some(4) },
    Cpu { label: "i9-9900K", score: 136, cores: Some(8) },
    Cpu { label: "i9-10900K", score: 140, cores: Some(10) },
    Cpu { label: "i9-11900K", score: 148, cores: Some(8) },
    Cpu { label: "i9-12900K", score: 172, cores: Some(16) },
    Cpu { label: "i9-13900K", score: 190, cores: Some(24) },
    Cpu { label: "i9-14900K", score: 196, cores: Some(24) },
    // iGPU de portátil: el score es el del chip con límite de vatios, no el de escritorio.
    Cpu { label: "i5-1135G7", score: 84, cores: Some(4) },
    Cpu { label: "i7-1165G7", score: 90, cores: Some(4) },
];

/// Minúsculas y espacios colapsados, con `™`/`®` fuera (Steam mete `Intel®`).
fn normalize(s: &str) -> String {
    s.to_lowercase()
        .replace('®', " ")
        .replace('™', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Busca en `text` el modelo **más específico** de la tabla.
///
/// Se queda con el nombre más largo que aparezca: con `"Ryzen 5 5600X"` no debe ganar
/// `"Ryzen 5 5600"` (tampoco al revés), y con `"i5-12400F"` no debe ganar `"i5-12400"`.
fn find_in(text: &str) -> Option<&'static Cpu> {
    let hay = normalize(text);
    // Sin este guard, `"".contains("")` es `true` y un nombre de CPU vacío devolvería
    // la CPU más larga de la tabla, dando un veredicto falso.
    if hay.is_empty() {
        return None;
    }
    CPUS.iter()
        .filter(|c| hay.contains(&normalize(c.label)))
        .max_by_key(|c| c.label.len())
}

/// CPU del equipo, si está en la tabla.
pub fn find(name: &str) -> Option<&'static Cpu> {
    find_in(name)
}

/// La CPU **más débil** que menciona el texto de Steam.
///
/// Steam lista alternativas unidas por comas o barras (`Ryzen 5 3600 / i5-8400`) y
/// basta con llegar a una de ellas: se compara contra la más baja.
pub fn weakest(text: &str) -> Option<&'static Cpu> {
    // La separación es por comas y barras: **no** se parte por la letra `o`, que
    // dentro de `Ryzen 7 5800 o similar` partiría el nombre del modelo
    // (`"Ryzen 7 5800 "` se reconoce, y eso da un falso veredicto).
    text.split([',', '/', ';', '|'])
        .filter_map(find_in)
        .min_by_key(|c| c.score)
}

/// Resultado de comparar la CPU del equipo con la que pide el juego.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// Modelo que pide Steam (el más débil de la lista).
    pub required: &'static Cpu,
    /// Modelo del equipo.
    pub actual: &'static Cpu,
    /// `actual.score / required.score`. `1.0` = mismo rendimiento por núcleo.
    pub ratio: f32,
    /// `Some(true)` si el equipo tiene menos núcleos que el modelo pedido;
    /// `None` si no se sabe (sin dato del equipo o del modelo).
    pub pocos_nucleos: Option<bool>,
}

/// Compara la CPU del equipo con el requisito de Steam.
///
/// `actual_cores` es el número de núcleos **físicos** del equipo, si se conoce. Con
/// `None` solo se compara el modelo: es lo que se hacía antes de tener la tabla, así
/// que no se pierde nada.
///
/// Devuelve `None` si el texto no menciona ninguna CPU de la tabla o si la CPU del
/// equipo no está en ella. **Nunca** inventa: sin dato, no hay veredicto.
pub fn compare(required_text: &str, actual_name: &str, actual_cores: Option<u32>) -> Option<Comparison> {
    let required = weakest(required_text)?;
    let actual = find(actual_name)?;
    let pocos_nucleos = match (actual_cores, required.cores) {
        (Some(mios), Some(suyos)) => Some(mios < suyos as u32),
        _ => None,
    };
    Some(Comparison {
        required,
        actual,
        ratio: actual.score as f32 / required.score as f32,
        pocos_nucleos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_los_nombres_que_devuelve_linux() {
        // Lo que da `/proc/cpuinfo` o `lscpu` en Linux.
        assert_eq!(find("AMD Ryzen 5 3600 6-Core Processor").map(|c| c.label), Some("Ryzen 5 3600"));
        assert_eq!(
            find("Intel(R) Core(TM) i5-8400 CPU @ 2.80GHz").map(|c| c.label),
            Some("i5-8400")
        );
        // `Intel®` con el símbolo del registro: sin limpiarlo, no habría coincidencia.
        assert_eq!(
            find("Intel® Core™ i7-8700K CPU @ 3.70GHz").map(|c| c.label),
            Some("i7-8700K")
        );
    }

    #[test]
    fn el_modelo_mas_especifico_gana() {
        assert_eq!(find("Ryzen 5 5600X").map(|c| c.label), Some("Ryzen 5 5600X"));
        assert_eq!(find("Ryzen 5 5600").map(|c| c.label), Some("Ryzen 5 5600"));
        assert_eq!(find("i5-12400F").map(|c| c.label), Some("i5-12400F"));
        // Sin la F: no debe coger el 12400F por prefijo.
        assert_eq!(find("i5-12400").map(|c| c.label), Some("i5-12400"));
        // 13600K vs 14600K: números distintos, no deben confundirse.
        assert_eq!(find("i5-13600K").map(|c| c.label), Some("i5-13600K"));
    }

    #[test]
    fn nombre_vacio_no_devuelve_una_cpu_cualquiera() {
        assert_eq!(find(""), None);
        assert_eq!(find("   "), None);
        assert_eq!(find("Procesador genérico"), None);
    }

    #[test]
    fn la_cpu_del_equipo_y_la_pedida_se_comparan_por_nucleo() {
        // Un 12100 rinde más por núcleo que un 8400 (y mucho más que un FX-6300).
        let c = compare("Intel Core i5-8400", "Intel Core i3-12100", Some(4)).unwrap();
        assert_eq!(c.required.label, "i5-8400");
        assert_eq!(c.actual.label, "i3-12100");
        assert!(c.ratio > 1.2, "el 12100 va muy por encima: {}", c.ratio);
        // Pero solo tiene 4 núcleos frente a los 6 pedidos: eso se dice.
        assert_eq!(c.pocos_nucleos, Some(true));
    }

    #[test]
    fn steam_lista_alternativas_y_basta_con_la_mas_debil() {
        // Lo que escribe Steam de verdad, con dos marcas.
        let c = compare("Ryzen 5 3600 / Intel i5-8400", "i3-12100", Some(4)).unwrap();
        // El más débil de los dos es el i5-8400 (96 frente a 100).
        assert_eq!(c.required.label, "i5-8400");
    }

    #[test]
    fn cpu_desconocida_no_da_veredicto() {
        // "Ryzen 3 4300U" no está en la tabla: no se extrapola por el número.
        assert!(compare("i5-8400", "AMD Ryzen 3 4300U", Some(4)).is_none());
        // Y una pedida desconocida tampoco.
        assert!(compare("Pentium G4560", "i5-12400", Some(6)).is_none());
    }

    #[test]
    fn sin_nucleos_conocidos_solo_se_compara_el_modelo() {
        let c = compare("i5-8400", "i3-12100", None).unwrap();
        assert_eq!(c.pocos_nucleos, None);
        assert!(c.ratio > 1.0);
    }

    #[test]
    fn los_nucleos_solo_cuentan_si_se_conocen_los_del_requisito() {
        // `Ryzen 7 7800X3D` tiene `cores: Some(8)`; si el equipo reporta 6, faltan.
        let c = compare("Ryzen 7 5800X", "Ryzen 5 5600", Some(6)).unwrap();
        assert_eq!(c.pocos_nucleos, Some(true));
        let ok = compare("Ryzen 5 3600", "Ryzen 5 5600", Some(6)).unwrap();
        assert_eq!(ok.pocos_nucleos, Some(false));
    }

    #[test]
    fn ningun_score_rompe_el_orden_cpu_vieja_contra_nueva() {
        // Regresión del motivo de la tabla: el número del modelo no sirve.
        let viejo = find("i7-7700K").unwrap();
        let nuevo = find("i3-12100").unwrap();
        assert!(
            nuevo.score > viejo.score,
            "un i3 nuevo rinde mas que un i7 de hace cinco generaciones"
        );
    }

    #[test]
    fn la_escala_es_coherente_con_la_de_gpus() {
        // Ryzen 5 3600 = 100, igual que GTX 1060 en gpu_rank. No es necesario en el
        // código, pero un cambio accidental de escala pasaría desapercibido.
        assert_eq!(find("Ryzen 5 3600").unwrap().score, 100);
    }
}
