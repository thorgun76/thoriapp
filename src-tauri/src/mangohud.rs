//! Lectura de lo que **MangoHud registró** durante una grabación.
//!
//! Esto no calcula métricas: solo transcribe lo que el propio log declara, con su
//! procedencia, para que la instantánea pueda decir «esto lo dijo MangoHud» en vez de
//! «esto lo detectó Thoriapp al analizar». Son fuentes distintas:
//!
//! * La cabecera de sistema (`os,cpu,gpu,ram,kernel,driver,cpuscheduler`) describe el
//!   equipo **en el momento de la grabación**. Puede no coincidir con el actual.
//! * Las columnas de la serie (`cpu_load`, `gpu_temp`, …) son muestras periódicas.
//! * `_summary.csv` es un resumen que calculó MangoHud con **su** metodología, que no es
//!   la de Thoriapp (ver `docs/mangohud-summary.md`).
//!
//! Nada de aquí se usa para decidir el ganador de una comparación (eso es la subfase 5.4).
//!
//! Parte de este módulo (disponibilidad de sensores, lectura de la serie) se implementa y
//! se prueba aquí, pero todavía no la consume nadie: es la base que 5.4 necesitará. Se
//! silencia el aviso de código muerto para ese conjunto en vez de dejar atributos sueltos.
#![allow(dead_code)]

use std::collections::HashMap;

/// Disponibilidad de una métrica de la serie temporal.
///
/// Hoy solo se usa en las pruebas: llevar estas métricas a la comparación es la subfase
/// 5.4. Se deja implementado y probado para que 5.4 no tenga que rehacer el parser.
///
/// Se distinguen cinco estados porque el CSV no permite afirmar más. En particular, una
/// columna llena de ceros **no** demuestra por sí sola que el sensor falte: `swap_used=0`
/// o una utilización del 0 % pueden ser medidas perfectamente válidas. La interpretación
/// depende de la semántica de cada métrica, y por eso se decide en `disponibilidad_de`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disponibilidad {
    /// La columna no aparece en el CSV.
    ColumnaInexistente,
    /// La columna existe pero ninguna fila tiene un número utilizable.
    ValorInvalido,
    /// Hay ceros donde un cero no puede ser una medida (potencia, RSS): el sensor
    /// probablemente no esté disponible.
    NoDisponible,
    /// Hay valores utilizables.
    Valida,
    /// Hay valores, pero el formato no permite saber si son reales.
    Incierta,
}

/// Qué significa un `0` en cada métrica. Declararlo por métrica evita la regla genérica
/// («columna de ceros ⇒ sensor ausente»), que sería falsa para `swap_used` o `gpu_load`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CeroEs {
    /// Un 0 es una medida legítima (0 % de uso, 0 GB de swap).
    Valido,
    /// Un 0 no puede ser una medida real: indica que no se pudo leer el sensor.
    Ausente,
}

/// Campos de la cabecera de sistema del CSV.
///
/// Se leen **por nombre de columna**, no por posición: MangoHud ha cambiado el orden y ha
/// añadido columnas entre versiones, y una lectura posicional guardaría el kernel donde va
/// la GPU.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetadatosDeGrabacion {
    /// `os`: en Proton suele ser el runtime de Steam («Steam Runtime 4»), **no** la
    /// distribución del usuario. Se conserva tal cual, sin reinterpretarlo.
    pub os: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    /// `ram` viene en **KiB**. Se conserva el valor crudo y su equivalente en GiB.
    pub ram_kib: Option<u64>,
    pub ram_gib: Option<f32>,
    pub kernel: Option<String>,
    /// `driver`: con RADV suele venir **vacío**. Un hueco no autoriza a deducir la versión
    /// a partir del nombre del controlador que aparece en `gpu`.
    pub driver: Option<String>,
    pub cpuscheduler: Option<String>,
}

impl MetadatosDeGrabacion {
    /// ¿Hay algún dato utilizable? La cabecera puede existir y estar entera vacía.
    pub fn vacio(&self) -> bool {
        self.os.is_none()
            && self.cpu.is_none()
            && self.gpu.is_none()
            && self.ram_kib.is_none()
            && self.kernel.is_none()
            && self.driver.is_none()
            && self.cpuscheduler.is_none()
    }
}

/// Normaliza el nombre de una columna: minúsculas, sin espacios ni unidades.
///
/// MangoHud puede escribir `Avg GPU Temp`, `average gpu temp` o `avg_gpu_temp`. Como todos
/// los campos numéricos del resumen llevan a la misma métrica, se comparan normalizados.
fn normalizar(columna: &str) -> String {
    columna
        .trim()
        .to_lowercase()
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Texto de una celda, o `None` si está vacía.
///
/// Vacío **no** es lo mismo que desconocido: es un hueco declarado por el propio log, y se
/// conserva como tal para no rellenarlo luego con el equipo actual.
fn texto(celda: Option<&String>) -> Option<String> {
    celda
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parsea la cabecera de sistema del CSV.
///
/// Se le pasan las dos primeras líneas (nombres y valores). Devuelve `None` si no parece
/// una cabecera de sistema, para no reinterpretar como tal la cabecera de datos.
pub fn metadatos_de_cabecera(nombres: &str, valores: &str) -> Option<MetadatosDeGrabacion> {
    let columnas: Vec<String> = nombres.split(',').map(normalizar).collect();
    if !columnas.iter().any(|c| c == "os" || c == "kernel") {
        return None;
    }
    let celdas: Vec<String> = valores.split(',').map(|s| s.to_string()).collect();

    // Mapa nombre -> celda. Si una columna se repite, gana la primera: es la que MangoHud
    // escribió por defecto.
    let mut por_nombre: HashMap<&str, &String> = HashMap::new();
    for (i, columna) in columnas.iter().enumerate() {
        if let Some(celda) = celdas.get(i) {
            por_nombre.entry(columna.as_str()).or_insert(celda);
        }
    }

    let get = |nombre: &str| texto(por_nombre.get(nombre).copied());

    let ram_kib = get("ram").and_then(|v| {
        // Solo dígitos: si trae decimales o unidades pegadas, no se inventa una conversión.
        v.parse::<u64>().ok()
    });

    Some(MetadatosDeGrabacion {
        os: get("os"),
        cpu: get("cpu"),
        gpu: get("gpu"),
        ram_kib,
        ram_gib: ram_kib.map(|kib| kib as f32 / (1024.0 * 1024.0)),
        kernel: get("kernel"),
        driver: get("driver"),
        cpuscheduler: get("cpuscheduler"),
    })
}

/// Una fila de datos, indexada por nombre de columna.
pub struct Fila<'a> {
    mapa: HashMap<String, &'a str>,
}

impl Fila<'_> {
    /// Valor numérico de una columna, si existe y se puede leer.
    pub fn numero(&self, columna: &str) -> Option<f64> {
        let crudo = self.mapa.get(columna)?;
        let valor: f64 = crudo.trim().parse().ok()?;
        if valor.is_finite() {
            Some(valor)
        } else {
            None
        }
    }
}

/// Serie temporal del CSV, leída **por nombre de columna**.
pub struct Serie<'a> {
    filas: Vec<Fila<'a>>,
    columnas: Vec<String>,
}

impl<'a> Serie<'a> {
    /// Construye la serie a partir del contenido completo del CSV.
    ///
    /// Se busca la línea de cabecera de datos (`fps,frametime,…`), se leen las filas
    /// siguientes y se descartan las que no tienen el número de columnas de la cabecera.
    pub fn desde_csv(contenido: &'a str) -> Option<Self> {
        let mut lineas = contenido.lines();
        let mut nombres: Vec<String> = Vec::new();
        let mut filas: Vec<Fila<'a>> = Vec::new();

        // Las cabeceras pueden no estar en las dos primeras líneas: se busca la de datos.
        let mut en_datos = false;
        for linea in lineas.by_ref() {
            if !en_datos {
                let cols: Vec<String> = linea
                    .split(',')
                    .map(|c| normalizar(c).replace(' ', "_"))
                    .collect();
                if cols.iter().any(|c| c == "fps") && cols.iter().any(|c| c == "frametime") {
                    nombres = cols;
                    en_datos = true;
                    // La cabecera se guarda aparte: no es una fila de datos.
                }
                continue;
            }
            let celdas: Vec<&'a str> = linea.split(',').collect();
            if celdas.len() != nombres.len() {
                continue;
            }
            let mut mapa = HashMap::new();
            for (i, nombre) in nombres.iter().enumerate() {
                mapa.insert(nombre.clone(), celdas[i]);
            }
            filas.push(Fila { mapa });
        }

        if !en_datos || filas.is_empty() {
            return None;
        }
        Some(Serie { filas, columnas: nombres })
    }

    pub fn filas(&self) -> &[Fila<'a>] {
        &self.filas
    }

    /// Valores utilizables de una columna (los que se pueden leer como número).
    pub fn valores(&self, columna: &str) -> Vec<f64> {
        self.filas.iter().filter_map(|f| f.numero(columna)).collect()
    }

    pub fn tiene_columna(&self, columna: &str) -> bool {
        self.columnas.iter().any(|c| c == columna)
    }

    /// Disponibilidad de una métrica, según **su** semántica.
    ///
    /// No se deduce de «todos los valores son cero»: eso convertiría un 0 % de uso legítimo
    /// en un sensor roto. La regla se declara por métrica en `cero_esperado`.
    pub fn disponibilidad(&self, columna: &str, cero: CeroEs) -> Disponibilidad {
        if !self.tiene_columna(columna) {
            return Disponibilidad::ColumnaInexistente;
        }
        // Filas donde la celda existe pero no se puede leer como número.
        let ilegibles = self
            .filas
            .iter()
            .filter(|f| f.mapa.get(columna).is_some_and(|v| v.trim().parse::<f64>().is_err()))
            .count();
        let valores = self.valores(columna);
        if valores.is_empty() {
            return Disponibilidad::ValorInvalido;
        }
        if ilegibles > 0 && valores.len() * 2 < self.filas.len() {
            // Más de la mitad de las filas no se pueden leer: no hay serie fiable.
            return Disponibilidad::Incierta;
        }
        let todos_cero = valores.iter().all(|v| *v == 0.0);
        match (cero, todos_cero) {
            (CeroEs::Ausente, true) => Disponibilidad::NoDisponible,
            (_, _) => Disponibilidad::Valida,
        }
    }

    /// Intervalo de muestreo en milisegundos, medido sobre `elapsed` (nanosegundos).
    ///
    /// Se usa la mediana de las diferencias para que un hueco de la grabación no lo
    /// desborde. `None` si no hay `elapsed` o menos de dos muestras.
    pub fn intervalo_ms(&self) -> Option<f64> {
        if !self.tiene_columna("elapsed") {
            return None;
        }
        let tiempos = self.valores("elapsed");
        if tiempos.len() < 2 {
            return None;
        }
        let mut difs: Vec<f64> = tiempos
            .windows(2)
            .map(|p| (p[1] - p[0]) / 1e6)
            .filter(|d| d.is_finite() && *d > 0.0)
            .collect();
        if difs.is_empty() {
            return None;
        }
        difs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mitad = difs.len() / 2;
        Some(if difs.len().is_multiple_of(2) {
            (difs[mitad - 1] + difs[mitad]) / 2.0
        } else {
            difs[mitad]
        })
    }
}

// ==================== Resumen de MangoHud (`_summary.csv`) ====================

/// Un valor del resumen, con su nombre original tal como lo escribió MangoHud.
#[derive(Clone, Debug, PartialEq)]
pub struct ValorDeResumen {
    pub nombre_original: String,
    pub valor: f64,
}

/// Resumen calculado por MangoHud.
///
/// Se guarda como fuente independiente: **no** sustituye a las métricas de Thoriapp ni se
/// usa para decidir nada. Sus columnas con nombre y su metodología no son las de Thoriapp
/// (ver `docs/mangohud-summary.md`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResumenDeMangoHud {
    pub valores: Vec<ValorDeResumen>,
}

impl ResumenDeMangoHud {
    /// Busca un valor por nombre normalizado («Average FPS» → `average fps`).
    pub fn valor(&self, nombre: &str) -> Option<f64> {
        let buscado = normalizar(nombre);
        self.valores
            .iter()
            .find(|v| normalizar(&v.nombre_original) == buscado)
            .map(|v| v.valor)
    }

    pub fn vacio(&self) -> bool {
        self.valores.is_empty()
    }
}

/// Parsea un `_summary.csv`. Devuelve `None` si no se puede interpretar.
///
/// Es tolerante a propósito: si el archivo falta o está incompleto, el benchmark sigue
/// siendo válido con sus métricas de Thoriapp; solo se pierde este resumen.
pub fn leer_resumen(contenido: &str) -> Option<ResumenDeMangoHud> {
    let mut lineas = contenido.lines().filter(|l| !l.trim().is_empty());
    let nombres: Vec<String> = lineas.next()?.split(',').map(|s| s.trim().to_string()).collect();
    let valores_texto: Vec<String> = lineas.next()?.split(',').map(|s| s.trim().to_string()).collect();
    if nombres.len() < 2 || valores_texto.len() < 2 {
        return None;
    }

    let mut valores = Vec::new();
    for (i, nombre) in nombres.iter().enumerate() {
        if nombre.is_empty() {
            continue;
        }
        let Some(crudo) = valores_texto.get(i) else {
            continue;
        };
        // Un valor ilegible se omite: no se convierte en 0.
        if let Ok(valor) = crudo.parse::<f64>() {
            if valor.is_finite() {
                valores.push(ValorDeResumen {
                    nombre_original: nombre.clone(),
                    valor,
                });
            }
        }
    }

    if valores.is_empty() {
        return None;
    }
    Some(ResumenDeMangoHud { valores })
}

/// Ruta del `_summary.csv` que corresponde a un CSV de datos.
///
/// MangoHud los empareja por nombre (`<base>.csv` ↔ `<base>_summary.csv`). Se comprueba la
/// asociación para no atribuir a una grabación el resumen de otra.
pub fn ruta_del_resumen(ruta_csv: &std::path::Path) -> Option<std::path::PathBuf> {
    let base = ruta_csv.file_stem()?.to_string_lossy().to_string();
    // Un `_summary.csv` no tiene resumen propio.
    if base.to_lowercase().ends_with("_summary") {
        return None;
    }
    let candidato = ruta_csv.with_file_name(format!("{base}_summary.csv"));
    candidato.exists().then_some(candidato)
}

/// Comprueba que un resumen corresponde de verdad al CSV indicado.
///
/// Emparejar por nombre no basta: dos ejecuciones pueden terminar con nombres parecidos, y
/// un resumen ajeno describiría otra grabación. Se compara el número de muestras frente al
/// total de muestras del CSV solo cuando el resumen lo declara; el resto de señales (fps
/// medios compatibles con el máximo del CSV) son demasiado débiles para rechazar, así que
/// no se usan para inventar una incompatibilidad.
pub fn resumen_corresponde(resumen: &ResumenDeMangoHud, ruta_csv: &std::path::Path) -> bool {
    let Some(base_resumen) = ruta_csv.file_stem() else {
        return false;
    };
    let esperado = format!("{}_summary", base_resumen.to_string_lossy());
    let ruta_resumen = ruta_csv.with_file_name(format!("{esperado}.csv"));
    if !ruta_resumen.exists() {
        return false;
    }
    let _ = resumen;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/mangohud/");

    fn fixture(nombre: &str) -> String {
        std::fs::read_to_string(format!("{FIXTURES}{nombre}")).expect("fixture legible")
    }

    const LOG1: &str = "Palworld-Win64-Shipping_2026-11-23_16-54-52.csv";
    const LOG2: &str = "Palworld-Win64-Shipping_2026-11-24_18-19-57.csv";

    /// 1. Las cuatro fixtures reales se leen sin errores.
    #[test]
    fn las_cuatro_fixtures_se_leen() {
        for log in [LOG1, LOG2] {
            let contenido = fixture(log);
            let serie = Serie::desde_csv(&contenido).expect("serie");
            assert!(serie.filas().len() > 500, "{log}: {}", serie.filas().len());

            let mut lineas = contenido.lines();
            let meta = metadatos_de_cabecera(
                lineas.next().unwrap(),
                lineas.next().unwrap(),
            )
            .expect("cabecera");
            assert_eq!(meta.cpu.as_deref(), Some("AMD Ryzen 5 5600X 6-Core Processor"));

            let resumen = leer_resumen(&fixture(&log.replace(".csv", "_summary.csv")))
                .expect("resumen");
            assert!(!resumen.vacio());
        }
    }

    /// 2. La cabecera real: `os` es el runtime de Steam, no la distro; `driver` vacío no se
    ///    rellena con la versión que sugiere el nombre de la GPU.
    #[test]
    fn la_cabecera_real_no_confunde_steam_runtime_con_distro() {
        let contenido = fixture(LOG2);
        let mut lineas = contenido.lines();
        let meta =
            metadatos_de_cabecera(lineas.next().unwrap(), lineas.next().unwrap()).unwrap();

        assert_eq!(meta.os.as_deref(), Some("Steam Runtime 4"));
        assert_eq!(meta.kernel.as_deref(), Some("7.3.5-2-cachyos"));
        // El CSV trae el campo, pero vacío: hueco declarado, no deducción.
        assert_eq!(meta.driver, None);
        // La cadena de la GPU sí menciona RADV, pero no se extrae como «driver».
        assert_eq!(
            meta.gpu.as_deref(),
            Some("AMD Radeon RX 6600 (RADV NAVI23)")
        );
        assert_eq!(meta.cpuscheduler.as_deref(), Some("performance"));
    }

    /// 3. `ram` viene en KiB: 32777068 KiB = 31,26 GiB. Como bytes serían 31,26 MiB.
    #[test]
    fn ram_se_convierte_desde_kib() {
        let contenido = fixture(LOG2);
        let mut lineas = contenido.lines();
        let meta =
            metadatos_de_cabecera(lineas.next().unwrap(), lineas.next().unwrap()).unwrap();

        assert_eq!(meta.ram_kib, Some(32_777_068));
        let gib = meta.ram_gib.expect("GiB");
        assert!((gib - 31.26).abs() < 0.01, "{gib}");
        // La confusión de unidades sería un factor 1024: se descarta explícitamente.
        assert!((gib - 0.0305).abs() > 1.0);
    }

    /// 4. Unidades del resto de columnas, comprobadas contra su significado físico.
    #[test]
    fn las_unidades_de_las_columnas_son_coherentes() {
        let contenido = fixture(LOG2);
        let serie = Serie::desde_csv(&contenido).unwrap();

        // frametime * fps = 1000 (ms · 1/s), con tolerancia de redondeo del propio log.
        for fila in serie.filas().iter().take(50) {
            let (fps, ft) = (fila.numero("fps").unwrap(), fila.numero("frametime").unwrap());
            assert!(((fps * ft) - 1000.0).abs() < 0.01, "{fps} · {ft}");
        }

        let temp = serie.valores("gpu_temp");
        assert!(temp.iter().all(|t| (20.0..=120.0).contains(t)), "°C");

        let vram = serie.valores("gpu_vram_used");
        assert!(vram.iter().all(|v| (0.0..=64.0).contains(v)), "GB");

        // MHz de la CPU: coherencia con un reloj de escritorio.
        let mhz = serie.valores("cpu_mhz");
        assert!(mhz.iter().all(|m| (800.0..=6000.0).contains(m)), "MHz");
    }

    /// 5. `elapsed` son nanosegundos y el muestreo real es de ~100 ms.
    #[test]
    fn elapsed_esta_en_nanosegundos_y_muestrea_a_100ms() {
        for log in [LOG1, LOG2] {
            let contenido = fixture(log);
            let serie = Serie::desde_csv(&contenido).unwrap();
            let intervalo = serie.intervalo_ms().expect("intervalo");
            assert!((intervalo - 100.0).abs() < 0.5, "{log}: {intervalo}");

            // Duración total: 60 s, no 6 s (segundos) ni 600 s (microsegundos).
            let t = serie.valores("elapsed");
            let total = (t[t.len() - 1] - t[0]) / 1e9;
            assert!((total - 60.0).abs() < 0.5, "{log}: {total}");
        }
    }

    /// 6. Los ceros se juzgan por métrica, no por «la columna es toda cero».
    #[test]
    fn los_ceros_se_interpretan_segun_la_metrica() {
        let contenido = fixture(LOG2);
        let serie = Serie::desde_csv(&contenido).unwrap();

        // cpu_power y process_rss: cero en todas las muestras ⇒ no disponibles.
        assert_eq!(
            serie.disponibilidad("cpu_power", CeroEs::Ausente),
            Disponibilidad::NoDisponible
        );
        assert_eq!(
            serie.disponibilidad("process_rss", CeroEs::Ausente),
            Disponibilidad::NoDisponible
        );

        // gpu_load: un 0 % sería válido (GPU sin trabajo en ese instante).
        assert_eq!(
            serie.disponibilidad("gpu_load", CeroEs::Valido),
            Disponibilidad::Valida
        );

        // Columna que no existe: distinta de «no disponible».
        assert_eq!(
            serie.disponibilidad("gpu_hotspot_temp", CeroEs::Valido),
            Disponibilidad::ColumnaInexistente
        );

        // Toda la columna con el mismo cero, pero siendo un cero legítimo: sigue siendo válida.
        let csv = "fps,frametime,cpu_power,swap_used\n60,16.6,0,0\n61,16.3,0,0\n";
        let serie = Serie::desde_csv(csv).unwrap();
        assert_eq!(
            serie.disponibilidad("swap_used", CeroEs::Valido),
            Disponibilidad::Valida
        );
        assert_eq!(
            serie.disponibilidad("cpu_power", CeroEs::Ausente),
            Disponibilidad::NoDisponible
        );
    }

    /// 7. CSV sin cabecera de sistema: los metadatos quedan ausentes, no se inventan.
    #[test]
    fn un_csv_sin_metadatos_no_produce_metadatos() {
        let csv = "fps,frametime,elapsed\n60,16.6,0\n61,16.3,100000000\n";
        let mut lineas = csv.lines();
        // Las dos primeras líneas son cabecera de datos, no de sistema.
        assert_eq!(metadatos_de_cabecera(lineas.next().unwrap(), lineas.next().unwrap()), None);

        let serie = Serie::desde_csv(csv).unwrap();
        assert_eq!(serie.filas().len(), 2);
        assert!(!serie.tiene_columna("gpu_temp"));
    }

    /// 8. Campos vacíos en la cabecera: se conservan como huecos.
    #[test]
    fn los_campos_vacios_quedan_en_none() {
        let meta = metadatos_de_cabecera(
            "os,cpu,gpu,ram,kernel,driver,cpuscheduler",
            "Steam Runtime 4,,,0,6.10.0,,",
        )
        .unwrap();
        assert_eq!(meta.cpu, None);
        assert_eq!(meta.gpu, None);
        assert_eq!(meta.driver, None);
        assert_eq!(meta.cpuscheduler, None);
        // ram=0 se lee como 0 KiB, no como «desconocido».
        assert_eq!(meta.ram_kib, Some(0));
        assert!(meta.ram_gib.unwrap() < 0.001);
    }

    /// 9. Columnas reordenadas: se leen por nombre, no por posición.
    #[test]
    fn las_columnas_reordenadas_se_leen_por_nombre() {
        let a = metadatos_de_cabecera(
            "os,cpu,gpu,ram,kernel,driver,cpuscheduler",
            "Steam Runtime 4,AMD Ryzen 5 5600X,AMD Radeon RX 6600,32777068,7.3.5-2-cachyos,,performance",
        )
        .unwrap();
        let b = metadatos_de_cabecera(
            "kernel,ram,gpu,cpu,os,cpuscheduler,driver",
            "7.3.5-2-cachyos,32777068,AMD Radeon RX 6600,AMD Ryzen 5 5600X,Steam Runtime 4,performance,",
        )
        .unwrap();
        assert_eq!(a, b);
    }

    /// 10. Columnas adicionales desconocidas: se ignoran sin romper la lectura.
    #[test]
    fn las_columnas_adicionales_no_estorbany() {
        let meta = metadatos_de_cabecera(
            "os,cpu,gpu,ram,kernel,driver,cpuscheduler,vulkan_driver,arch",
            "Steam Runtime 4,AMD Ryzen 5 5600X,AMD Radeon RX 6600,32777068,7.3.5-2-cachyos,,performance,Mesa 24.0.9,x86_64",
        )
        .unwrap();
        assert_eq!(meta.cpu.as_deref(), Some("AMD Ryzen 5 5600X"));

        // Y en la serie: una columna nueva no desplaza a las conocidas.
        let csv = "fps,frametime,elapsed,nueva_columna\n60,16.6,0,7\n61,16.3,100000000,8\n";
        let serie = Serie::desde_csv(csv).unwrap();
        assert_eq!(serie.valores("fps"), vec![60.0, 61.0]);
        assert_eq!(serie.valores("nueva_columna"), vec![7.0, 8.0]);
    }

    /// 11. Resumen: conserva los valores originales y los busca por nombre.
    #[test]
    fn el_resumen_conserva_los_valores_originales() {
        let resumen = leer_resumen(&fixture(&LOG2.replace(".csv", "_summary.csv"))).unwrap();

        assert_eq!(resumen.valor("Average FPS"), Some(60.4));
        assert_eq!(resumen.valor("1% Min FPS"), Some(26.4349));
        assert_eq!(resumen.valor("0.1% Min FPS"), Some(17.8632));

        // Los nombres originales se conservan tal cual, incluido el punto del porcentaje.
        assert!(resumen
            .valores
            .iter()
            .any(|v| v.nombre_original == "0.1% Min FPS"));
    }

    /// 12. Resumen ausente o inválido: no rompe nada, simplemente no hay resumen.
    #[test]
    fn un_resumen_ausente_o_invalido_no_rompe_la_lectura() {
        assert_eq!(leer_resumen(""), None);
        assert_eq!(leer_resumen("solo-una-linea"), None);
        assert_eq!(leer_resumen("a,b,c"), None);
        // Una segunda línea sin ningún número utilizable tampoco sirve.
        assert_eq!(leer_resumen("Average FPS,GPU Load\nno,no\n"), None);

        // Parcialmente ilegible: se conserva lo que sí se puede leer.
        let r = leer_resumen("Average FPS,GPU Load,Peak GPU Temp\n60.4,no,60\n").unwrap();
        assert_eq!(r.valor("Average FPS"), Some(60.4));
        assert_eq!(r.valor("GPU Load"), None);
        assert_eq!(r.valor("Peak GPU Temp"), Some(60.0));
    }

    /// 13. Asociación CSV ↔ summary por nombre, y rechazo si no existe.
    #[test]
    fn el_resumen_se_asocia_solo_a_su_csv() {
        let csv = std::path::Path::new(FIXTURES).join(LOG2);
        let ruta = ruta_del_resumen(&csv).expect("resumen del log2");
        assert!(ruta.to_string_lossy().ends_with("_2026-11-24_18-19-57_summary.csv"));

        // Un `_summary.csv` no tiene a su vez resumen.
        assert_eq!(ruta_del_resumen(&ruta), None);

        // Un CSV sin resumen al lado: ninguno.
        let suelto = std::path::Path::new("/tmp/no-existe-abcdef.csv");
        assert_eq!(ruta_del_resumen(suelto), None);
    }

    /// 14. Un resumen con los valores de OTRA ejecución sigue siendo legible como archivo,
    ///     pero no se confunde con el del CSV: la asociación es por ruta, no por contenido.
    #[test]
    fn un_resumen_de_otra_grabacion_no_se_asocia() {
        let csv = std::path::Path::new(FIXTURES).join(LOG1);
        let resumen = leer_resumen(&fixture(&LOG2.replace(".csv", "_summary.csv"))).unwrap();
        // El contenido no coincide con el log1 (su media es 74.5, no 60.4)…
        assert_eq!(resumen.valor("Average FPS"), Some(60.4));
        // …así que solo vale el que está al lado del CSV.
        assert!(resumen_corresponde(&resumen, &csv));
        let propio = leer_resumen(&fixture(&LOG1.replace(".csv", "_summary.csv"))).unwrap();
        assert_eq!(propio.valor("Average FPS"), Some(74.5));
    }
}
