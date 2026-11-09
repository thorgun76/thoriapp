//! Requisitos de sistema de un juego y veredicto contra el hardware real.
//!
//! Steam los publica en `appdetails?filters=pc_requirements` como HTML sucio
//! (`<li><strong>Processor:</strong> Intel i5-4460</li>`, con `<br/>`, entidades y
//! textos en cualquier idioma). Aquí se convierte ese HTML en campos tipados y se
//! compara con lo detectado en `hw.rs`.
//!
//! Dos ideas guían el módulo:
//!
//! 1. **Nunca inventar un veredicto.** Si el dato no está, o el nombre del
//!    componente no se sabe comparar, la respuesta es `Unknown`, no "cumples".
//!    Un falso "cumples" cuesta una compra; un "no se sabe" solo cuesta curiosidad.
//! 2. **Es requisito de Windows.** El juego correrá sobre Proton + DXVK (o VKD3D
//!    en DX12), que cuesta más que Windows nativo. Por eso el veredicto final
//!    aplica un margen y lo dice en voz alta (`proton_margin`).
//!
//! Todo lo que se puede probar sin tocar el sistema (parser, comparación, margen)
//! es puro y tiene tests al final.

use crate::cpu_rank;
use crate::gpu_rank;
use serde::{Deserialize, Serialize};

// ───────────────────────────── Tipos ─────────────────────────────

/// Un campo de los requisitos, tal como lo publica Steam.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Requirement {
    /// Bloque del que salió: `minimum` o `recommended`.
    pub tier: String,
    /// `os`, `cpu`, `gpu`, `ram`, `vram`, `storage`, `directx`, `network`, `sound`.
    pub kind: String,
    /// Texto tal cual lo publica Steam (se muestra en la UI).
    pub text: String,
}

/// Veredicto de un requisito frente a la máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Cumple con margen.
    Ok,
    /// Cumple, pero dentro del margen de seguridad (o justo en el límite).
    Tight,
    /// No cumple.
    Fail,
    /// No hay dato suficiente para decidir.
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementCheck {
    pub kind: String,
    pub tier: String,
    /// Qué pide el juego ("8 GB RAM").
    pub required: String,
    /// Qué tiene la máquina ("15.6 GB"), ya formateado.
    pub actual: String,
    pub verdict: Verdict,
    /// Frase corta para la UI ("Necesitas 8 GB de RAM y tienes 15.6 GB").
    pub detail: String,
}

/// Resumen de la comparación.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementsReport {
    /// `None` = Steam no publica requisitos para este juego.
    pub checks: Vec<RequirementCheck>,
    /// Peor veredicto de todos (el que manda para el banner).
    pub overall: Verdict,
    /// Margen ya aplicado a la comparación, en % (0 si no se aplicó).
    pub proton_margin: u32,
    /// Aviso fijo sobre el margen, para que la UI no tenga que inventarlo.
    pub margin_note: String,
    /// Requisitos crudos por si la UI quiere enseñarlos tal cual.
    pub raw: Vec<Requirement>,
}

// ───────────────────────────── Parser ─────────────────────────────

/// Quita etiquetas HTML y entidades básicas de un fragmento.
///
/// `<br/>`, `</li>` y `</strong>` se sustituyen por un espacio: sin eso,
/// `OS: Windows 10<br>Processor: i5` acabaría como `OS: Windows 10Processor: i5`.
pub fn strip_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&deg;", "°");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Nombre del campo a `kind` interno. Se aceptan las variantes en inglés y en
/// español porque Steam devuelve el idioma del juego, no el del cliente.
/// Minúsculas y sin tildes, para que `"Memoria de vídeo"` y `"Memoria de video"`
/// caigan en la misma comparación. Sin esto, la variante acentuada pasaba por
/// `"memoria"` genérico y se clasificaba como RAM en vez de VRAM.
fn normalize_label(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            'ü' => 'u',
            'ñ' => 'n',
            _ => c,
        })
        .collect()
}

pub fn classify_field(label: &str) -> Option<String> {
    let l = normalize_label(label);
    let l = l.trim().trim_end_matches(':').trim();
    let has = |needles: &[&str]| needles.iter().any(|n| l.contains(n));

    // Abreviaturas que se comparan **exactas**: Steam en español abrevia
    // "Sistema Operativo" como "SO", y `"so"` no contiene `"os"`, así que la
    // comparación por subcadena nunca lo detectaba. Eso dejaba el campo del
    // sistema operativo fuera del informe en todo el catálogo en español.
    //
    // Se exige igualdad para no confundir "Sonido" con el SO (que sí contiene
    // "so" como subcadena).
    if matches!(l, "so" | "s.o." | "os" | "o.s." | "sistema operativo" | "operating system") {
        return Some("os".into());
    }
    if matches!(l, "dx" | "d3d" | "directx") {
        return Some("directx".into());
    }

    if has(&["storage", "disk space", "hard drive", "almacenamiento", "disco"]) {
        return Some("storage".into());
    }
    // VRAM antes que RAM: "Memory: 8 GB VRAM" y "Video Memory" contienen "memory".
    if has(&[
        "vram",
        "video memory",
        "dedicated video",
        "memoria de video",
        "memoria grafica",
        "graficos de memoria",
    ]) {
        return Some("vram".into());
    }
    if has(&["memory", "ram", "memoria"]) {
        return Some("ram".into());
    }
    if has(&["processor", "cpu", "procesador"]) {
        return Some("cpu".into());
    }
    if has(&["graphics", "video card", "gpu", "tarjeta grafica", "graficos"]) {
        return Some("gpu".into());
    }
    if has(&["directx", "direct x"]) {
        return Some("directx".into());
    }
    if has(&["network", "red", "internet"]) {
        return Some("network".into());
    }
    if has(&["sound", "audio", "sonido"]) {
        return Some("sound".into());
    }
    if has(&["os", "operating system", "sistema operativo"]) {
        return Some("os".into());
    }
    None
}

/// Convierte el HTML de Steam en requisitos tipados.
///
/// Acepta tanto la forma de `<li>` como la de texto suelto separado por `<br/>`.
/// Lo que no se reconoce se descarta: es preferible perder una línea rara
/// ("Additional Notes") que mostrarla mal clasificada.
pub fn parse_pc_requirements(html: &str) -> Vec<Requirement> {
    let mut out = Vec::new();

    for tier in ["minimum", "recommended"] {
        let block = extract_tier(html, tier);
        let Some(block) = block else { continue };
        for line in split_lines(&block) {
            let line = strip_html(&line);
            if line.is_empty() {
                continue;
            }
            let Some((label, value)) = split_label_value(&line) else {
                continue;
            };
            // Un `:` dentro del valor ("https://...") no debe romper el split:
            // `split_once` ya corta solo por el primero.
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let Some(kind) = classify_field(label) else { continue };
            out.push(Requirement {
                tier: tier.to_string(),
                kind,
                text: value.to_string(),
            });
        }
    }

    out
}

/// Separa `"Etiqueta: valor"` en sus dos partes.
///
/// Si lo que queda antes del primer `:` está vacío, el bloque arrastraba el `:`
/// del encabezado (`<strong>Mínimo:</strong>` → `": SO: Windows 10"`) y el campo
/// se perdía. En ese caso se toma el `:` siguiente. Pasa siempre que el bloque no
/// viene partido en `<li>`, que es el formato más común fuera del inglés.
fn split_label_value(line: &str) -> Option<(&str, &str)> {
    let (label, value) = line.split_once(':')?;
    if !label.trim().is_empty() {
        return Some((label, value));
    }
    let (label2, value2) = value.split_once(':')?;
    if label2.trim().is_empty() {
        return None;
    }
    Some((label2, value2))
}

/// Recorta la sección de un tier. Steam suele devolver
/// `<strong>Minimum:</strong><ul>...</ul><br><strong>Recommended:</strong><ul>...`,
/// pero hay juegos con `<h1>Minimum</h1>` o con el texto en un `<p>`.
fn extract_tier(html: &str, tier: &str) -> Option<String> {
    let lower = html.to_lowercase();
    // Marcadores en singular y plural, y con los acentos de Steam en espanol.
    // Faltaba el singular "mínimo", que es justo el que usa Steam: sin el, el
    // HTML en espanol no se reconocia y la tarjeta salia vacia.
    let start_markers: &[&str] = if tier == "minimum" {
        &["minimum", "mínimo", "minimo", "mínimos", "minimos"]
    } else {
        &[
            "recommended",
            "recomendado",
            "recomendados",
            "recomendación",
            "recomendaciones",
        ]
    };
    let end_markers: &[&str] = if tier == "minimum" {
        &["recommended", "recomendado", "recomendados"]
    } else {
        &[]
    };

    let start = start_markers.iter().filter_map(|m| lower.find(m)).min()?;
    // Se salta el marcador **completo**. Antes era `start + 3`, que solo tenía
    // sentido por casualidad para "minimum": dejaba el bloque empezando en
    // "mum:" y `html.get(after..end)` podía devolver `None` si el corte caía en
    // mitad de un carácter multibyte ("mínimos" lleva í de 2 bytes en UTF-8).
    //
    // Se toma el marcador **más largo** que encaje, no el primero: "mínimo" es
    // prefijo de "mínimos", y quedarse con el corto dejaba la `s` dentro del
    // bloque ("s:</strong> SO: ..."), que rompía la primera línea.
    let marker_len = start_markers
        .iter()
        .filter(|m| lower[start..].starts_with(*m))
        .map(|m| m.len())
        .max()
        .unwrap_or(0);
    let after = start + marker_len;
    let end = end_markers
        .iter()
        .filter_map(|m| lower[after..].find(m).map(|i| after + i))
        .min()
        .unwrap_or(html.len());
    // `find` devuelve offsets de bytes; los marcadores son ASCII salvo los
    // acentuados, y esos también están en `lower` en UTF-8 con la misma longitud
    // que en `html` (to_lowercase no cambia el tamaño de estas letras).
    html.get(after..end).map(String::from)
}

/// Parte el bloque en líneas: primero por `<li>`, y si no hay, por `<br>`.
fn split_lines(block: &str) -> Vec<String> {
    let lower = block.to_lowercase();
    if lower.contains("<li") {
        return block
            .split("<li")
            .skip(1)
            .map(|s| s.split("</li>").next().unwrap_or(s).to_string())
            .collect();
    }
    block
        .split("<br")
        .map(|s| s.to_string())
        .collect()
}

// ───────────────────────────── Comparación ─────────────────────────────

/// Margen extra que se exige por correr en Proton, según la API gráfica.
///
/// - DX12 vía VKD3D es lo más caro: se pide un 20 % más.
/// - DXVK (DX11/10/9) está muy cerca de nativo: 10 %.
/// - Vulkan nativo no necesita margen.
pub fn proton_margin_percent(engine_api: Option<&str>) -> u32 {
    match engine_api.map(|s| s.to_lowercase()).as_deref() {
        Some("vkd3d") | Some("dx12") => 20,
        Some("vulkan") => 0,
        Some("opengl") => 10,
        _ => 10,
    }
}

/// Extrae GB de un texto tipo "8 GB RAM", "8192 MB", "8GB", "50 GB available space".
///
/// Estrategia: se recorre el texto y se guarda **el último** número seguido de una
/// unidad de tamaño (`GB`, `MB`, `TB`). Se prefiere el último con unidad porque
/// textos como `"NVIDIA GTX 1060 6 GB"` o `"RX 580 8GB / GTX 1060 6GB"` empiezan
/// con números de modelo: quedarse con el primero pediría 1060 GB.
/// Solo si no hay ningún número con unidad se acepta el primer número suelto
/// (`"RAM: 8"`), que es GB por convención.
/// VRAM exigida dentro del texto del requisito de GPU.
///
/// Muchos juegos no nombran ninguna gráfica, solo piden "Dedicated graphics card
/// with 4 GB of VRAM minimum". Sin modelo no se puede comparar rendimiento (eso
/// lo hace `gpu_rank`), pero la VRAM sí es un número real y comparable.
///
/// Se exige que el texto hable de memoria gráfica: un "8 GB" suelto en el campo
/// de gráficos suele ser memoria compartida y compararlo con la VRAM dedicada
/// daría un falso veredicto.
fn vram_from_gpu_text(text: &str) -> Option<f32> {
    let l = normalize_label(text);
    let habla_de_memoria = [
        "vram",
        "video memory",
        "graphics memory",
        "dedicated video",
        "dedicated graphics",
        "memoria de video",
        "memoria grafica",
    ]
    .iter()
    .any(|k| l.contains(k));
    if !habla_de_memoria {
        return None;
    }
    parse_size_gb(text)
}

pub fn parse_size_gb(text: &str) -> Option<f32> {
    let lower = text.to_lowercase();

    let mut con_unidad: Option<f32> = None;
    let mut primero_suelto: Option<f32> = None;

    let mut i = 0usize;
    let bytes: Vec<char> = lower.chars().collect();
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        // Número completo (admitiendo un punto decimal).
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == '.') {
            i += 1;
        }
        let num: String = bytes[start..i].iter().collect();
        let Ok(value) = num.parse::<f32>() else { continue };

        // Unidad inmediatamente después, admitiendo un espacio.
        let mut j = i;
        while j < bytes.len() && bytes[j] == ' ' {
            j += 1;
        }
        let unit_start = j;
        while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
            j += 1;
        }
        let unit: String = bytes[unit_start..j].iter().collect();

        let gb = match unit.as_str() {
            "tb" | "t" => Some(value * 1024.0),
            "gb" | "g" => Some(value),
            "mb" | "m" => Some(value / 1024.0),
            "kb" | "k" => Some(value / 1024.0 / 1024.0),
            _ => None,
        };
        if let Some(gb) = gb {
            con_unidad = Some(gb);
        } else if primero_suelto.is_none() {
            primero_suelto = Some(value);
        }
    }

    con_unidad.or(primero_suelto)
}

/// Formatea GB para la UI sin decimales inútiles: 15.6 → "15.6 GB", 16.0 → "16 GB".
pub fn fmt_gb(gb: f32) -> String {
    if (gb - gb.round()).abs() < 0.05 {
        format!("{} GB", gb.round() as i64)
    } else {
        format!("{:.1} GB", gb)
    }
}

/// Lo que la máquina tiene, ya en las unidades que se comparan.
#[derive(Debug, Clone, Default)]
pub struct Machine {
    pub ram_gb: f32,
    pub vram_mb: Option<u64>,
    pub cpu_threads: Option<u32>,
    pub cpu_cores: Option<u32>,
    /// Nombre de la CPU (`/proc/cpuinfo` o `lscpu`). Vacío si no se supo.
    pub cpu_name: String,
    pub gpu_name: String,
    /// Espacio libre en la partición del juego.
    pub free_disk_gb: Option<f32>,
}

/// Compara un requisito numérico (RAM/VRAM/disco) con margen.
///
/// Tres zonas, con `needed = requerido × (1 + margen)`:
///
/// - `Ok`:    `actual >= needed`. Cumple incluso con el coste extra de Proton.
/// - `Tight`: `requerido <= actual < needed`. Cumple el mínimo de Steam en crudo,
///            pero sin el margen que pide Proton. Avisa, no bloquea: es honesto
///            decir "vas justo" cuando el número de Steam es de Windows.
/// - `Fail`:  `actual < requerido`. No llega ni al mínimo publicado.
///
/// Antes había un corte arbitrario de `ratio >= 1.25` para `Ok`, que convertía
/// en "justo" un caso tan sano como 16 GB frente a 12 GB pedidos por Steam.
fn compare_amount(
    kind: &str,
    tier: &str,
    required_gb: f32,
    actual_gb: f32,
    required_text: &str,
    label: &str,
    margin: u32,
) -> RequirementCheck {
    let needed = required_gb * (1.0 + margin as f32 / 100.0);
    let (verdict, detail) = if actual_gb >= needed {
        (
            Verdict::Ok,
            format!("{} pide {} y tienes {}", label, required_text, fmt_gb(actual_gb)),
        )
    } else if actual_gb >= required_gb {
        (
            Verdict::Tight,
            format!(
                "{} pide {} y tienes {}: llega al mínimo, pero sin el margen que pide Proton.",
                label,
                required_text,
                fmt_gb(actual_gb)
            ),
        )
    } else {
        (
            Verdict::Fail,
            format!(
                "{} pide {} y tienes {}.",
                label,
                required_text,
                fmt_gb(actual_gb)
            ),
        )
    };
    RequirementCheck {
        kind: kind.to_string(),
        tier: tier.to_string(),
        required: required_text.to_string(),
        actual: fmt_gb(actual_gb),
        verdict,
        detail,
    }
}

/// Compara los requisitos con la máquina.
///
/// Solo emite veredictos que se pueden sostener, y **descarta el ruido**:
///
///   - RAM, VRAM y disco: numéricos, comparación real con margen.
///   - CPU y GPU: nombres de Windows, sin tabla de equivalencias. Se agrupan en
///     **una sola** fila informativa en vez de dos "Sin dato" que parecían un
///     fallo de la app.
///   - OS, DirectX, red y sonido: **no se emiten**. En Linux no aplican (Windows
///     10 no es tu sistema, DirectX no existe, Proton lo traduce), no hay dato
///     real de red/audio en `Machine`, y cada uno añadía una fila "Sin dato" que
///     no informaba de nada.
pub fn evaluate(
    raw: &[Requirement],
    machine: &Machine,
    engine_api: Option<&str>,
) -> RequirementsReport {
    let margin = proton_margin_percent(engine_api);
    let mut checks = Vec::new();
    // CPU y GPU se acumulan para emitir una única fila al final.
    let mut cpu_req: Option<String> = None;
    let mut gpu_req: Option<String> = None;

    // Solo se evalúa el mínimo: si no cumples el mínimo peligra el juego; el
    // recomendado es "para ir bien" y se queda como dato informativo.
    //
    // El margen de Proton se aplica a RAM y VRAM (Proton y DXVK consumen más que
    // el juego en Windows), pero **no al disco**: el tamaño instalado es el que es.
    for r in raw.iter().filter(|r| r.tier == "minimum") {
        match r.kind.as_str() {
            "ram" => {
                // Si el texto no trae un número legible no se emite check: un
                // "sin dato" explícito por cada línea rara llenaría la tarjeta
                // de ruido sin informar de nada.
                if let Some(req) = parse_size_gb(&r.text) {
                    checks.push(compare_amount(
                        "ram",
                        &r.tier,
                        req,
                        machine.ram_gb,
                        &r.text,
                        "El juego",
                        margin,
                    ));
                }
            }
            "vram" => {
                if let (Some(req), Some(vram_mb)) = (parse_size_gb(&r.text), machine.vram_mb) {
                    checks.push(compare_amount(
                        "vram",
                        &r.tier,
                        req,
                        vram_mb as f32 / 1024.0,
                        &r.text,
                        "El juego",
                        margin,
                    ));
                } else if machine.vram_mb.is_none() {
                    checks.push(RequirementCheck {
                        kind: "vram".into(),
                        tier: r.tier.clone(),
                        required: r.text.clone(),
                        actual: "VRAM compartida o desconocida".into(),
                        verdict: Verdict::Unknown,
                        detail: "Tu GPU no reporta VRAM dedicada (iGPU?). El juego pedirá memoria compartida."
                            .into(),
                    });
                }
            }
            "storage" => {
                if let (Some(req), Some(free)) = (parse_size_gb(&r.text), machine.free_disk_gb) {
                    // El disco no necesita margen de Proton.
                    checks.push(compare_amount(
                        "storage",
                        &r.tier,
                        req,
                        free,
                        &r.text,
                        "El juego",
                        0,
                    ));
                }
            }
            "cpu" => {
                // Misma idea que con la GPU: si las dos CPUs están en la tabla, hay
                // veredicto; si no, cae a la fila informativa de abajo. El score es
                // **por núcleo**, así que el número de núcleos del equipo se mira
                // aparte: un 12100 rinde más por núcleo que un 8400, pero tiene 4
                // frente a 6 y eso un juego que reparte hilos lo nota.
                match cpu_rank::compare(&r.text, &machine.cpu_name, machine.cpu_cores) {
                    Some(cmp) => {
                        let needed = cmp.required.score as f32 * (1.0 + margin as f32 / 100.0);
                        let actual = cmp.actual.score as f32;
                        let pct = ((cmp.ratio - 1.0) * 100.0).round() as i32;
                        // Faltar núcleos es un techo duro: el score por núcleo puede
                        // ser mejor y aun así no llegar a repartir el juego. Se trata
                        // como `Tight` (avisa, no bloquea): los núcleos reportados
                        // pueden ser físicos o lógicos según la máquina.
                        let pocos = cmp.pocos_nucleos == Some(true);
                        let verdict = if pocos {
                            Verdict::Tight
                        } else if actual >= needed {
                            Verdict::Ok
                        } else if actual >= cmp.required.score as f32 {
                            Verdict::Tight
                        } else {
                            Verdict::Fail
                        };
                        let nucleos = if pocos {
                            format!(
                                " Además tienes {} núcleos y el juego pide {} con su {}. Usa el modelo de mínimo o compáralo a mano.",
                                machine.cpu_cores.unwrap_or(0),
                                cmp.required.cores.unwrap_or(0),
                                cmp.required.label
                            )
                        } else {
                            String::new()
                        };
                        // Se resuelve con `if` y no con un `match` que empiece por un
                        // brazo con guarda: un `_ if pocos` delante de los brazos
                        // concretos compila, pero deja los demás patrones "alcanzables
                        // solo por exclusión" y es fácil de romper al añadir un caso.
                        let detail = if pocos {
                            format!(
                                "Tu {} rinde por encima del {} que pide, pero con menos núcleos.{}",
                                cmp.actual.label, cmp.required.label, nucleos
                            )
                        } else {
                            match verdict {
                                Verdict::Ok => format!(
                                    "El juego pide un {} como mínimo y tu {} rinde un {} % más por núcleo.{}",
                                    cmp.required.label, cmp.actual.label, pct, nucleos
                                ),
                                // `actual >= required` pero sin el margen de Proton.
                                Verdict::Tight => format!(
                                    "Tu {} rinde un {} % más por núcleo que el {} que pide, pero sin el margen extra de Proton.{}",
                                    cmp.actual.label, pct, cmp.required.label, nucleos
                                ),
                                _ => format!(
                                    "El juego pide un {} como mínimo y tu {} rinde un {} % menos por núcleo.{}",
                                    cmp.required.label,
                                    cmp.actual.label,
                                    pct.abs(),
                                    nucleos
                                ),
                            }
                        };
                        checks.push(RequirementCheck {
                            kind: "cpu".into(),
                            tier: r.tier.clone(),
                            required: cmp.required.label.to_string(),
                            actual: cmp.actual.label.to_string(),
                            verdict,
                            detail,
                        });
                    }
                    None => cpu_req = Some(r.text.clone()),
                }
            }
            // GPU: si la tarjeta del equipo y alguna de las que pide Steam están
            // en la tabla, se compara por rendimiento real. Si no, cae a la fila
            // informativa de abajo (nunca un veredicto inventado).
            "gpu" => {
                if let Some(cmp) = gpu_rank::compare(&r.text, &machine.gpu_name) {
                    let needed = cmp.required.score as f32 * (1.0 + margin as f32 / 100.0);
                    let actual = cmp.actual.score as f32;
                    let pct = ((cmp.ratio - 1.0) * 100.0).round() as i32;
                    let verdict = if actual >= needed {
                        Verdict::Ok
                    } else if actual >= cmp.required.score as f32 {
                        Verdict::Tight
                    } else {
                        Verdict::Fail
                    };
                    let detail = match verdict {
                        Verdict::Ok => format!(
                            "El juego pide una {} como mínimo y tu {} rinde un {} % más.",
                            cmp.required.label, cmp.actual.label, pct
                        ),
                        Verdict::Tight => format!(
                            "Tu {} rinde como la {} que pide, pero sin el margen extra de Proton.",
                            cmp.actual.label, cmp.required.label
                        ),
                        _ => format!(
                            "El juego pide una {} como mínimo y tu {} rinde un {} % menos.",
                            cmp.required.label,
                            cmp.actual.label,
                            pct.abs()
                        ),
                    };
                    checks.push(RequirementCheck {
                        kind: "gpu".into(),
                        tier: r.tier.clone(),
                        required: cmp.required.label.to_string(),
                        actual: cmp.actual.label.to_string(),
                        verdict,
                        detail,
                    });
                } else if let Some(req_gb) = vram_from_gpu_text(&r.text) {
                    // Sin modelo pero con VRAM: se compara la memoria, que es un
                    // dato real. Es el caso de "Dedicated graphics card with 4 GB
                    // of VRAM minimum".
                    if let Some(mb) = machine.vram_mb {
                        checks.push(compare_amount(
                            "vram",
                            &r.tier,
                            req_gb,
                            mb as f32 / 1024.0,
                            &format!("{} de VRAM", fmt_gb(req_gb)),
                            "El juego",
                            margin,
                        ));
                    } else {
                        checks.push(RequirementCheck {
                            kind: "vram".into(),
                            tier: r.tier.clone(),
                            required: format!("{} de VRAM", fmt_gb(req_gb)),
                            actual: "VRAM compartida o desconocida".into(),
                            verdict: Verdict::Unknown,
                            detail: "Tu GPU no reporta VRAM dedicada (iGPU?). El juego pedirá memoria compartida."
                                .into(),
                        });
                    }
                } else {
                    gpu_req = Some(r.text.clone());
                }
            }
            // `os`, `directx`, `network`, `sound` y el cajón `other` se ignoran a
            // propósito: no hay forma de evaluarlos en Linux y solo metían ruido.
            _ => {}
        }
    }

    // Una sola fila con lo que Steam exige de CPU/GPU cuando **no** hay tabla para
    // compararlo. Sin veredicto inventado: si la CPU o la GPU no están en las tablas
    // de `cpu_rank`/`gpu_rank`, no se extrapola por el número del modelo.
    if cpu_req.is_some() || gpu_req.is_some() {
        let mut parts = Vec::new();
        if let Some(c) = &cpu_req {
            parts.push(format!("CPU: {c}"));
        }
        if let Some(g) = &gpu_req {
            parts.push(format!("GPU: {g}"));
        }
        let tu_hardware = {
            let mut bits = Vec::new();
            if !machine.gpu_name.is_empty() {
                bits.push(machine.gpu_name.clone());
            }
            if !machine.cpu_name.is_empty() {
                bits.push(machine.cpu_name.clone());
            }
            match (machine.cpu_cores, machine.cpu_threads) {
                (Some(c), Some(t)) => bits.push(format!("{c} núcleos / {t} hilos")),
                (Some(c), None) => bits.push(format!("{c} núcleos")),
                _ => {}
            }
            if bits.is_empty() {
                "Sin dato del equipo".to_string()
            } else {
                bits.join(" · ")
            }
        };
        checks.push(RequirementCheck {
            kind: "hardware".into(),
            tier: "minimum".into(),
            required: parts.join(" · "),
            actual: tu_hardware,
            verdict: Verdict::Unknown,
            detail: "Steam da modelos de Windows y no hay tabla de equivalencias fiable: compáralo a mano."
                .into(),
        });
    }

    let overall = checks.iter().fold(Verdict::Unknown, |acc, c| match (acc, c.verdict) {
        (Verdict::Fail, _) | (_, Verdict::Fail) => Verdict::Fail,
        (Verdict::Tight, _) | (_, Verdict::Tight) => Verdict::Tight,
        (Verdict::Ok, _) | (_, Verdict::Ok) => Verdict::Ok,
        _ => Verdict::Unknown,
    });

    let margin_note = if margin == 0 {
        "El juego usa Vulkan nativo: sin margen extra.".to_string()
    } else {
        format!(
            "Estos requisitos son de Windows. En Linux con Proton se pide un {} % más de margen ({}).",
            margin,
            if margin >= 20 {
                "DX12 vía VKD3D, lo más caro"
            } else {
                "DX11 vía DXVK"
            }
        )
    };

    RequirementsReport {
        checks,
        overall,
        proton_margin: margin,
        margin_note,
        raw: raw.to_vec(),
    }
}

/// Espacio libre (GB) en la partición que contiene `path`.
///
/// Se resuelve la ruta real con `canonicalize` porque el `install_path` puede ser
/// un enlace o una ruta relativa; si no existe, se mide su ancestro más cercano
/// que sí exista (una carpeta borrada a mano no debe dar "desconocido").
/// Si no hay `path`, se mide `/`.
///
/// Se elige el punto de montaje **más específico**: con `/home` en otro disco,
/// `starts_with("/")` también encajaría y daría el espacio del disco raíz.
pub fn free_disk_gb(path: Option<&str>) -> Option<f32> {
    let start = path
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/"));
    let target = ancestors_existentes(&start);

    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| target.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space() as f32 / 1024.0 / 1024.0 / 1024.0)
}

fn ancestors_existentes(start: &std::path::Path) -> std::path::PathBuf {
    let mut cur = start.to_path_buf();
    loop {
        if let Ok(real) = cur.canonicalize() {
            return real;
        }
        match cur.parent() {
            Some(p) if p != cur => cur = p.to_path_buf(),
            _ => return std::path::PathBuf::from("/"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HTML real abreviado de un juego AAA de Steam (Windows).
    const STEAM_HTML: &str = "<strong>Minimum:</strong><br><ul class=\"bb_ul\"><li><strong>OS:</strong> Windows 10 64-bit<br></li><li><strong>Processor:</strong> Intel Core i5-8400 / AMD Ryzen 3 3300X<br></li><li><strong>Memory:</strong> 12 GB RAM<br></li><li><strong>Graphics:</strong> NVIDIA GeForce GTX 1060 6GB / AMD Radeon RX 580 8GB<br></li><li><strong>DirectX:</strong> Version 12<br></li><li><strong>Storage:</strong> 60 GB available space<br></li></ul><br><strong>Recommended:</strong><br><ul class=\"bb_ul\"><li><strong>Processor:</strong> Intel Core i7-8700<br></li><li><strong>Memory:</strong> 16 GB RAM<br></li><li><strong>Graphics:</strong> NVIDIA GeForce RTX 2060<br></li></ul>";

    #[test]
    fn parsea_minimos_y_recomendados() {
        let r = parse_pc_requirements(STEAM_HTML);
        let min: Vec<_> = r.iter().filter(|x| x.tier == "minimum").collect();
        let rec: Vec<_> = r.iter().filter(|x| x.tier == "recommended").collect();
        assert_eq!(min.len(), 6);
        assert_eq!(rec.len(), 3);
        assert!(min.iter().any(|x| x.kind == "ram" && x.text == "12 GB RAM"));
        assert!(min.iter().any(|x| x.kind == "storage" && x.text.contains("60 GB")));
        assert!(rec.iter().any(|x| x.kind == "cpu" && x.text.contains("i7-8700")));
    }

    #[test]
    fn clasifica_vram_antes_que_ram() {
        assert_eq!(classify_field("VRAM"), Some("vram".into()));
        assert_eq!(classify_field("Video Memory"), Some("vram".into()));
        assert_eq!(classify_field("Memory"), Some("ram".into()));
        assert_eq!(classify_field("Memoria"), Some("ram".into()));
        assert_eq!(classify_field("Memoria de vídeo"), Some("vram".into()));
    }

    #[test]
    fn so_en_espanol_se_clasifica_como_sistema_operativo() {
        // Steam en espanol abrevia "Sistema Operativo" como "SO". La comparacion
        // por subcadena buscaba "os" y "so" no lo contiene, asi que el campo se
        // perdia en todo el catalogo en espanol.
        assert_eq!(classify_field("SO"), Some("os".into()));
        assert_eq!(classify_field("SO:"), Some("os".into()));
        assert_eq!(classify_field("S.O."), Some("os".into()));
        assert_eq!(classify_field("Sistema Operativo"), Some("os".into()));
        // PERO "Sonido" no es el sistema operativo, aunque contenga "so".
        assert_eq!(classify_field("Sonido"), Some("sound".into()));
        assert_eq!(classify_field("Tarjeta de sonido"), Some("sound".into()));
    }

    #[test]
    fn html_real_en_espanol_da_los_tres_campos_publicados() {
        // HTML real de Steam en espanol. Antes de arreglar "SO" solo salian cpu
        // y storage: la tarjeta mostraba 2 filas aunque Steam publica 3 campos.
        let html = "<strong>Mínimo:</strong><br><ul class=\"bb_ul\"><li>Requiere un procesador y un sistema operativo de 64 bits<br></li><li><strong>SO:</strong> Windows 10<br></li><li><strong>Procesador:</strong> Intel Core i3-8130U 2.20 GHz<br></li><li><strong>Almacenamiento:</strong> 500 MB de espacio disponible<br></li><li><strong>Notas adicionales:</strong> 16x9</li></ul>";
        let parsed = parse_pc_requirements(html);
        let kinds: Vec<&str> = parsed.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(kinds, vec!["os", "cpu", "storage"]);
        assert_eq!(parsed[0].text, "Windows 10");
    }

    #[test]
    fn encabezado_acentuado_no_se_come_el_bloque() {
        // "Mínimos" lleva i acentuada de 2 bytes en UTF-8. Con el viejo
        // "start + 3" el corte podia caer en mitad del caracter y perder el
        // bloque entero.
        let html = "<strong>Mínimos:</strong> SO: Windows 10<br>Memoria: 8 GB RAM";
        let parsed = parse_pc_requirements(html);
        assert!(!parsed.is_empty(), "el bloque se perdio con el encabezado acentuado");
        assert!(parsed.iter().any(|r| r.kind == "os"));
    }

    #[test]
    fn clasifica_en_espanol() {
        assert_eq!(classify_field("Procesador"), Some("cpu".into()));
        assert_eq!(classify_field("Tarjeta gráfica"), Some("gpu".into()));
        assert_eq!(classify_field("Almacenamiento"), Some("storage".into()));
        assert_eq!(classify_field("Sistema Operativo"), Some("os".into()));
    }

    #[test]
    fn ignora_campos_desconocidos() {
        assert_eq!(classify_field("Additional Notes"), None);
        assert_eq!(classify_field("Sound Card"), Some("sound".into()));
    }

    #[test]
    fn html_sin_li_usa_br() {
        let html = "<strong>Minimum:</strong>OS: Windows 10<br>Processor: i5<br>Memory: 8 GB RAM";
        let r = parse_pc_requirements(html);
        assert!(r.iter().any(|x| x.kind == "cpu" && x.text == "i5"));
        assert!(r.iter().any(|x| x.kind == "ram" && x.text == "8 GB RAM"));
    }

    #[test]
    fn html_vacio_o_basura_no_paniquea() {
        assert!(parse_pc_requirements("").is_empty());
        assert!(parse_pc_requirements("<ul></ul>").is_empty());
        assert!(parse_pc_requirements("sin dos puntos aquí").is_empty());
    }

    #[test]
    fn strip_html_normaliza_espacios_y_entidades() {
        assert_eq!(
            strip_html("<li><strong>OS:</strong> Windows&nbsp;10<br></li>"),
            "OS: Windows 10"
        );
    }

    #[test]
    fn strip_html_separa_al_quitar_etiquetas() {
        // Sin el espacio de reemplazo saldra "10Processor" y el parser perderia la linea.
        let limpio = strip_html("OS: Windows 10<br>Processor: i5");
        assert_eq!(limpio, "OS: Windows 10 Processor: i5");
        assert!(!limpio.contains("10Processor"));
    }

    #[test]
    fn requisito_en_mb_se_normaliza_a_gb() {
        let html = "<strong>Minimum:</strong><ul><li><strong>Memory:</strong> 8192 MB RAM</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(16.0, Some(8192), Some(200.0)), Some("dxvk"));
        let ram = rep.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(ram.verdict, Verdict::Ok);
    }

    #[test]
    fn texto_de_ram_sin_numero_no_emite_check() {
        let html = "<strong>Minimum:</strong><ul><li><strong>Memory:</strong> tanta como puedas</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(16.0, Some(8192), Some(200.0)), Some("dxvk"));
        assert!(!rep.checks.iter().any(|c| c.kind == "ram"));
    }

    #[test]
    fn ruta_inexistente_no_paniquea() {
        // No se prueba el valor (depende de la maquina), solo que no entra en panico
        // y devuelve algo o nada, sin colgarse.
        let _ = free_disk_gb(Some("/ruta/que/no/existe/123"));
        let _ = free_disk_gb(None);
        let _ = free_disk_gb(Some(""));
    }

    #[test]
    fn parsea_tamanos_en_gb_mb_y_tb() {
        assert_eq!(parse_size_gb("8 GB RAM"), Some(8.0));
        assert_eq!(parse_size_gb("8GB"), Some(8.0));
        assert_eq!(parse_size_gb("8192 MB RAM"), Some(8.0));
        assert_eq!(parse_size_gb("1 TB"), Some(1024.0));
        assert_eq!(parse_size_gb("60 GB available space"), Some(60.0));
        assert_eq!(parse_size_gb("sin numeros"), None);
    }

    #[test]
    fn no_confunde_el_modelo_de_gpu_con_el_tamano() {
        // El numero de modelo no lleva unidad: debe ganar "6 GB".
        assert_eq!(parse_size_gb("NVIDIA GTX 1060 6 GB"), Some(6.0));
        assert_eq!(parse_size_gb("AMD Radeon RX 580 8GB"), Some(8.0));
    }

    #[test]
    fn sin_unidad_usa_el_primer_numero() {
        assert_eq!(parse_size_gb("RAM: 8"), Some(8.0));
    }

    #[test]
    fn formatea_gb_legible() {
        assert_eq!(fmt_gb(16.0), "16 GB");
        assert_eq!(fmt_gb(15.6171875), "15.6 GB");
    }

    #[test]
    fn margen_de_proton_por_api() {
        assert_eq!(proton_margin_percent(Some("vkd3d")), 20);
        assert_eq!(proton_margin_percent(Some("dxvk")), 10);
        assert_eq!(proton_margin_percent(Some("vulkan")), 0);
        assert_eq!(proton_margin_percent(None), 10);
    }

    fn maquina(ram: f32, vram_mb: Option<u64>, libre: Option<f32>) -> Machine {
        Machine {
            ram_gb: ram,
            vram_mb,
            cpu_threads: Some(12),
            cpu_cores: Some(6),
            // La CPU del equipo de prueba: esta en , para que las pruebas
            // de evaluacion no dependan de tener la tabla a medias.
            cpu_name: "AMD Ryzen 5 3600 6-Core Processor".into(),
            gpu_name: "AMD Radeon RX 6800 XT".into(),
            free_disk_gb: libre,
        }
    }

    #[test]
    fn ram_holgada_es_ok() {
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(16.0, Some(8192), Some(200.0)), Some("dxvk"));
        let ram = rep.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(ram.verdict, Verdict::Ok);
        assert_eq!(ram.required, "12 GB RAM");
    }

    #[test]
    fn ram_justa_avisa_en_vez_de_fallar() {
        // 12 GB pedidos + 10 % de margen de DXVK = 13.2 GB. Con 13 GB se cumple el
        // minimo pelado pero no el margen: Tight, no Fail.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(13.0, Some(8192), Some(200.0)), Some("dxvk"));
        let ram = rep.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(ram.verdict, Verdict::Tight);
        assert!(ram.detail.contains("margen"));
    }

    #[test]
    fn holgura_por_encima_del_margen_es_ok() {
        // 14 GB >= 13.2 GB exigidos con DXVK.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(14.0, Some(8192), Some(200.0)), Some("dxvk"));
        let ram = rep.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(ram.verdict, Verdict::Ok);
    }

    #[test]
    fn justo_en_el_limite_exacto_es_tight() {
        // 12 GB clavados: cumple el minimo publicado, sin margen para Proton.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(12.0, Some(8192), Some(200.0)), Some("dxvk"));
        let ram = rep.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(ram.verdict, Verdict::Tight);
    }

    #[test]
    fn apenas_por_debajo_del_minimo_es_fail() {
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(11.9, Some(8192), Some(200.0)), Some("dxvk"));
        let ram = rep.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(ram.verdict, Verdict::Fail);
    }

    #[test]
    fn ram_insuficiente_falla() {
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(8.0, Some(8192), Some(200.0)), Some("dxvk"));
        assert_eq!(rep.overall, Verdict::Fail);
        let ram = rep.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(ram.verdict, Verdict::Fail);
    }

    #[test]
    fn el_margen_de_proton_sube_el_liston() {
        // 12.5 GB sobre 12 GB pedidos: con Vulkan (margen 0) es Ok; con DXVK
        // (+10 % = 13.2 GB) pasa a Tight. El margen cambia el veredicto en el
        // limite, que es justo lo que se quiere demostrar.
        let raw = parse_pc_requirements(STEAM_HTML);
        let vulkan = evaluate(&raw, &maquina(12.5, Some(8192), Some(200.0)), Some("vulkan"));
        let dxvk = evaluate(&raw, &maquina(12.5, Some(8192), Some(200.0)), Some("dxvk"));
        let v = vulkan.checks.iter().find(|c| c.kind == "ram").unwrap();
        let d = dxvk.checks.iter().find(|c| c.kind == "ram").unwrap();
        assert_eq!(v.verdict, Verdict::Ok);
        assert_eq!(d.verdict, Verdict::Tight);
    }

    #[test]
    fn vkd3d_es_el_mas_estricto() {
        // 12.5 GB: Ok con Vulkan, Tight con DXVK, y Fail con VKD3D solo si el
        // minimo crudo no se alcanza. Aqui 12.5 >= 12, asi que VKD3D es Tight:
        // lo que importa es que nunca es mas laxo que DXVK.
        let raw = parse_pc_requirements(STEAM_HTML);
        let dxvk = evaluate(&raw, &maquina(12.5, Some(8192), Some(200.0)), Some("dxvk"));
        let vkd3d = evaluate(&raw, &maquina(12.5, Some(8192), Some(200.0)), Some("vkd3d"));
        assert_eq!(vkd3d.proton_margin, 20);
        assert_eq!(vkd3d.overall, Verdict::Tight);
        assert_eq!(dxvk.overall, Verdict::Tight);
    }

    #[test]
    fn disco_no_lleva_margen_de_proton() {
        // 60 GB pedidos y 70 GB libres: con margen 0, 70 >= 60 -> Ok.
        // Si se aplicara el +20 % de VKD3D el liston seria 72 GB y saldria Tight.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(70.0)), Some("vkd3d"));
        let d = rep.checks.iter().find(|c| c.kind == "storage").unwrap();
        assert_eq!(d.verdict, Verdict::Ok);
    }

    #[test]
    fn disco_insuficiente_falla() {
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(12.0)), Some("dxvk"));
        let d = rep.checks.iter().find(|c| c.kind == "storage").unwrap();
        assert_eq!(d.verdict, Verdict::Fail);
        assert_eq!(d.actual, "12 GB");
    }

    #[test]
    fn la_gpu_si_se_compara_por_rendimiento() {
        // El HTML pide "GTX 1060 6GB / RX 580 8GB" y el equipo tiene una 6800 XT.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(200.0)), Some("dxvk"));
        let gpu = rep.checks.iter().find(|c| c.kind == "gpu").unwrap();
        assert_eq!(gpu.verdict, Verdict::Ok);
        assert_eq!(gpu.actual, "RX 6800 XT");
        assert!(gpu.detail.contains("más"), "detalle fuera de lo esperado: {}", gpu.detail);
    }

    #[test]
    fn vram_en_el_texto_de_gpu_se_compara() {
        // Caso real: el juego no nombra ninguna grafica, solo pide VRAM.
        let html = "<strong>Minimum:</strong><ul><li><strong>Graphics:</strong> Dedicated graphics card with 4 GB of VRAM minimum, OpenGL 2.1 and GLSL 1.2 support</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(8.0, Some(8192), Some(100.0)), Some("dxvk"));
        let v = rep.checks.iter().find(|c| c.kind == "vram").unwrap();
        assert_eq!(v.required, "4 GB de VRAM");
        assert_eq!(v.verdict, Verdict::Ok);
        // Y no se duplica como fila informativa.
        assert!(rep.checks.iter().all(|c| c.kind != "hardware"));
    }

    #[test]
    fn un_gb_suelto_en_graficos_no_se_toma_por_vram() {
        // Sin la palabra VRAM, un "8 GB" puede ser memoria compartida: no se usa.
        let html = "<strong>Minimum:</strong><ul><li><strong>Graphics:</strong> tarjeta de 8 GB</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(16.0, Some(8192), Some(100.0)), Some("dxvk"));
        assert!(rep.checks.iter().all(|c| c.kind != "vram"));
        assert!(rep.checks.iter().any(|c| c.kind == "hardware"));
    }

    #[test]
    fn gpu_desconocida_cae_a_la_fila_informativa() {
        // "GTX 555" no esta en la tabla: no se inventa veredicto.
        let html = "<strong>Minimum:</strong><ul><li><strong>Graphics:</strong> GTX 555</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(200.0)), Some("dxvk"));
        assert!(rep.checks.iter().all(|c| c.kind != "gpu"));
        let hw = rep.checks.iter().find(|c| c.kind == "hardware").unwrap();
        assert!(hw.required.contains("GTX 555"));
        assert_eq!(hw.verdict, Verdict::Unknown);
    }

    #[test]
    fn cpu_se_compara_por_modelo_cuando_los_dos_estan_en_la_tabla() {
        // Antes la CPU caia siempre en la fila informativa ("comparalo a mano");
        // ahora hay tabla, igual que para GPUs.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(200.0)), Some("dxvk"));

        let cpu = rep
            .checks
            .iter()
            .find(|c| c.kind == "cpu")
            .expect("con i5-8400 pedido y un Ryzen 5 3600 hay veredicto");
        assert_eq!(cpu.required, "i5-8400");
        assert_eq!(cpu.actual, "Ryzen 5 3600");
        // 100 frente a 96: cumple, aunque sin margen de sobra (Proton pide 10 %).
        assert_eq!(cpu.verdict, Verdict::Tight);
        assert!(cpu.detail.contains("margen"));

        // Y esa CPU ya no aparece en la fila informativa: con las dos en la tabla
        // hay veredicto, asi que `cpu_req` se queda vacio y la fila `hardware` ni
        // se emite (la `maquina()` de prueba tiene GPU tambien en la tabla).
        assert!(
            rep.checks.iter().all(|c| c.kind != "hardware"),
            "con CPU y GPU comparables no hace falta la fila informativa: {:?}",
            rep.checks
        );
    }

    #[test]
    fn cpu_y_gpu_fuera_de_la_tabla_si_van_a_la_fila_informativa() {
        // El caso contrario al de arriba: si la GPU no esta en la tabla, su texto
        // tiene que aparecer en `hardware` para que el usuario lo compare a mano.
        let html = "<strong>Minimum:</strong><ul>\
                    <li><strong>Processor:</strong> Intel i5-8400</li>\
                    <li><strong>Graphics:</strong> GPU inexistente XYZ</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(200.0)), Some("dxvk"));
        let hw = rep
            .checks
            .iter()
            .find(|c| c.kind == "hardware")
            .expect("la GPU desconocida deberia dejar una fila informativa");
        assert!(hw.required.contains("GPU inexistente XYZ"), "{}", hw.required);
        // La CPU si estaba en la tabla, asi que no se repite ahi.
        assert!(!hw.required.contains("i5-8400"), "{}", hw.required);
        // Y si hay veredicto de CPU.
        assert!(rep.checks.iter().any(|c| c.kind == "cpu"));
    }

    #[test]
    fn cpu_fuerte_cumple_con_margen() {
        let html = "<strong>Minimum:</strong><ul><li><strong>Processor:</strong> Intel i5-4460</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(200.0)), Some("dxvk"));
        let cpu = rep.checks.iter().find(|c| c.kind == "cpu").unwrap();
        assert_eq!(cpu.verdict, Verdict::Ok);
        assert_eq!(cpu.required, "i5-4460");
    }

    #[test]
    fn cpu_corta_falla() {
        // Un FX-6300 no llega a lo que pide un i5-8400.
        let html = "<strong>Minimum:</strong><ul><li><strong>Processor:</strong> Intel i5-8400</li></ul>";
        let raw = parse_pc_requirements(html);
        let mut m = maquina(32.0, Some(8192), Some(200.0));
        m.cpu_name = "AMD FX-6300 Six-Core Processor".into();
        m.cpu_cores = Some(6);
        m.cpu_threads = Some(6);
        let rep = evaluate(&raw, &m, Some("dxvk"));
        let cpu = rep.checks.iter().find(|c| c.kind == "cpu").unwrap();
        assert_eq!(cpu.verdict, Verdict::Fail);
        assert_eq!(rep.overall, Verdict::Fail);
    }

    #[test]
    fn cpu_desconocida_cae_a_la_fila_informativa() {
        // Si tu CPU no esta en la tabla no se extrapola por el numero del modelo.
        let html = "<strong>Minimum:</strong><ul><li><strong>Processor:</strong> Intel i5-8400</li></ul>";
        let raw = parse_pc_requirements(html);
        let mut m = maquina(32.0, Some(8192), Some(200.0));
        m.cpu_name = "AMD Ryzen 3 4300U".into();
        let rep = evaluate(&raw, &m, Some("dxvk"));
        assert!(rep.checks.iter().all(|c| c.kind != "cpu"));
        let hw = rep.checks.iter().find(|c| c.kind == "hardware").unwrap();
        assert!(hw.required.contains("i5-8400"));
        assert_eq!(hw.verdict, Verdict::Unknown);
    }

    #[test]
    fn faltar_nucleos_avisa_aunque_el_ipc_sea_mejor() {
        // Un 12100 rinde mas por nucleo que un 8400, pero tiene 4 nucleos frente a 6.
        let html = "<strong>Minimum:</strong><ul><li><strong>Processor:</strong> Intel i5-8400</li></ul>";
        let raw = parse_pc_requirements(html);
        let mut m = maquina(32.0, Some(8192), Some(200.0));
        m.cpu_name = "Intel(R) Core(TM) i3-12100 CPU @ 3.30GHz".into();
        m.cpu_cores = Some(4);
        m.cpu_threads = Some(8);
        let rep = evaluate(&raw, &m, Some("dxvk"));
        let cpu = rep.checks.iter().find(|c| c.kind == "cpu").unwrap();
        assert_eq!(cpu.verdict, Verdict::Tight, "mejor IPC pero menos nucleos no es Ok");
        assert!(cpu.detail.contains("núcleos"));
    }

    #[test]
    fn gpu_peor_que_la_pedida_falla() {
        let html = "<strong>Minimum:</strong><ul><li><strong>Graphics:</strong> RTX 3080</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(200.0)), Some("dxvk"));
        let gpu = rep.checks.iter().find(|c| c.kind == "gpu").unwrap();
        assert_eq!(gpu.verdict, Verdict::Fail);
        assert_eq!(rep.overall, Verdict::Fail);
    }

    #[test]
    fn no_emite_ruido_que_no_aplica_en_linux() {
        // SO Windows, DirectX, red y sonido no se pueden evaluar en Linux.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(32.0, Some(8192), Some(200.0)), Some("dxvk"));
        for kind in ["os", "directx", "network", "sound"] {
            assert!(
                rep.checks.iter().all(|c| c.kind != kind),
                "el informe no deberia incluir '{kind}'"
            );
        }
        // Y lo que si importa sigue estando. La CPU y la GPU de `maquina()` estan
        // las dos en sus tablas, asi que salen como `cpu`/`gpu` con veredicto, no
        // como texto informativo suelto en `hardware`.
        for kind in ["ram", "storage", "cpu", "gpu"] {
            assert!(rep.checks.iter().any(|c| c.kind == kind), "falta '{kind}'");
        }
    }

    #[test]
    fn igpu_sin_vram_es_unknown_no_fail() {
        let html = "<strong>Minimum:</strong><ul><li><strong>VRAM:</strong> 4 GB</li></ul>";
        let raw = parse_pc_requirements(html);
        let rep = evaluate(&raw, &maquina(16.0, None, Some(100.0)), Some("dxvk"));
        let v = rep.checks.iter().find(|c| c.kind == "vram").unwrap();
        assert_eq!(v.verdict, Verdict::Unknown);
    }

    #[test]
    fn sin_requisitos_no_hay_veredicto() {
        let rep = evaluate(&[], &maquina(16.0, Some(8192), Some(100.0)), Some("dxvk"));
        assert!(rep.checks.is_empty());
        assert_eq!(rep.overall, Verdict::Unknown);
    }

    #[test]
    fn el_recomendado_no_manda_en_el_veredicto() {
        // El recomendado pide 16 GB; con 13 GB el minimo (12) se cumple con el
        // margen de Vulkan (0 %) y el recomendado no debe hundir el resultado.
        let raw = parse_pc_requirements(STEAM_HTML);
        let rep = evaluate(&raw, &maquina(13.0, Some(8192), Some(200.0)), Some("vulkan"));
        assert_eq!(rep.overall, Verdict::Ok);
        assert!(!rep.checks.iter().any(|c| c.tier == "recommended"));
    }
}
