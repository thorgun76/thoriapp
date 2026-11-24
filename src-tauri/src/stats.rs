//! Estadística de la comparación entre configuraciones.
//!
//! Comparar **una grabación contra otra** no distingue una mejora real de la varianza
//! del propio juego: si dos partidas del mismo juego en la misma configuración difieren
//! un 6 %, decir «mejoró un 6 %» es una lotería. Aquí se compara **un grupo de
//! grabaciones** (la condición «antes») contra otro («después») y se responde con
//! tres estados, no con dos:
//!
//! - `mejora` / `empeora`: la diferencia supera el ruido estimado de verdad.
//! - `no_concluyente`: no hay datos suficientes o la diferencia cabe dentro de la
//!   variabilidad medida. Decirlo es más útil que inventar un ganador.
//!
//! Se usa un **bootstrap de la diferencia de medianas** (5000 remuestreos, semilla
//! fija). Motivos: con 3-10 grabaciones por lado las medidas no son normales (una
//! sesión con un tirón gordo mueve la media), la mediana es robusta, y el bootstrap no
//! exige suponer ninguna distribución. La semilla fija hace que el mismo par de grupos
//! dé siempre el mismo intervalo: un resultado que cambia al recargar la pantalla no
//! es un resultado.
//!
//! No se afirma nada con menos de [`MIN_RUNS`] grabaciones por lado: con una o dos,
//! lo honesto es decir «repite» en vez de calcular un intervalo que parecería rigor.

use serde::{Deserialize, Serialize};

/// Grabaciones mínimas por lado para poder concluir algo.
pub const MIN_RUNS: usize = 3;

/// Remuestreos del bootstrap.
const BOOTSTRAP_ITERATIONS: usize = 5000;

/// Semilla fija: el mismo par de grupos da siempre el mismo intervalo.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Error de estimación de los percentiles bajos (1 % y 0,1 %) con pocas grabaciones.
///
/// El FPS promedio de una grabación es una estimación bastante estable; el 1 % low
/// depende de un puñado de frames y cambia mucho más entre partidas. Exigirle a los
/// percentiles el mismo margen relativo que al promedio daría «mejoras» fantasma, así
/// que su umbral mínimo es mayor.
pub const PERCENTILE_NOISE_FLOOR_PERCENT: f64 = 8.0;

/// Qué se está midiendo en la comparación.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    AvgFps,
    Fps1PercentLow,
    Fps01PercentLow,
}

impl Metric {
    pub fn label(self) -> &'static str {
        match self {
            Metric::AvgFps => "FPS promedio",
            Metric::Fps1PercentLow => "1 % de los peores frames",
            Metric::Fps01PercentLow => "0,1 % de los peores frames",
        }
    }

    /// Margen mínimo (en %) que se exige a esta métrica aunque el bootstrap diga que
    /// el ruido es menor. Los percentiles son más inestables que el promedio.
    pub fn floor_percent(self) -> f64 {
        match self {
            Metric::AvgFps => 3.0,
            Metric::Fps1PercentLow | Metric::Fps01PercentLow => PERCENTILE_NOISE_FLOOR_PERCENT,
        }
    }
}

/// Resultado de comparar dos grupos de grabaciones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupComparison {
    pub metric: Metric,
    /// Grabaciones válidas en cada grupo (más o menos de las que se pasaron si alguna
    /// tenía el dato vacío).
    pub antes_runs: usize,
    pub despues_runs: usize,
    /// Medianas de cada grupo (más robustas que la media con pocas muestras).
    pub antes_median: f64,
    pub despues_median: f64,
    /// Cambio porcentual de la mediana.
    pub change_percent: f64,
    /// Intervalo de confianza del 95 % del cambio (en %).
    pub ci_low_percent: f64,
    pub ci_high_percent: f64,
    /// Margen mínimo exigido (%), antes de cruzarlo con el intervalo.
    pub required_margin_percent: f64,
    /// `mejora`, `empeora` o `no_concluyente`.
    pub verdict: String,
    /// Frase lista para la interfaz.
    pub conclusion: String,
}

/// RNG determinista (xorshift64*). No hace falta un generador de calidad
/// criptográfica: solo remuestrear índices, y así los tests son reproducibles.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Índice uniforme en `[0, n)`.
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Mediana. Con un número par de muestras, media de las dos centrales.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    Some(if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    })
}

/// Cambio porcentual de `b` respecto a `a`. `0` si `a` es 0 (dato inválido) en vez de
/// infinito o NaN, que romperían la interfaz.
fn pct_change(a: f64, b: f64) -> f64 {
    if a.abs() < f64::EPSILON {
        0.0
    } else {
        (b - a) / a * 100.0
    }
}

/// Percentil por interpolación lineal (`p` en 0..=100). `None` si no hay muestras.
fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        Some(sorted[lo])
    } else {
        let frac = idx - lo as f64;
        Some(sorted[lo] * (1.0 - frac) + sorted[hi] * frac)
    }
}

/// Compara dos grupos de grabaciones («antes» y «después») para una métrica.
///
/// `antes` y `despues` son los valores de cada grabación (FPS). Se descartan los que
/// no son números positivos: una grabación sin ese dato no es un cero, es un hueco.
pub fn compare_groups(antes: &[f64], despues: &[f64], metric: Metric) -> GroupComparison {
    let a: Vec<f64> = antes
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();
    let d: Vec<f64> = despues
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();

    let med_a = median(&a).unwrap_or(0.0);
    let med_d = median(&d).unwrap_or(0.0);
    let change = pct_change(med_a, med_d);
    let floor = metric.floor_percent();

    // Sin grabaciones suficientes no se calcula intervalo: se dice que faltan, que es
    // la información útil. Calcularlo con 2+2 daría un ±20 % que asusta sin motivo.
    if a.len() < MIN_RUNS || d.len() < MIN_RUNS {
        let faltan = MIN_RUNS.saturating_sub(a.len().min(d.len()));
        return GroupComparison {
            metric,
            antes_runs: a.len(),
            despues_runs: d.len(),
            antes_median: med_a,
            despues_median: med_d,
            change_percent: change,
            ci_low_percent: 0.0,
            ci_high_percent: 0.0,
            required_margin_percent: floor,
            verdict: "no_concluyente".to_string(),
            conclusion: format!(
                "Faltan grabaciones para concluir: {} por lado como mínimo y te falta{}. \
                 Con menos, cualquier diferencia puede ser casualidad.",
                MIN_RUNS,
                if faltan == 1 {
                    " 1".to_string()
                } else {
                    format!("n {faltan}")
                }
            ),
        };
    }

    // Bootstrap de la diferencia de medianas.
    let mut rng = Rng(SEED);
    let mut diffs: Vec<f64> = Vec::with_capacity(BOOTSTRAP_ITERATIONS);
    let mut sample_a: Vec<f64> = Vec::with_capacity(a.len());
    let mut sample_d: Vec<f64> = Vec::with_capacity(d.len());

    for _ in 0..BOOTSTRAP_ITERATIONS {
        sample_a.clear();
        sample_d.clear();
        for _ in 0..a.len() {
            sample_a.push(a[rng.below(a.len())]);
        }
        for _ in 0..d.len() {
            sample_d.push(d[rng.below(d.len())]);
        }
        let ma = median(&sample_a).unwrap_or(0.0);
        let md = median(&sample_d).unwrap_or(0.0);
        diffs.push(pct_change(ma, md));
    }

    let ci_low = percentile(&diffs, 2.5).unwrap_or(change);
    let ci_high = percentile(&diffs, 97.5).unwrap_or(change);

    // Se concluye solo si TODO el intervalo queda fuera del margen exigido. Es la
    // condición conservadora: si el intervalo cruza el 0 o roza el margen, no se
    // afirma nada. Un intervalo entero por encima de `floor` es una señal mucho más
    // sólida que comparar solo los puntos medios.
    let margin = floor.max(0.0);
    let verdict = if ci_low > margin {
        "mejora"
    } else if ci_high < -margin {
        "empeora"
    } else {
        "no_concluyente"
    };

    let conclusion = match verdict {
        "mejora" => format!(
            "Mejora en {}: {:+.1} % (intervalo del 95 %: {:+.1} % a {:+.1} %, {} grabaciones por lado).",
            metric.label(),
            change,
            ci_low,
            ci_high,
            a.len().min(d.len())
        ),
        "empeora" => format!(
            "Empeora en {}: {:+.1} % (intervalo del 95 %: {:+.1} % a {:+.1} %, {} grabaciones por lado).",
            metric.label(),
            change,
            ci_low,
            ci_high,
            a.len().min(d.len())
        ),
        _ => format!(
            "No concluyente en {}: el cambio medido es {:+.1} % y el intervalo del 95 % \
             ({:+.1} % a {:+.1} %) incluye diferencias de hasta ±{:.0} %. Repite la prueba \
             para separar el efecto del ruido.",
            metric.label(),
            change,
            ci_low,
            ci_high,
            margin
        ),
    };

    GroupComparison {
        metric,
        antes_runs: a.len(),
        despues_runs: d.len(),
        antes_median: med_a,
        despues_median: med_d,
        change_percent: change,
        ci_low_percent: ci_low,
        ci_high_percent: ci_high,
        required_margin_percent: margin,
        verdict: verdict.to_string(),
        conclusion,
    }
}

// ==================== TESTS ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_mediana_aguanta_un_valor_extremo() {
        // Este es el motivo de usar mediana y no media: una sesión con un tirón
        // brutal mueve la media varios FPS y la mediana casi nada.
        let con_pico = [60.0, 61.0, 59.0, 60.5, 20.0];
        let media = con_pico.iter().sum::<f64>() / con_pico.len() as f64;
        // Ordenadas: 20, 59, 60, 60.5, 61 → la del centro es 60.
        assert_eq!(median(&con_pico), Some(60.0));
        assert!(media < 53.0, "la media sí se hunde: {media}");
    }

    #[test]
    fn la_mediana_de_un_numero_par_promedia_las_centrales() {
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn percentiles_por_interpolacion() {
        let v = [10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(percentile(&v, 0.0), Some(10.0));
        assert_eq!(percentile(&v, 100.0), Some(50.0));
        assert_eq!(percentile(&v, 50.0), Some(30.0));
        assert_eq!(percentile(&v, 25.0), Some(20.0));
    }

    #[test]
    fn con_pocas_grabaciones_no_se_concluye_nada() {
        let r = compare_groups(&[60.0, 61.0], &[90.0, 91.0, 92.0], Metric::AvgFps);
        assert_eq!(r.verdict, "no_concluyente");
        assert!(r.conclusion.contains("Faltan grabaciones"), "{}", r.conclusion);
        // El cambio se informa igual, pero sin intervalo: no se finge precisión.
        assert!(r.change_percent > 40.0);
    }

    #[test]
    fn una_mejora_clara_y_consistente_se_detecta() {
        let antes = [58.0, 59.0, 60.0, 59.5, 60.5];
        let despues = [70.0, 71.0, 69.5, 70.5, 70.0];
        let r = compare_groups(&antes, &despues, Metric::AvgFps);
        assert_eq!(r.verdict, "mejora", "{}", r.conclusion);
        assert!(r.ci_low_percent > 3.0);
        assert!(r.change_percent > 15.0);
    }

    #[test]
    fn un_empeoramiento_claro_se_detecta() {
        let antes = [70.0, 71.0, 69.5, 70.5, 70.0];
        let despues = [58.0, 59.0, 60.0, 59.5, 60.5];
        let r = compare_groups(&antes, &despues, Metric::AvgFps);
        assert_eq!(r.verdict, "empeora", "{}", r.conclusion);
        assert!(r.ci_high_percent < -3.0);
    }

    #[test]
    fn diferencias_pequenas_dentro_del_ruido_son_no_concluyentes() {
        // Dos grupos que se solapan: la mediana cambia, pero no se puede afirmar nada.
        let antes = [55.0, 60.0, 65.0, 58.0, 62.0];
        let despues = [57.0, 61.0, 63.0, 59.0, 66.0];
        let r = compare_groups(&antes, &despues, Metric::AvgFps);
        assert_eq!(r.verdict, "no_concluyente", "{}", r.conclusion);
        assert!(r.ci_low_percent < 0.0 && r.ci_high_percent > 0.0);
    }

    #[test]
    fn los_percentiles_exigen_mas_margen_que_el_promedio() {
        assert!(
            Metric::Fps1PercentLow.floor_percent() > Metric::AvgFps.floor_percent(),
            "el 1 % low es más ruidoso que el promedio y no puede tener el mismo listón"
        );
        assert_eq!(
            Metric::Fps01PercentLow.floor_percent(),
            PERCENTILE_NOISE_FLOOR_PERCENT
        );
    }

    #[test]
    fn una_mejora_pequena_del_percentil_no_se_afirma_con_el_liston_del_promedio() {
        // +5 % en el 1 % low: por encima del listón del promedio (3 %), por debajo del
        // suyo (8 %). No se puede afirmar.
        let antes = [48.0, 49.0, 50.0, 49.5, 48.5];
        let despues = [51.0, 51.5, 52.0, 52.5, 51.2];
        let r = compare_groups(&antes, &despues, Metric::Fps1PercentLow);
        assert_eq!(r.verdict, "no_concluyente", "{}", r.conclusion);
    }

    #[test]
    fn el_resultado_es_reproducible() {
        // Misma semilla: dos llamadas idénticas dan el mismo intervalo. Si esto
        // cambia, la pantalla mostraría cifras distintas al recargar.
        let antes = [55.0, 60.0, 65.0, 58.0, 62.0, 59.0];
        let despues = [57.0, 61.0, 63.0, 59.0, 66.0, 60.0];
        let uno = compare_groups(&antes, &despues, Metric::AvgFps);
        let dos = compare_groups(&antes, &despues, Metric::AvgFps);
        assert_eq!(uno.ci_low_percent, dos.ci_low_percent);
        assert_eq!(uno.ci_high_percent, dos.ci_high_percent);
        assert_eq!(uno.verdict, dos.verdict);
    }

    #[test]
    fn descarta_los_valores_invalidos_en_vez_de_contarlos_como_cero() {
        let antes = [60.0, 61.0, f64::NAN, 0.0, 60.5];
        let r = compare_groups(&antes, &[70.0, 71.0, 70.5], Metric::AvgFps);
        assert_eq!(r.antes_runs, 3, "NaN y 0 no son grabaciones con 0 FPS");
        assert_eq!(r.despues_runs, 3);
    }

    #[test]
    fn un_grupo_vacio_no_divide_por_cero() {
        let r = compare_groups(&[], &[60.0, 61.0, 62.0], Metric::AvgFps);
        assert_eq!(r.verdict, "no_concluyente");
        assert!(r.change_percent.is_finite());
    }
}
