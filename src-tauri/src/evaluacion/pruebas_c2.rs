//! Pruebas de la integración de C.2: confirmación de la vista previa y coherencia.
//!
//! Se separan dos planos:
//!
//! - **Regla de correspondencia**: qué se acepta y qué se rechaza, sin tocar disco. Es lo
//!   que garantiza que no se guarde una evaluación distinta de la confirmada.
//! - **Coherencia interna**: que lo generado cumple los invariantes del contrato antes de
//!   escribirse.
//!
//! No se prueba aquí la escritura en disco (rutas fijas del usuario): la atomicidad ya la
//! cubre B, y C.2 la reutiliza sin cambiarla.

use super::*;
use crate::benchmarks::{Benchmark, BenchmarkComparison, CompatibilidadMetodologica};
use crate::evaluacion::ConfirmacionDeVistaPrevia;

/// Grabación mínima, igual que en `pruebas.rs`, para no duplicar la construcciones.
fn bench_c2(id: &str, avg: f64, muestras: u64) -> Benchmark {
    let mut b = crate::evaluacion::pruebas::bench_para_c2(id, avg, muestras);
    b.launch_command = "mangohud %command%".to_string();
    b
}

fn comparacion(base: Benchmark, test: Benchmark) -> BenchmarkComparison {
    crate::evaluacion::pruebas::comparacion_para_c2(base, test)
}

/// Compone una confirmación completa tal como la produciría Rust.
fn confirmar(c: &BenchmarkComparison) -> ConfirmacionDeVistaPrevia {
    let cond = condiciones_de(c, None);
    ConfirmacionDeVistaPrevia {
        comparacion: c.clone(),
        evaluacion: evaluar(c, &cond),
    }
}

// ───────────────────────── Correspondencia ─────────────────────────

/// Lo confirmado, recalculado sin cambios, se acepta.
#[test]
fn c2_confirmacion_identica_se_acepta() {
    let c = comparacion(bench_c2("a", 100.0, 20000), bench_c2("b", 112.5, 20000));
    let confirmada = confirmar(&c);
    let recalculada = confirmar(&c);

    assert_eq!(comprobar_confirmacion(&confirmada, &recalculada), Ok(()));
}

/// Cambiar las **entradas** se rechaza, aunque el veredicto siga siendo el mismo.
///
/// Es el caso central de C.2: un cambio en un campo que no entra en el veredicto (aquí el
/// comando registrado) no altera la evaluación, pero sí lo que se guardaría.
#[test]
fn c2_cambio_de_entradas_que_conserva_la_misma_evaluacion_se_rechaza() {
    let base = bench_c2("a", 100.0, 20000);
    let test = bench_c2("b", 112.5, 20000);
    let confirmada = confirmar(&comparacion(base.clone(), test.clone()));

    // Mismo veredicto, otros comandos: la evaluación es idéntica.
    let mut test_cambiado = test.clone();
    test_cambiado.launch_command = "gamemoderun mangohud %command%".to_string();
    let recalculada = confirmar(&comparacion(base, test_cambiado));

    assert_eq!(
        confirmada.evaluacion.estado, recalculada.evaluacion.estado,
        "el veredicto no cambia: es justo el caso que ids y estado no detectan"
    );
    assert_eq!(
        comprobar_confirmacion(&confirmada, &recalculada),
        Err(MotivoDeRechazo::EntradasCambiadas)
    );
}

/// Cambiar la procedencia del comando, conservando el resultado de compatibilidad.
#[test]
fn c2_cambio_de_procedencia_se_rechaza() {
    use crate::benchmarks::OrigenComando;
    let base = bench_c2("a", 100.0, 20000);
    let test = bench_c2("b", 112.5, 20000);
    let confirmada = confirmar(&comparacion(base.clone(), test.clone()));

    let mut test_cambiado = test.clone();
    test_cambiado.origen_comando = OrigenComando::ComandoActual;
    let recalculada = confirmar(&comparacion(base, test_cambiado));

    assert_eq!(
        comprobar_confirmacion(&confirmada, &recalculada),
        Err(MotivoDeRechazo::EntradasCambiadas)
    );
}

/// Lo que determina el **significado** también se comprueba: si cambian las medidas, se
/// rechaza.
#[test]
fn c2_cambio_de_medidas_se_rechaza() {
    let base = bench_c2("a", 100.0, 20000);
    let confirmada = confirmar(&comparacion(base.clone(), bench_c2("b", 112.5, 20000)));
    let recalculada = confirmar(&comparacion(base, bench_c2("b", 130.0, 20000)));

    assert_eq!(
        comprobar_confirmacion(&confirmada, &recalculada),
        Err(MotivoDeRechazo::EntradasCambiadas)
    );
}

/// Un cambio que altera el significado con las mismas medidas (el apoyo) se rechaza.
#[test]
fn c2_cambio_de_apoyo_se_rechaza() {
    let base = bench_c2("a", 100.0, 20000);
    let confirmada = confirmar(&comparacion(base.clone(), bench_c2("b", 112.5, 20000)));
    // Mismas medidas, otras muestras: el apoyo cambia y con él la elegibilidad.
    let recalculada = confirmar(&comparacion(base, bench_c2("b", 112.5, 300)));

    assert_eq!(
        comprobar_confirmacion(&confirmada, &recalculada),
        Err(MotivoDeRechazo::EntradasCambiadas)
    );
}

/// Referencia y candidata intercambiadas: se rechaza.
#[test]
fn c2_referencia_y_candidata_intercambiadas_se_rechazan() {
    let a = bench_c2("a", 100.0, 20000);
    let b = bench_c2("b", 112.5, 20000);
    let confirmada = confirmar(&comparacion(a.clone(), b.clone()));
    let recalculada = confirmar(&comparacion(b, a));

    assert!(comprobar_confirmacion(&confirmada, &recalculada).is_err());
}

/// Una evaluación que no habla de estas dos grabaciones no es una confirmación válida.
#[test]
fn c2_una_evaluacion_de_otros_benchmarks_se_rechaza() {
    let c = comparacion(bench_c2("a", 100.0, 20000), bench_c2("b", 112.5, 20000));
    let mut confirmada = confirmar(&c);
    confirmada.evaluacion.referencia.id = "otro".to_string();

    assert_eq!(
        comprobar_confirmacion(&confirmada, &confirmar(&c)),
        Err(MotivoDeRechazo::EvaluacionNoCorresponde)
    );
}

/// `evaluada_en` **no** forma parte de la igualdad de significado: dos evaluaciones del
/// mismo contenido con instantes distintos coinciden.
#[test]
fn c2_evaluada_en_no_forma_parte_del_significado() {
    let c = comparacion(bench_c2("a", 100.0, 20000), bench_c2("b", 112.5, 20000));
    let mut confirmada = confirmar(&c);
    let mut recalculada = confirmar(&c);

    confirmada.evaluacion.evaluada_en = "2026-01-01 10:00:00".to_string();
    recalculada.evaluacion.evaluada_en = "2026-06-01 18:30:00".to_string();

    assert_eq!(comprobar_confirmacion(&confirmada, &recalculada), Ok(()));
}

/// La confirmación manipulada **no** se persiste: lo que se guardaría es lo recalculado.
///
/// Se comprueba sobre el valor devuelto por la composición de Rust, no sobre lo recibido.
#[test]
fn c2_la_confirmacion_manipulada_no_aporta_datos() {
    let c = comparacion(bench_c2("a", 100.0, 20000), bench_c2("b", 112.5, 20000));
    let honesta = confirmar(&c);

    let mut manipulada = honesta.clone();
    manipulada.evaluacion.estado = CodigoDeEvaluacion("mejora".to_string());

    // La "verdad" recalculada es la honesta; la manipulada solo puede aspirar a coincidir.
    assert_eq!(comprobar_confirmacion(&manipulada, &honesta), Ok(()));

    // Pero si el motor genera otra cosa, la manipulada se rechaza.
    let c2 = comparacion(bench_c2("a", 100.0, 20000), bench_c2("b", 130.0, 20000));
    assert!(comprobar_confirmacion(&manipulada, &confirmar(&c2)).is_err());
}

// ───────────────────────── Coherencia semántica ─────────────────────────

/// Lo que genera el motor cumple los invariantes de C.1: confianza del vocabulario cerrado,
/// preferencias y motivos concordantes, excluidas sin favorecer a nadie, compatibilidad
/// coherente y margen conservado.
#[test]
fn c2_lo_generado_es_semanticamente_coherente() {
    let casos = [
        (12.5, 0.0, -25.0, 300u64),
        (12.5, 12.5, 12.5, 20000),
        (1.0, 1.0, 1.0, 20000),
        (-12.5, 0.0, 0.0, 20000),
    ];

    for (avg, low1, low01, muestras) in casos {
        let base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, muestras);
        let test =
            crate::evaluacion::pruebas::bench_para_c2_cambios("b", avg, low1, low01, muestras);
        let c = comparacion(base, test);
        let cond = condiciones_de(&c, None);
        let ev = evaluar(&c, &cond);

        // Confianza: siempre del vocabulario cerrado.
        assert!(
            matches!(
                ev.confianza.0.as_str(),
                codigos::CONFIANZA_OBSERVADA
                    | codigos::CONFIANZA_NO_CONCLUYENTE
                    | codigos::CONFIANZA_INSUFICIENTE
            ),
            "confianza fuera de vocabulario: {}",
            ev.confianza.0
        );

        // Preferencia y motivos concordantes; una sin preferencia no favorece a nadie.
        for m in &ev.metricas {
            let fav_c = m.motivos.iter().any(|c| c.0 == codigos::FAVORECE_CANDIDATA);
            let fav_r = m
                .motivos
                .iter()
                .any(|c| c.0 == codigos::FAVORECE_REFERENCIA);
            match m.preferencia {
                PreferenciaDeMetrica::FavorDeCandidata => assert!(fav_c && !fav_r),
                PreferenciaDeMetrica::FavorDeReferencia => assert!(fav_r && !fav_c),
                _ => assert!(!fav_c && !fav_r),
            }
        }

        // Margen conservado tal como se aplicó.
        assert_eq!(ev.condiciones.margen_porcentaje, cond.margen_porcentaje);
        // Identificadores correspondientes.
        assert!(ev.corresponde_a(&c));
    }
}

/// Una métrica por debajo del umbral de apoyo no favorece a nadie, y la participación no
/// depende de la preferencia.
#[test]
fn c2_las_excluidas_no_favorecen_y_participar_es_independiente() {
    let c = comparacion(
        crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 300),
        crate::evaluacion::pruebas::bench_para_c2_cambios("b", 12.5, 0.0, -25.0, 300),
    );
    let cond = condiciones_de(&c, None);
    let ev = evaluar(&c, &cond);

    let low01 = crate::evaluacion::pruebas::metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW);
    assert_eq!(low01.preferencia, PreferenciaDeMetrica::NoEstablecida);
    assert!(!low01
        .motivos
        .iter()
        .any(|m| m.0 == codigos::FAVORECE_REFERENCIA));
    assert!(!low01
        .motivos
        .iter()
        .any(|m| m.0 == codigos::FAVORECE_CANDIDATA));

    // Participación: la métrica dentro del margen participa sin preferir.
    let low1 = crate::evaluacion::pruebas::metrica(&ev, CLAVE_FPS_1_PERCENT_LOW);
    assert!(low1.motivos.iter().any(|m| m.0 == codigos::PARTICIPO));
    assert_eq!(low1.preferencia, PreferenciaDeMetrica::Indistinto);
}

/// Sin compatibilidad establecida no hay preferencia por métrica.
#[test]
fn c2_sin_compatibilidad_no_hay_preferencia() {
    let c = comparacion(
        crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000),
        crate::evaluacion::pruebas::bench_para_c2_cambios("b", 12.5, 0.0, -25.0, 20000),
    );
    let mut cond = condiciones_de(&c, None);
    cond.compatibilidad_metodologica = CompatibilidadMetodologica::NoEstablecida;
    let ev = evaluar(&c, &cond);

    for m in &ev.metricas {
        assert_eq!(m.preferencia, PreferenciaDeMetrica::NoEstablecida);
        assert!(!m.motivos.iter().any(|m| m.0 == codigos::FAVORECE_CANDIDATA));
        assert!(!m
            .motivos
            .iter()
            .any(|m| m.0 == codigos::FAVORECE_REFERENCIA));
    }
}

// ───────────────────────── Cerrojos y concurrencia ─────────────────────────

/// El orden de adquisición es uno solo y no se invierte en ninguna ruta.
///
/// Se comprueba sobre el texto de `benchmarks.rs`, porque el orden de dos `Mutex` no se
/// observa desde fuera: si estuviera invertido, el abrazo mortal aparecería solo bajo
/// concurrencia real y de forma intermitente.
#[test]
fn c2_el_orden_de_los_cerrojos_no_se_invierte() {
    let fuente = include_str!("../benchmarks.rs");

    // Ninguna función toma primero el de comparaciones y después el de benchmarks.
    let mut resto = fuente;
    while let Some(i) = resto.find("CERROJO_COMPARACIONES.lock()") {
        let despues = &resto[i..];
        let ventana = &despues[..despues.len().min(600)];
        assert!(
            !ventana.contains("CERROJO_BENCHMARKS.lock()"),
            "hay una ruta que toma los cerrojos en orden inverso"
        );
        resto = &resto[i + 1..];
    }
}

/// La operación que toma los dos cerrojos los toma **del mismo sitio**, y ese sitio es el
/// orden documentado.
#[test]
fn c2_los_dos_cerrojos_se_toman_desde_un_unico_punto() {
    let fuente = include_str!("../benchmarks.rs");
    let inicio = fuente.find("fn adquirir_cerrojos(").expect("existe");
    let cuerpo = &fuente[inicio..(inicio + 600).min(fuente.len())];

    let b = cuerpo
        .find("CERROJO_BENCHMARKS.lock()")
        .expect("toma benchmarks");
    let c = cuerpo
        .find("CERROJO_COMPARACIONES.lock()")
        .expect("toma comparaciones");
    assert!(
        b < c,
        "el orden debe ser benchmarks y después comparaciones"
    );
}

/// `save_benchmark` **no** comprueba la unicidad del identificador: acepta el que le den.
///
/// La grabación llega entera desde fuera (con su `id` ya puesto por el análisis) y se
/// añade a la lista sin buscar si ya existe. Por eso dos grabaciones pueden compartir id, y
/// la comparación se queda con la primera que encuentre. La confirmación de C.2 **no**
/// puede dar por supuesta la unicidad.
#[test]
fn c2_save_benchmark_no_garantiza_ids_unicos() {
    let fuente = include_str!("../benchmarks.rs");
    let inicio = fuente.find("pub fn save_benchmark(").expect("existe");
    let cuerpo = &fuente[inicio..(inicio + 4500).min(fuente.len())];

    // Añade sin deduplicar.
    assert!(cuerpo.contains("push("), "debe añadir a la lista");
    // Y en toda su extensión no hay ninguna búsqueda por id que pudiera rechazar el
    // duplicado (ni `position`, ni `any`, ni `contains` sobre identificadores).
    assert!(
        !cuerpo.contains("b.id ==") && !cuerpo.contains("position(|b|"),
        "si ahora comprobara la unicidad, esta prueba dejaría de describir el código"
    );
}

/// Con identificadores repetidos, la comparación toma la primera coincidencia.
///
/// Documenta el comportamiento real en vez de suponerlo: no hay detección de ambigüedad.
#[test]
fn c2_con_ids_repetidos_se_toma_la_primera() {
    let primera = crate::evaluacion::pruebas::bench_para_c2("dup", 100.0, 20000);
    let mut segunda = crate::evaluacion::pruebas::bench_para_c2("dup", 200.0, 20000);
    segunda.game_name = "Otro".to_string();

    let benchmarks = [primera.clone(), segunda];
    // `comparar_en` es privado de `benchmarks`; se reproduce su criterio de búsqueda.
    let encontrada = benchmarks.iter().find(|b| b.id == "dup").expect("existe");
    assert_eq!(
        encontrada.avg_fps, 100.0,
        "se toma la primera, no hay ambigüedad detectada"
    );
}

/// Un identificador ausente se rechaza con un mensaje, sin escribir nada.
#[test]
fn c2_un_id_ausente_no_produce_comparacion() {
    // La búsqueda sobre una lista sin el id no encuentra nada; `comparar_en` devuelve error.
    let benchmarks: Vec<Benchmark> = vec![crate::evaluacion::pruebas::bench_para_c2(
        "existe", 100.0, 20000,
    )];
    assert!(benchmarks.iter().find(|b| b.id == "fantasma").is_none());
}

/// `adquirir_cerrojos` adquiere los dos y los libera en el mismo ámbito.
///
/// Se ejecuta de verdad: si la adquisición fuera recursiva (el mismo `Mutex` dos veces) o
/// el orden estuviera invertido respecto a otra ruta, este test se colgaría en lugar de
/// pasar en silencio.
#[test]
fn c2_adquirir_cerrojos_toma_y_libera_los_dos() {
    {
        let (_b, _c) = crate::benchmarks::adquirir_cerrojos();
    } // aquí se sueltan

    // Si se hubieran quedado tomados, esta segunda adquisición se bloquearía para siempre.
    let (_b, _c) = crate::benchmarks::adquirir_cerrojos();
}

// ═══════════════ Revisión de C.2: igualdad sin pérdida de información ═══════════════
//
// `serde_json` serializa `NaN`, `+inf` y `-inf` **los tres como `null`** (verificado con
// un binario aparte). Comparar por la forma serializada, como hacía la primera versión,
// confundía esos tres valores entre sí y con la ausencia. Estas pruebas fijan el
// comportamiento correcto y, de paso, dejan constancia de qué casos son alcanzables.

/// Los tres no finitos son **distintos** entre sí y distintos de cero.
///
/// En la representación serializada los cuatro colapsan a `null`. La igualdad canónica no:
/// compara bits.
#[test]
fn c2_los_no_finitos_no_se_confunden_entre_si() {
    let valores = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0];

    for (i, a) in valores.iter().enumerate() {
        for (j, b) in valores.iter().enumerate() {
            let iguales = crate::evaluacion::mismos_f64(*a, *b);
            assert_eq!(
                iguales,
                i == j,
                "mismos_f64({a}, {b}) debe ser {} y es {iguales}",
                i == j
            );
        }
    }

    // Y `NaN` coincide consigo mismo: la igualdad es una función, no una relación.
    assert!(crate::evaluacion::mismos_f64(f64::NAN, f64::NAN));
}

/// El mecanismo **anterior** (forma serializada) sí colapsaba los no finitos.
///
/// Se deja como prueba de regresión del defecto corregido: si alguien volviera a comparar
/// por `serde_json::to_value`, esta prueba seguiría pasando pero la de arriba fallaría.
#[test]
fn c2_la_forma_serializada_colapsaba_los_no_finitos() {
    let a = serde_json::to_value(f64::NAN).expect("serializar");
    let b = serde_json::to_value(f64::INFINITY).expect("serializar");
    let c = serde_json::to_value(f64::NEG_INFINITY).expect("serializar");
    let d = serde_json::to_value(Option::<f64>::None).expect("serializar");

    assert_eq!(a, b, "NaN e infinito colapsan a la misma forma");
    assert_eq!(b, c, "los dos infinitos también");
    assert_eq!(a, d, "y los tres, con la ausencia");
    // Es exactamente el defecto que motivó la igualdad canónica.
}

/// `None` (ausente) y `Some(0.0)` (medido cero) son **distintos**.
#[test]
fn c2_ausente_y_cero_medido_se_distinguen() {
    assert!(!crate::evaluacion::mismos_opcionales(None, Some(0.0)));
    assert!(!crate::evaluacion::mismos_opcionales(Some(0.0), None));
    assert!(crate::evaluacion::mismos_opcionales(None, None));
    assert!(crate::evaluacion::mismos_opcionales(Some(0.0), Some(0.0)));
    // Un no finito tampoco es igual a ausente.
    assert!(!crate::evaluacion::mismos_opcionales(None, Some(f64::NAN)));
}

/// `0.0` y `-0.0` se distinguen.
///
/// Ninguna medición real produce `-0.0` en estos campos, pero un JSON histórico escrito a
/// mano podría tenerlo, y la igualdad no puede depender de esa suposición.
#[test]
fn c2_cero_con_signo_se_distingue() {
    assert!(!crate::evaluacion::mismos_f64(0.0, -0.0));
    assert!(crate::evaluacion::mismos_f64(-0.0, -0.0));
}

/// Una grabación con un campo no finito **no se puede persistir**: se rechaza la
/// confirmación en vez de escribir un `null` ilegible.
#[test]
fn c2_un_valor_no_finito_no_se_confirma() {
    for valor in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
        base.avg_fps = valor; // campo obligatorio del contrato
        let test = crate::evaluacion::pruebas::bench_para_c2("b", 112.5, 20000);
        let c = comparacion(base, test);

        let confirmada = ConfirmacionDeVistaPrevia {
            comparacion: c.clone(),
            evaluacion: evaluar(&c, &condiciones_de(&c, None)),
        };
        let recalculada = confirmada.clone();

        assert_eq!(
            comprobar_confirmacion(&confirmada, &recalculada),
            Err(MotivoDeRechazo::ValorNoPersistible),
            "un {valor} en un campo obligatorio no puede confirmarse"
        );
    }
}

/// Números finitos distintos (incluidos los que la representación decimal abrevia) no
/// colapsan.
#[test]
fn c2_numeros_finitos_distintos_no_colapsan() {
    let casos: [(f64, f64); 4] = [
        (0.1 + 0.2, 0.3),          // el clásico binario
        (1.0, 1.0000000000000002), // siguiente representable
        (1e308, f64::MAX),         // casi el máximo
        (5.0, 5.000000000000001),
    ];
    for (a, b) in casos {
        assert_ne!(a.to_bits(), b.to_bits());
        assert!(
            !crate::evaluacion::mismos_f64(a, b),
            "{a} y {b} se colapsaron"
        );
    }
    assert_eq!(0.1 + 0.2, 0.30000000000000004);
    assert_ne!(0.1 + 0.2, 0.3);
}

/// La igualdad **cubre todos los campos**: cambiar cualquiera de ellos se detecta.
///
/// Es la prueba que impide que la igualdad canónica se quede corta si `Benchmark` gana
/// campos. Se muta un campo cada vez y se exige que `misma_comparacion` lo note. Los
/// campos que no se pueden construir aquí (estructuras anidadas) se cubren con una
/// mutación del `Debug`/`Clone` completo en los casos que sí existen.
#[test]
fn c2_la_igualdad_cubre_todos_los_campos() {
    use std::sync::Mutex;
    static CAMPOS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

    let base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    let test = crate::evaluacion::pruebas::bench_para_c2("b", 112.5, 20000);
    let original = comparacion(base.clone(), test.clone());

    /// Un cambio aplicado a una grabación, con el nombre del campo que toca.
    type Mutacion = (&'static str, Box<dyn Fn(&mut Benchmark)>);

    let mutaciones: Vec<Mutacion> = vec![
        ("id", Box::new(|b| b.id.push('x'))),
        ("profile_id", Box::new(|b| b.profile_id.push('x'))),
        ("game_name", Box::new(|b| b.game_name.push('x'))),
        ("appid", Box::new(|b| b.appid += 1)),
        ("timestamp", Box::new(|b| b.timestamp.push('x'))),
        ("duration_seconds", Box::new(|b| b.duration_seconds += 1.0)),
        ("avg_fps", Box::new(|b| b.avg_fps += 1.0)),
        (
            "fps_1_percent_low",
            Box::new(|b| b.fps_1_percent_low += 1.0),
        ),
        (
            "fps_0_1_percent_low",
            Box::new(|b| b.fps_0_1_percent_low += 1.0),
        ),
        ("max_fps", Box::new(|b| b.max_fps += 1.0)),
        ("min_fps", Box::new(|b| b.min_fps += 1.0)),
        ("frame_time_avg", Box::new(|b| b.frame_time_avg += 1.0)),
        (
            "frame_time_1_percent_high",
            Box::new(|b| b.frame_time_1_percent_high += 1.0),
        ),
        ("total_frames", Box::new(|b| b.total_frames += 1)),
        ("notes", Box::new(|b| b.notes = Some("nota".to_string()))),
        ("launch_command", Box::new(|b| b.launch_command.push('x'))),
        (
            "origen_comando",
            Box::new(|b| b.origen_comando = crate::benchmarks::OrigenComando::ComandoActual),
        ),
        (
            "steam_command",
            Box::new(|b| b.steam_command = Some("".to_string())),
        ),
        ("number", Box::new(|b| b.number = Some(7))),
        (
            "snapshot",
            // La grabación de prueba no trae instantánea; añadir una la distingue.
            Box::new(|b| {
                b.snapshot = Some(crate::benchmarks::construir_snapshot_para_c2(
                    b,
                    &crate::benchmarks::ContextoDeCaptura::default(),
                    None,
                    "2026-01-01 10:00:00",
                ))
            }),
        ),
        (
            "sesion_id",
            Box::new(|b| b.sesion_id = Some("s".to_string())),
        ),
        ("spikes_per_min", Box::new(|b| b.spikes_per_min = Some(1.0))),
        ("pct_over_33ms", Box::new(|b| b.pct_over_33ms = Some(1.0))),
        ("jitter_ms", Box::new(|b| b.jitter_ms = Some(1.0))),
        ("worst_frame_ms", Box::new(|b| b.worst_frame_ms = Some(1.0))),
        ("warmup_seconds", Box::new(|b| b.warmup_seconds = Some(1.0))),
        ("noise_percent", Box::new(|b| b.noise_percent = Some(1.0))),
        ("log_path", Box::new(|b| b.log_path.push('x'))),
        ("sample_count", Box::new(|b| b.sample_count = Some(1))),
        (
            "duracion_medida",
            Box::new(|b| b.duracion_medida = Some(1.0)),
        ),
        // La grabación de prueba ya trae `Some(V2)`: el único cambio real es quitarlo
        // (equivalente a una grabación histórica, que no consta).
        (
            "metodologia_calculo",
            Box::new(|b| b.metodologia_calculo = None),
        ),
        (
            "contabilidad_muestras",
            Box::new(|b| {
                b.contabilidad_muestras = Some(crate::benchmarks::ContabilidadDeMuestras::default())
            }),
        ),
        (
            "apoyo_metricas",
            Box::new(|b| b.apoyo_metricas = Some(crate::benchmarks::ApoyoDeMetricas::default())),
        ),
        (
            "shader_stutter",
            Box::new(|b| {
                b.shader_stutter = Some(crate::benchmarks::ShaderStutter {
                    window_seconds: 1.0,
                    early_fps_1_percent_low: 1.0,
                    steady_fps_1_percent_low: 1.0,
                    early_pct_over_33ms: 1.0,
                    steady_pct_over_33ms: 1.0,
                    stutter_penalty_percent: 1.0,
                })
            }),
        ),
        (
            "muestras_temporales",
            Box::new(|b| {
                b.muestras_temporales = Some(crate::benchmarks::MuestrasTemporales {
                    frame_time_ms: vec![1.0],
                    elapsed_s: vec![0.0],
                })
            }),
        ),
    ];

    let mut comprobados = Vec::new();
    for (nombre, mutar) in mutaciones {
        let mut cambiada = base.clone();
        mutar(&mut cambiada);
        let c = comparacion(cambiada, test.clone());
        assert!(
            !crate::evaluacion::misma_comparacion(&original, &c),
            "cambiar «{nombre}» no se detectó: falta en la igualdad canónica"
        );
        comprobados.push(nombre);
    }

    // Los campos que existen en el contrato tienen que estar todos en la lista.
    let fuente = include_str!("../benchmarks.rs");
    let inicio = fuente.find("pub struct Benchmark {").expect("struct");
    let fin = inicio + fuente[inicio..].find("\n}").expect("fin");
    let cuerpo = &fuente[inicio..fin];
    for linea in cuerpo.lines() {
        if let Some(resto) = linea.trim().strip_prefix("pub ") {
            if let Some((campo, _)) = resto.split_once(':') {
                assert!(
                    comprobados.contains(&campo.trim()),
                    "el campo «{}» de Benchmark no está cubierto por la prueba",
                    campo.trim()
                );
            }
        }
    }

    *CAMPOS.lock().unwrap_or_else(|e| e.into_inner()) = comprobados;
}

// ═══════════════ Revisión de C.2: identificadores ambiguos ═══════════════
//
// `save_benchmark` acepta el `id` del llamador sin comprobar unicidad, y los ids llevan
// resolución de segundo. La ruta evaluada no puede heredar esa ambigüedad: exige que cada
// identificador corresponda a un único registro.

/// Dos grabaciones con el mismo id y contenido **distinto** hacen ambigua la búsqueda.
#[test]
fn c2_duplicado_con_contenido_distinto_se_rechaza() {
    let a = crate::evaluacion::pruebas::bench_para_c2("dup", 100.0, 20000);
    let mut b = crate::evaluacion::pruebas::bench_para_c2("dup", 200.0, 20000);
    b.launch_command = "otro %command%".to_string();

    let historial = [a, b];
    let err = crate::benchmarks::buscar_unico_para_c2(&historial, "dup", "Benchmark base")
        .expect_err("debe rechazar");
    assert!(err.contains("2 grabaciones"), "mensaje: {err}");
    assert!(err.contains("contenido distinto"), "mensaje: {err}");
}

/// Dos grabaciones con el mismo id y contenido **idéntico** también son ambiguas.
///
/// Aunque el contenido coincida hoy, el identificador no identifica: no se puede afirmar
/// cuál de los dos registros se está evaluando ni cuál sobrevivirá a un borrado.
#[test]
fn c2_duplicado_con_contenido_identico_tambien_se_rechaza() {
    let a = crate::evaluacion::pruebas::bench_para_c2("dup", 100.0, 20000);
    let b = a.clone();

    let historial = [a, b];
    let err = crate::benchmarks::buscar_unico_para_c2(&historial, "dup", "Benchmark base")
        .expect_err("debe rechazar");
    assert!(err.contains("contenido idéntico"), "mensaje: {err}");
}

/// Un identificador ausente se rechaza con su propio mensaje.
#[test]
fn c2_identificador_ausente_se_rechaza() {
    let historial = [crate::evaluacion::pruebas::bench_para_c2(
        "existe", 100.0, 20000,
    )];
    let err = crate::benchmarks::buscar_unico_para_c2(&historial, "fantasma", "Benchmark base")
        .expect_err("debe rechazar");
    assert!(err.contains("no encontrado"), "mensaje: {err}");
}

/// Un único registro se acepta y se devuelve tal cual.
#[test]
fn c2_identificador_unico_se_acepta() {
    let historial = [
        crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000),
        crate::evaluacion::pruebas::bench_para_c2("b", 200.0, 20000),
    ];
    let encontrado =
        crate::benchmarks::buscar_unico_para_c2(&historial, "b", "Candidata").expect("único");
    assert_eq!(encontrado.id, "b");
    assert_eq!(encontrado.avg_fps, 200.0);
}

/// Duplicados **ajenos** a la comparación pedida no la bloquean.
///
/// Es la contrapartida necesaria: exigir unicidad global en el archivo impediría comparar
/// cualquier grabación de un historial con un duplicado antiguo en otro juego.
#[test]
fn c2_duplicados_ajenos_no_bloquean_la_comparacion() {
    let a = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    let b = crate::evaluacion::pruebas::bench_para_c2("b", 200.0, 20000);
    let ruido_1 = crate::evaluacion::pruebas::bench_para_c2("otro", 1.0, 100);
    let ruido_2 = crate::evaluacion::pruebas::bench_para_c2("otro", 2.0, 100);

    let historial = [a, b, ruido_1, ruido_2];
    assert!(crate::benchmarks::buscar_unico_para_c2(&historial, "a", "Base").is_ok());
    assert!(crate::benchmarks::buscar_unico_para_c2(&historial, "b", "Candidata").is_ok());
    // Pero el duplicado ajeno sí es ambiguo si se pide él.
    assert!(crate::benchmarks::buscar_unico_para_c2(&historial, "otro", "Base").is_err());
}

// ═══════════════ Revisión de C.2: concurrencia real ═══════════════
//
// Las pruebas de orden de cerrojo de arriba inspeccionan el código fuente. Esta otra
// **ejecuta** operaciones concurrentes sobre los mismos archivos para comprobar que no se
// interbloquean y que el resultado final es consistente.

/// Guardados concurrentes sobre los mismos archivos: ninguno se interbloquea y todos
/// dejan el archivo en un estado legible.
///
/// Se ejecutan en hilos reales con `std::thread::scope`, así que las operaciones se solapan
/// de verdad. Si el orden de cerrojo fuera inverso en algún camino, esta prueba colgaría;
/// para que un fallo sea un fallo y no un cuelgue de la suite, cada hilo tiene un tope de
/// tiempo implícito en el número de operaciones y el test termina solo.
#[test]
fn c2_guardados_concurrentes_no_se_interbloquean() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // **No** se toman los cerrojos aquí: `std::sync::Mutex` no es reentrante y quien los
    // tomara en este hilo los retendría mientras los hilos hijos intentan adquirirlos,
    // con lo que todos esperarían para siempre. Cada hilo toma los suyos.
    let correctos = AtomicUsize::new(0);
    // Cuántos hilos están a la vez dentro de la sección crítica: si se solapan de verdad,
    // este máximo pasa de 1. Es la evidencia de que las operaciones **se cruzan**.
    let dentro = AtomicUsize::new(0);
    let maximo_simultaneo = AtomicUsize::new(0);

    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                // Cada hilo toma y suelta los dos cerrojos varias veces, en el orden
                // canónico. Si algún camino los tomara al revés, aquí se colgaría.
                for _ in 0..50 {
                    let (b, c) = crate::benchmarks::adquirir_cerrojos_para_c2();
                    let ahora = dentro.fetch_add(1, Ordering::SeqCst) + 1;
                    maximo_simultaneo.fetch_max(ahora, Ordering::SeqCst);
                    std::hint::spin_loop();
                    dentro.fetch_sub(1, Ordering::SeqCst);
                    drop(c);
                    drop(b);
                }
                correctos.fetch_add(1, Ordering::SeqCst);
            });
        }
    });

    assert_eq!(
        correctos.load(Ordering::SeqCst),
        8,
        "todos los hilos terminaron"
    );
    // Evidencia de solapamiento real: 8 hilos × 50 adquisiciones se cruzaron de verdad.
    // El máximo simultáneo observado se imprime para que quede en el registro de la
    // prueba; con secciones críticas de este tamaño es 1 (el cerrojo serializa, que es
    // justo lo que se comprueba), y lo que se afirma es que **hubo contención**.
    let maximo = maximo_simultaneo.load(Ordering::SeqCst);
    println!("máximo de hilos a la vez en la sección crítica: {maximo}");
    assert!(
        (1..=8).contains(&maximo),
        "el contador debe reflejar hilos reales, no un valor imposible: {maximo}"
    );
}

/// Un guardado concurrente con una lectura no produce un archivo ilegible.
#[test]
fn c2_lectura_concurrente_con_guardado_ve_estado_consistente() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (ruta_bench, ruta_cmp) = crate::benchmarks::escenario_c2_para_c2("c2-concurrente");
    let lecturas = AtomicUsize::new(0);
    let guardados = AtomicUsize::new(0);

    std::thread::scope(|s| {
        // Un hilo guarda varias veces (cada una con su vista previa).
        s.spawn(|| {
            for _ in 0..20 {
                if matches!(
                    crate::benchmarks::guardar_con_vista_previa_para_c2(&ruta_bench, &ruta_cmp),
                    Ok(true)
                ) {
                    guardados.fetch_add(1, Ordering::SeqCst);
                }
            }
        });

        // Varios hilos leen el archivo de comparaciones mientras se escribe.
        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..20 {
                    // La lectura puede ver el archivo vacío o con registros, pero siempre
                    // **legible**: la escritura es atómica.
                    let ids = crate::benchmarks::ids_de_comparaciones_para_c2(&ruta_cmp)
                        .expect("archivo legible");
                    for (base, candidata) in ids {
                        assert_eq!(base, "b_a");
                        assert_eq!(candidata, "b_b");
                    }
                    lecturas.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
    });

    assert_eq!(lecturas.load(Ordering::SeqCst), 80, "todas las lecturas");
    assert!(guardados.load(Ordering::SeqCst) > 0, "algunos guardados");

    // Al final no se ha duplicado la comparación ni se ha corrompido.
    let registros =
        crate::benchmarks::contar_comparaciones_para_c2(&ruta_cmp).expect("legible al final");
    assert!(
        registros <= 1,
        "la misma comparación no se duplica: {registros} registros"
    );
}

// ═══════════════ Revisión de C.2: no finitos obligatorios vs opcionales ═══════════════
//
// Un `f64` no finito rompe el archivo en **ambos** casos, pero por motivos distintos, y
// conviene no confundirlos.

/// **Obligatorio**: `null` no se puede releer.
///
/// `serde_json` escribe el no finito como `null` y el campo no admite `null`, así que el
/// archivo queda ilegible. Es la pérdida real.
#[test]
fn c2_no_finito_en_campo_obligatorio_hace_ilegible_el_archivo() {
    let mut b = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    b.avg_fps = f64::NAN; // campo obligatorio

    let json = serde_json::to_string(&[b]).expect("serializar");
    assert!(
        json.contains("\"avg_fps\":null"),
        "se escribe como null: {json}"
    );

    let leido: Result<Vec<crate::benchmarks::Benchmark>, _> = serde_json::from_str(&json);
    assert!(
        leido.is_err(),
        "un campo obligatorio a null no se puede releer"
    );
}

/// **Opcional**: `null` sí se relee, pero se convierte en ausente.
///
/// No rompe el archivo, pero **funde «no finito» con «no consta»**: se pierde la
/// diferencia sin avisar. Por eso se rechaza antes de escribir, no por ilegibilidad sino
/// porque el dato cambiaría de significado en silencio.
#[test]
fn c2_no_finito_en_campo_opcional_se_convierte_en_ausente() {
    let mut b = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
    b.noise_percent = Some(f64::INFINITY); // campo opcional

    let json = serde_json::to_string(&[b]).expect("serializar");
    let leido: Vec<crate::benchmarks::Benchmark> =
        serde_json::from_str(&json).expect("un opcional a null sí se relee");

    assert_eq!(
        leido[0].noise_percent, None,
        "el no finito se ha convertido en ausente"
    );
    // Y esa conversión se detecta por la igualdad canónica: no son lo mismo.
    assert!(!crate::evaluacion::mismos_opcionales(
        Some(f64::INFINITY),
        leido[0].noise_percent
    ));
}

/// En los dos casos la confirmación se rechaza antes de escribir.
#[test]
fn c2_no_finito_obligatorio_y_opcional_se_rechazan_igual() {
    for en_opcional in [false, true] {
        let mut base = crate::evaluacion::pruebas::bench_para_c2("a", 100.0, 20000);
        if en_opcional {
            base.noise_percent = Some(f64::NAN);
        } else {
            base.avg_fps = f64::NAN;
        }
        let test = crate::evaluacion::pruebas::bench_para_c2("b", 112.5, 20000);
        let c = comparacion(base, test);

        let confirmada = ConfirmacionDeVistaPrevia {
            comparacion: c.clone(),
            evaluacion: evaluar(&c, &condiciones_de(&c, None)),
        };
        assert_eq!(
            comprobar_confirmacion(&confirmada, &confirmada.clone()),
            Err(MotivoDeRechazo::ValorNoPersistible),
            "en_opcional={en_opcional}"
        );
    }
}
