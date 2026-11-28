//! Pruebas de regresión de **C3-20**: el viaje de un `f64` por JSON tiene que devolver
//! **el mismo número**, bit a bit.
//!
//! ## El defecto que se fija aquí
//!
//! `serde_json` sin la feature `float_roundtrip` deserializaba algunos decimales al `f64`
//! **contiguo** en vez de al más cercano. El caso real que lo destapó fue el escenario
//! `SIN_CAMBIO`:
//!
//! ```text
//! Rust calcula   frame_time_change_percent = -1.4778325123152847   (0xbff7a533b455c130)
//! to_string      "-1.4778325123152847"                             correcto
//! from_str       0xbff7a533b455c131                                1 ULP por encima
//! ```
//!
//! La vista previa viajaba bien hasta el frontend (Node lo parsea bien), pero al volver a
//! Rust el número cambiaba. La confirmación recalculada coincidía con la original y la
//! recibida no, así que el guardado se rechazaba con `EntradasCambiadas` — el mensaje que
//! el usuario veía era «La vista previa ya no corresponde a los datos actuales».
//!
//! La corrección **no** toca la comparación: `mismos_f64` sigue exigiendo bits idénticos.
//! Lo que se arregla es que el número llegue intacto.
//!
//! ## Por qué no lo detectaban las pruebas anteriores
//!
//! `c2_flujo_completo_vista_previa_a_persistencia` sí hacía el viaje por JSON, pero con
//! `avg_fps` de `1.0`, `2.0` y `112.5`, cuyas representaciones vuelven exactas. Faltaba un
//! valor en la frontera de redondeo. Estos valores **no son exclusivos de los datos
//! sintéticos**: cualquier cociente de medidas reales puede caer en la misma zona (por
//! ejemplo, `100 / 10.1513…`), que es justo lo que produce un frame time medio.

use super::*;

/// Valores `f64` que la deserialización ingenua devolvía desplazados 1 ULP.
///
/// Se fijan por sus bits: si alguien cambia la constante decimal a mano, la prueba deja de
/// hablar del caso que motivó el defecto.
const VALORES_FRONTERA: &[(&str, u64)] = &[
    // El que destapó C3-20: `pct_change(10.0, 9.852216748768473)` en `SIN_CAMBIO`.
    ("-1.4778325123152847", 0xbff7a533b455c130),
    // El cociente que lo origina: 100 / 10.1513…, un frame time medio real.
    ("9.852216748768473", 0x4023b455c0f220c9),
    // Vecinos inmediatos: si el parser se equivoca por 1 ULP, alguno cae.
    ("-1.4778325123152845", 0xbff7a533b455c12f),
    ("-1.477832512315285", 0xbff7a533b455c131),
    // Cambios negativos y positivos con cola larga, como los de otros escenarios.
    ("-16.66666666666666", 0xc030aaaaaaaaaaa9),
    ("26.315789473684188", 0x403a50d79435e507),
    // Casos triviales que ya funcionaban: la prueba no debe romperlos.
    ("-20.0", 0xc034000000000000),
    ("0.0", 0x0000000000000000),
];

// ─────────────────── 1. Round-trip exacto de f64 problemáticos ───────────────────

/// El viaje `f64 -> texto -> f64` conserva **los bits** en todos los valores de frontera.
///
/// Sin `float_roundtrip` esta prueba falla en `-1.4778325123152847` (y en sus vecinos);
/// con la feature pasa. Es la prueba que mide la corrección en sí.
#[test]
fn c3_20_el_round_trip_de_f64_conserva_los_bits() {
    for (texto, bits) in VALORES_FRONTERA {
        let valor = f64::from_bits(*bits);
        let escrito = serde_json::to_string(&valor).expect("serializar el f64");
        let vuelta: f64 = serde_json::from_str(&escrito).expect("deserializar el f64");

        assert_eq!(
            vuelta.to_bits(),
            *bits,
            "{texto} (escrito como {escrito}) volvió con bits 0x{:016x} en vez de 0x{bits:016x}",
            vuelta.to_bits()
        );
    }
}

/// El viaje que realmente hace el IPC: el valor **dentro** de un objeto, pasando por
/// `serde_json::Value`, que es como Tauri lo entrega.
///
/// Comprueba que `Value` no introduce una representación distinta de la de `to_string`.
#[test]
fn c3_20_el_round_trip_por_value_tambien_conserva_los_bits() {
    for (texto, bits) in VALORES_FRONTERA {
        let valor = f64::from_bits(*bits);
        let como_valor = serde_json::to_value(valor).expect("a Value");
        let escrito = serde_json::to_string(&como_valor).expect("Value a texto");
        let vuelta: f64 = serde_json::from_str(&escrito).expect("texto a f64");

        assert_eq!(
            vuelta.to_bits(),
            *bits,
            "{texto} por Value volvió con bits 0x{:016x} en vez de 0x{bits:016x}",
            vuelta.to_bits()
        );
    }
}

/// La comparación estricta **sigue siendo estricta**: dos `f64` que difieren 1 ULP son
/// distintos. La corrección arregla la deserialización, no la confirmación.
#[test]
fn c3_20_la_comparacion_por_bits_sigue_siendo_estricta() {
    let a = f64::from_bits(0xbff7a533b455c130);
    let b = f64::from_bits(0xbff7a533b455c131);
    assert!(a != b, "dos valores a 1 ULP tienen que ser distintos");
    assert!(!mismos_f64(a, b), "`mismos_f64` no puede tolerarlos");
    assert!(mismos_f64(a, a), "y un valor sigue siendo igual a sí mismo");
}

// ────────── 4. Protección contra la eliminación futura de float_roundtrip ──────────

/// Guardia de la feature: se lee la declaración de `Cargo.toml` y se exige
/// `float_roundtrip` en `serde_json`.
///
/// La prueba de bits de arriba ya fallaría si se quitara, pero este fallo dice **por qué**
/// y dónde, en vez de dejar un `0xbff7…131 != 0xbff7…130` sin contexto. Es la mitad de la
/// protección; la otra mitad es la prueba de bits.
#[test]
fn c3_20_la_feature_float_roundtrip_sigue_declarada() {
    let manifest = include_str!("../../Cargo.toml");
    let linea = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("serde_json"))
        .expect("Cargo.toml tiene que declarar serde_json");

    assert!(
        linea.contains("float_roundtrip"),
        "serde_json perdió la feature `float_roundtrip`: sin ella, algunos f64 vuelven \
         desplazados 1 ULP del JSON y el guardado de la vista previa se rechaza. \
         Línea actual: {linea}"
    );
    assert!(
        linea.contains("1.0"),
        "la versión de serde_json no debe cambiar en esta corrección: {linea}"
    );
}

// ──── 3. La corrección no debilita la confirmación (garantías de C.2) ────

/// Compone la confirmación completa tal como la produciría Rust.
fn confirmacion_completa(c: &crate::benchmarks::BenchmarkComparison) -> ConfirmacionDeVistaPrevia {
    let cond = condiciones_de(c, None);
    ConfirmacionDeVistaPrevia {
        comparacion: c.clone(),
        evaluacion: evaluar(c, &cond),
    }
}

/// Una diferencia **real** de 1 ULP en la confirmación recibida se sigue rechazando.
///
/// Este es el contrapeso de la corrección: el parser ahora devuelve el número exacto, así
/// que cualquier diferencia que **quede** es una manipulación de verdad. Si alguien
/// introdujera una tolerancia para «arreglar» C3-20, esta prueba se pondría en rojo.
#[test]
fn c3_20_una_diferencia_real_de_un_ulp_se_sigue_rechazando() {
    let base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    let mut test = crate::evaluacion::pruebas::bench_para_c2("b", 101.5, 20000);
    test.frame_time_avg = 9.852216748768473;

    let comparacion = crate::evaluacion::pruebas::comparacion_para_c2(base, test);
    let confirmada = confirmacion_completa(&comparacion);

    // La confirmación manipulada difiere en el último bit de un campo de la comparación.
    let mut manipulada = confirmada.clone();
    manipulada.comparacion.frame_time_change_percent =
        f64::from_bits(confirmada.comparacion.frame_time_change_percent.to_bits() + 1);

    assert_ne!(
        manipulada.comparacion.frame_time_change_percent.to_bits(),
        confirmada.comparacion.frame_time_change_percent.to_bits(),
        "el montaje tiene que diferir de verdad"
    );

    assert_eq!(
        comprobar_confirmacion(&manipulada, &confirmada),
        Err(MotivoDeRechazo::EntradasCambiadas),
        "1 ULP introducido a mano no puede colarse"
    );
}

/// Una evaluación alterada se sigue rechazando.
#[test]
fn c3_20_una_evaluacion_alterada_se_sigue_rechazando() {
    let base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    let test = crate::evaluacion::pruebas::bench_para_c2("b", 101.5, 20000);
    let comparacion = crate::evaluacion::pruebas::comparacion_para_c2(base, test);
    let confirmada = confirmacion_completa(&comparacion);

    let mut manipulada = confirmada.clone();
    manipulada.evaluacion.estado = crate::benchmarks::CodigoDeEvaluacion("mejora".to_string());

    assert_eq!(
        comprobar_confirmacion(&manipulada, &confirmada),
        Err(MotivoDeRechazo::SignificadoCambiado),
        "cambiar el estado de la evaluación no puede aceptarse"
    );
}

/// Una entrada realmente modificada se sigue rechazando.
#[test]
fn c3_20_una_entrada_modificada_se_sigue_rechazando() {
    let base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    let test = crate::evaluacion::pruebas::bench_para_c2("b", 101.5, 20000);
    let comparacion = crate::evaluacion::pruebas::comparacion_para_c2(base, test);
    let confirmada = confirmacion_completa(&comparacion);

    let mut manipulada = confirmada.clone();
    manipulada.comparacion.test.avg_fps = 999.0;

    assert_eq!(
        comprobar_confirmacion(&manipulada, &confirmada),
        Err(MotivoDeRechazo::EntradasCambiadas),
        "cambiar una medida no puede aceptarse"
    );
}

/// Un valor no persistible se sigue rechazando antes de comparar.
#[test]
fn c3_20_un_valor_no_persistible_se_sigue_rechazando() {
    let base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    let test = crate::evaluacion::pruebas::bench_para_c2("b", 101.5, 20000);
    let comparacion = crate::evaluacion::pruebas::comparacion_para_c2(base, test);
    let confirmada = confirmacion_completa(&comparacion);

    let mut manipulada = confirmada.clone();
    manipulada.comparacion.test.avg_fps = f64::NAN;

    assert_eq!(
        comprobar_confirmacion(&manipulada, &confirmada),
        Err(MotivoDeRechazo::ValorNoPersistible),
        "NaN no puede aceptarse"
    );
}

/// Un valor de frontera viajado por JSON **no** se confunde con una manipulación: se acepta.
///
/// Es el caso de C3-20 visto desde la confirmación: el número vuelve intacto, así que la
/// confirmación coincide. Si el parser volviera a desplazarlo 1 ULP, esto fallaría.
#[test]
fn c3_20_el_valor_de_frontera_viaja_y_se_acepta() {
    let base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    let mut test = crate::evaluacion::pruebas::bench_para_c2("b", 101.5, 20000);
    test.frame_time_avg = 9.852216748768473;

    let comparacion = crate::evaluacion::pruebas::comparacion_para_c2(base, test);
    let confirmada = confirmacion_completa(&comparacion);

    let json = serde_json::to_string(&confirmada).expect("serializar");
    let devuelta: ConfirmacionDeVistaPrevia = serde_json::from_str(&json).expect("deserializar");

    assert_eq!(
        devuelta.comparacion.frame_time_change_percent.to_bits(),
        confirmada.comparacion.frame_time_change_percent.to_bits(),
        "el campo de frontera cambió al cruzar JSON"
    );
    assert_eq!(
        comprobar_confirmacion(&devuelta, &confirmada),
        Ok(()),
        "una vista previa sin manipular tiene que aceptarse tras el viaje"
    );
}
